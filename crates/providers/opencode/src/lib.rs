use chrono::{DateTime, Duration, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch, UsageEvent,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{code_for_status, get_text, JsonRequest};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    CollectionOutcome, CollectionResult, DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use lnwdeck_security::IdentifierHasher;
use lnwdeck_windows_integration::{CredentialError, CredentialStore};
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

/// OpenCode CLI local-session collector.
///
/// Reads only metadata columns (`tokens_*`, `cost`, `model`, timestamps)
/// from the OpenCode SQLite store. Session identifiers are persisted as
/// keyed hashes; raw ids, prompts, responses and paths never leave the
/// source as normalized data.
pub struct OpenCodeAdapter {
    hasher: IdentifierHasher,
    db_path: PathBuf,
}

const ADAPTER_VERSION: &str = "0.3.0";

/// Credential Manager id for the OpenCode Go workspace and auth cookie pair.
pub const OPENCODE_GO_CREDENTIAL_ID: &str = "opencode_go";
const OPENCODE_GO_AUTH_COOKIE_ENV: &str = "OPENCODE_GO_AUTH_COOKIE";
const OPENCODE_GO_WORKSPACE_ID_ENV: &str = "OPENCODE_GO_WORKSPACE_ID";
const OPENCODE_GO_DASHBOARD_ORIGIN: &str = "https://opencode.ai";
const MAX_DASHBOARD_HTML_BYTES: usize = 8 * 1024 * 1024;

/// Validated OpenCode Go configuration kept inside one OS credential.
///
/// The cookie is intentionally not `Debug`, `Serialize`, or `Clone` so it
/// cannot accidentally cross a read-model or diagnostic boundary.
struct OpenCodeGoConfig {
    workspace_id: String,
    auth_cookie: String,
}

/// OpenCode may store the model as a JSON descriptor like
/// `{"id":"glm-5.2","providerID":"opencode-go"}`. Reduce it to the model id
/// when possible; otherwise keep the raw string.
fn normalize_model(raw: Option<String>) -> String {
    let Some(model) = raw else {
        return "unknown".to_string();
    };
    let trimmed = model.trim();
    if trimmed.starts_with('{') {
        if let Ok(serde_json::Value::Object(map)) =
            serde_json::from_str::<serde_json::Value>(trimmed)
        {
            if let Some(serde_json::Value::String(id)) = map.get("id") {
                return id.clone();
            }
        }
    }
    trimmed.to_string()
}

impl OpenCodeGoConfig {
    fn new(workspace_id: &str, auth_cookie: &str) -> Result<Self, String> {
        let workspace_id = workspace_id.trim();
        if workspace_id.is_empty()
            || workspace_id.len() > 128
            || !workspace_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("NOT_CONFIGURED".to_string());
        }

        let mut auth_cookie = auth_cookie.trim();
        if let Some(value) = auth_cookie.strip_prefix("auth=") {
            auth_cookie = value.trim();
        }
        if auth_cookie.is_empty()
            || auth_cookie.len() > 4096
            || auth_cookie
                .bytes()
                .any(|byte| matches!(byte, b'\r' | b'\n' | b';'))
        {
            return Err("NOT_CONFIGURED".to_string());
        }

        Ok(Self {
            workspace_id: workspace_id.to_string(),
            auth_cookie: auth_cookie.to_string(),
        })
    }
}

/// Validates and encodes the two OpenCode Go values for Credential Manager.
///
/// The returned JSON is an internal credential payload. It must only be
/// passed to [`CredentialStore::set`] and never returned to the UI, logs, or
/// diagnostics.
pub fn encode_go_config(workspace_id: &str, auth_cookie: &str) -> Result<String, String> {
    let config = OpenCodeGoConfig::new(workspace_id, auth_cookie)?;
    serde_json::to_string(&serde_json::json!({
        "workspace_id": config.workspace_id,
        "auth_cookie": config.auth_cookie,
    }))
    .map_err(|_| "CREDENTIAL_SERIALIZATION_FAILED".to_string())
}

/// Returns the secret-free state of the OpenCode Go credential pair.
pub fn go_config_state() -> &'static str {
    match read_go_config() {
        Ok(Some(_)) => "configured",
        Ok(None) => "missing",
        Err(_) => "expired",
    }
}

fn decode_go_config(serialized: &str) -> Result<OpenCodeGoConfig, String> {
    let value: serde_json::Value =
        serde_json::from_str(serialized).map_err(|_| "NOT_CONFIGURED".to_string())?;
    let workspace_id = value
        .get("workspace_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "NOT_CONFIGURED".to_string())?;
    let auth_cookie = value
        .get("auth_cookie")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "NOT_CONFIGURED".to_string())?;
    OpenCodeGoConfig::new(workspace_id, auth_cookie)
}

fn read_go_config() -> Result<Option<OpenCodeGoConfig>, String> {
    let workspace_from_env = std::env::var(OPENCODE_GO_WORKSPACE_ID_ENV).ok();
    let cookie_from_env = std::env::var(OPENCODE_GO_AUTH_COOKIE_ENV).ok();
    if workspace_from_env.is_some() || cookie_from_env.is_some() {
        let workspace = workspace_from_env.unwrap_or_default();
        let cookie = cookie_from_env.unwrap_or_default();
        return OpenCodeGoConfig::new(&workspace, &cookie).map(Some);
    }

    match CredentialStore::get(OPENCODE_GO_CREDENTIAL_ID) {
        Ok(serialized) => decode_go_config(&serialized).map(Some),
        Err(CredentialError::NotFound | CredentialError::Unsupported) => Ok(None),
        Err(CredentialError::Corrupt) => Err("AUTH_EXPIRED".to_string()),
        Err(error) => Err(error.to_string()),
    }
}

/// Parses the authoritative OpenCode Go workspace dashboard response.
///
/// OpenCode reports utilization and reset seconds for each rolling window but
/// does not expose an absolute dollar limit in this response. The returned
/// windows therefore carry the real percentage and reset timestamp while
/// leaving absolute usage and limits unknown.
pub fn windows_from_dashboard_html(
    html: &str,
    now: DateTime<Utc>,
) -> Result<Vec<QuotaWindow>, String> {
    if html.len() > MAX_DASHBOARD_HTML_BYTES {
        return Err("PROVIDER_RESPONSE_TOO_LARGE".to_string());
    }

    let definitions = [
        ("rollingUsage", "5h", "5-hour", QuotaWindowScope::Rolling),
        ("weeklyUsage", "7d", "7-day", QuotaWindowScope::Weekly),
        ("monthlyUsage", "30d", "30-day", QuotaWindowScope::Monthly),
    ];
    let mut windows = Vec::with_capacity(definitions.len());
    for (object_key, window_key, label, scope) in definitions {
        let Some(segment) = dashboard_object_segment(html, object_key) else {
            continue;
        };
        let Some(raw_percent) = numeric_field(segment, "usagePercent") else {
            continue;
        };
        let Some(raw_reset_seconds) = numeric_field(segment, "resetInSec") else {
            continue;
        };
        let Some(used_percent) = normalize_dashboard_percent(raw_percent) else {
            continue;
        };
        let Some(reset_at) = dashboard_reset_at(now, raw_reset_seconds) else {
            continue;
        };
        windows.push(QuotaWindow::from_percent(
            window_key,
            label,
            scope,
            QuotaKind::Credits,
            used_percent,
            Some(reset_at),
            Confidence::High,
        ));
    }

    if windows.is_empty() {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    Ok(windows)
}

fn dashboard_object_segment<'a>(html: &'a str, object_key: &str) -> Option<&'a str> {
    let key_start = html.find(object_key)?;
    let object_start = key_start + html[key_start..].find('{')?;
    let bytes = html.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().enumerate().skip(object_start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match *byte {
            b'"' => in_string = true,
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return html.get(object_start..=index);
                }
            }
            _ => {}
        }
    }
    None
}

fn numeric_field(segment: &str, field: &str) -> Option<f64> {
    let mut search_from = 0;
    while let Some(relative_start) = segment[search_from..].find(field) {
        let start = search_from + relative_start;
        let mut after = segment.get(start + field.len()..)?.trim_start();
        if let Some(stripped) = after.strip_prefix('"') {
            after = stripped.trim_start();
        }
        let Some(stripped) = after.strip_prefix(':').or_else(|| after.strip_prefix('=')) else {
            search_from = start + field.len();
            continue;
        };
        let value = stripped.trim_start();
        let value = if let Some(quoted) = value.strip_prefix('"') {
            let end = quoted.find('"')?;
            &quoted[..end]
        } else {
            value
        };
        let bytes = value.as_bytes();
        let mut end = 0;
        while end < bytes.len()
            && matches!(bytes[end], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
        {
            end += 1;
        }
        if end == 0 {
            search_from = start + field.len();
            continue;
        }
        if let Ok(parsed) = value[..end].parse::<f64>() {
            return Some(parsed);
        }
        search_from = start + field.len();
    }
    None
}

fn normalize_dashboard_percent(value: f64) -> Option<f64> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let percent = if value > 0.0 && value < 1.0 {
        value * 100.0
    } else {
        value
    };
    if !percent.is_finite() || percent > 100.0 {
        return None;
    }
    Some(percent)
}

fn dashboard_reset_at(now: DateTime<Utc>, seconds: f64) -> Option<DateTime<Utc>> {
    if !seconds.is_finite() || seconds < 0.0 || seconds > i64::MAX as f64 {
        return None;
    }
    now.checked_add_signed(Duration::seconds(seconds.floor() as i64))
}

impl OpenCodeAdapter {
    /// Resolves the OpenCode data location from environment variables and
    /// builds an adapter that hashes source identifiers with `hash_key`.
    pub fn new(hash_key: &[u8]) -> Self {
        let db_path = Self::default_db_path().unwrap_or_else(|| PathBuf::from("opencode.db"));
        Self::with_db_path(hash_key, db_path)
    }

    /// Adapter pinned to an explicit database path (used by tests and
    /// future user-configured sources).
    pub fn with_db_path(hash_key: &[u8], db_path: PathBuf) -> Self {
        Self {
            hasher: IdentifierHasher::new(hash_key),
            db_path,
        }
    }

    fn default_db_path() -> Option<PathBuf> {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            let candidate = PathBuf::from(xdg).join("opencode").join("opencode.db");
            if candidate.exists() {
                return Some(candidate);
            }
        }
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join(".local/share/opencode/opencode.db"))
    }

    fn open_read_only(&self) -> Result<Connection, String> {
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "SOURCE_UNAVAILABLE".to_string())
    }

    fn detection(&self) -> Result<DetectionResult, String> {
        let source_exists = self.db_path.is_file();
        let config_state = read_go_config();
        let configured = matches!(&config_state, Ok(Some(_)));
        let config_error = config_state.as_ref().err().cloned();
        let mut result = DetectionResult {
            provider_id: "opencode".to_string(),
            display_name: "OpenCode (Go)".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_sqlite+credential".to_string(),
            source_type: "sqlite".to_string(),
            source_exists: source_exists || configured,
            permission_state: if configured {
                "credential_stored".to_string()
            } else {
                "credential_required".to_string()
            },
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: config_error.unwrap_or_else(|| {
                if configured {
                    String::new()
                } else {
                    "NOT_CONFIGURED".to_string()
                }
            }),
        };
        if !source_exists {
            if configured {
                result.detected = true;
                result.detection_method = "credential".to_string();
                result.source_type = "remote_api".to_string();
            }
            return Ok(result);
        }

        match self.open_read_only() {
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
                Ok(result)
            }
            Ok(conn) => {
                let has_session_table = conn.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM sqlite_master
                        WHERE type = 'table' AND name = 'session'
                     )",
                    [],
                    |row| row.get(0),
                );
                match has_session_table {
                    Ok(true) => {
                        result.detected = true;
                        if configured {
                            result.permission_state = "read_ok+credential_stored".to_string();
                            result.detection_error_code.clear();
                        }
                        Ok(result)
                    }
                    Ok(false) => {
                        result.detection_error_code = "INVALID_PROVIDER_DATA".to_string();
                        Ok(result)
                    }
                    Err(_) => {
                        result.detection_error_code = "INVALID_PROVIDER_DATA".to_string();
                        Ok(result)
                    }
                }
            }
        }
    }

    fn collect(&self, cursor: Option<&str>) -> CollectionResult {
        let started_at = Utc::now();
        let conn = match self.open_read_only() {
            Ok(conn) => conn,
            Err(code) => {
                return CollectionResult {
                    batch: None,
                    outcome: CollectionOutcome::failure(
                        "opencode",
                        "passive_scan",
                        started_at,
                        code,
                    ),
                    next_cursor: cursor.map(str::to_string),
                };
            }
        };

        let mut stmt = match conn.prepare(
            "SELECT id, project_id, model, cost, tokens_input, tokens_output,
                    tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated
             FROM session
             WHERE (tokens_input > 0 OR tokens_output > 0 OR tokens_reasoning > 0
                    OR tokens_cache_read > 0 OR tokens_cache_write > 0 OR cost > 0)
               AND (?1 IS NULL OR time_updated > ?1)
             ORDER BY time_updated",
        ) {
            Ok(stmt) => stmt,
            Err(_) => {
                return CollectionResult {
                    batch: None,
                    outcome: CollectionOutcome::failure(
                        "opencode",
                        "passive_scan",
                        started_at,
                        "INVALID_PROVIDER_DATA",
                    ),
                    next_cursor: cursor.map(str::to_string),
                };
            }
        };

        let rows = match stmt.query_map([cursor], |row| {
            let session_id: String = row.get(0)?;
            let project_id: String = row.get(1)?;
            let model: Option<String> = row.get(2)?;
            let cost: f64 = row.get(3)?;
            let tokens_input: i64 = row.get(4)?;
            let tokens_output: i64 = row.get(5)?;
            let tokens_reasoning: i64 = row.get(6)?;
            let tokens_cache_read: i64 = row.get(7)?;
            let tokens_cache_write: i64 = row.get(8)?;
            let time_updated: i64 = row.get(9)?;
            Ok((
                session_id,
                project_id,
                model,
                cost,
                tokens_input,
                tokens_output,
                tokens_reasoning,
                tokens_cache_read,
                tokens_cache_write,
                time_updated,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => {
                return CollectionResult {
                    batch: None,
                    outcome: CollectionOutcome::failure(
                        "opencode",
                        "passive_scan",
                        started_at,
                        "INVALID_PROVIDER_DATA",
                    ),
                    next_cursor: cursor.map(str::to_string),
                };
            }
        };

        let mut events = Vec::new();
        let mut seen: u64 = 0;
        let mut parsed: u64 = 0;
        let mut warnings = Vec::new();
        let mut next_cursor = cursor.map(str::to_string);

        for row in rows {
            seen += 1;
            let (
                session_id,
                project_id,
                model,
                cost,
                tokens_input,
                tokens_output,
                tokens_reasoning,
                tokens_cache_read,
                tokens_cache_write,
                time_updated,
            ) = match row {
                Ok(values) => values,
                Err(_) => {
                    warnings.push("ROW_SKIPPED".to_string());
                    continue;
                }
            };
            let Some(timestamp) = DateTime::from_timestamp_millis(time_updated) else {
                warnings.push("ROW_SKIPPED".to_string());
                continue;
            };

            let fingerprint = self
                .hasher
                .hash(format!("opencode:{session_id}:{time_updated}").as_bytes());

            events.push(UsageEvent {
                id: fingerprint,
                timestamp,
                provider_id: "opencode".to_string(),
                model: normalize_model(model),
                tokens_input: tokens_input.max(0) as u64,
                tokens_cached: tokens_cache_read.max(0) as u64,
                tokens_cache_write: tokens_cache_write.max(0) as u64,
                tokens_output: tokens_output.max(0) as u64,
                tokens_reasoning: tokens_reasoning.max(0) as u64,
                confidence: Confidence::High,
                data_source: "opencode_db".to_string(),
                cost: format!("{cost:.6}"),
                session_hash: Some(
                    self.hasher
                        .hash(format!("opencode:session:{session_id}").as_bytes()),
                ),
                project_hash: Some(
                    self.hasher
                        .hash(format!("opencode:project:{project_id}").as_bytes()),
                ),
            });
            parsed += 1;
            next_cursor = Some(time_updated.to_string());
        }

        let events_normalized = events.len() as u64;
        let mut outcome =
            CollectionOutcome::success("opencode", "passive_scan", started_at, events_normalized);
        outcome.source_records_seen = seen;
        outcome.records_parsed = parsed;
        if !warnings.is_empty() {
            outcome = outcome.with_warning("ROW_SKIPPED");
        }

        CollectionResult {
            batch: Some(UsageBatch {
                batch_id: format!("opencode_{}", Utc::now().timestamp()),
                events,
            }),
            outcome,
            next_cursor,
        }
    }

    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        let config = read_go_config()?;

        quota_report_from_go_config(config.as_ref(), |config| self.fetch_dashboard(config))
    }

    fn fetch_dashboard(&self, config: &OpenCodeGoConfig) -> Result<QuotaReport, String> {
        let endpoint = format!(
            "{OPENCODE_GO_DASHBOARD_ORIGIN}/workspace/{}/go",
            config.workspace_id
        );
        let cookie = format!("auth={}", config.auth_cookie);
        let headers = [
            ("accept", "text/html,application/xhtml+xml"),
            ("referer", OPENCODE_GO_DASHBOARD_ORIGIN),
            ("user-agent", "lnwdeck/0.3"),
        ];
        let (status, html) = get_text(
            JsonRequest {
                timeout: std::time::Duration::from_secs(10),
                ..JsonRequest::new(&endpoint)
            }
            .with_headers(&headers)
            .browser_cookie(&cookie),
        )?;
        if !(200..300).contains(&status) {
            return Err(code_for_status(status).to_string());
        }

        let windows = windows_from_dashboard_html(&html, Utc::now())?;
        let mut report = QuotaReport::new("opencode", "provider_api", windows, DEFAULT_FRESHNESS);
        report.plan = Some("OpenCode Go".to_string());
        Ok(report)
    }
}

fn quota_report_from_go_config<F>(
    config: Option<&OpenCodeGoConfig>,
    fetch: F,
) -> Result<Option<QuotaReport>, String>
where
    F: FnOnce(&OpenCodeGoConfig) -> Result<QuotaReport, String>,
{
    match config {
        Some(config) => fetch(config).map(Some),
        None => Err("NOT_CONFIGURED".to_string()),
    }
}

impl ProviderAdapter for OpenCodeAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: "opencode",
            display_name: "OpenCode (Go)",
            vendor: "OpenCode",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::BrowserCookie,
            adapter_version: ADAPTER_VERSION,
        }
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        self.collect(None)
            .batch
            .ok_or_else(|| "SOURCE_UNAVAILABLE".to_string())
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }
    fn health_check(&self) -> AdapterHealth {
        match self.detection() {
            Ok(detection) if detection.detected && detection.detection_error_code.is_empty() => {
                AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "OpenCode Go credentials configured".to_string(),
                }
            }
            Ok(detection) if detection.detection_error_code == "NOT_CONFIGURED" => AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "OpenCode Go credentials are not configured".to_string(),
            },
            Ok(detection) if detection.detected => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "OpenCode Go credentials are not configured".to_string(),
            },
            Ok(_) => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "OpenCode local store not found".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: code,
            },
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FileSystem,
            Permission::Network,
            Permission::Credential,
        ]
    }
    fn detect(&self) -> Result<DetectionResult, String> {
        self.detection()
    }
    fn collect_usage_with_cursor(&self, cursor: Option<&str>) -> CollectionResult {
        self.collect(cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_go_config_returns_not_configured_without_a_report() {
        let result = quota_report_from_go_config(None, |_config| {
            panic!("a missing credential must not trigger a dashboard request")
        });

        assert_eq!(result, Err("NOT_CONFIGURED".to_string()));
    }

    #[test]
    fn go_config_is_validated_and_auth_prefix_is_normalized() {
        let encoded = encode_go_config("workspace-test_123", "auth=cookie-value")
            .expect("valid workspace and cookie");
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("credential JSON");
        assert_eq!(value["workspace_id"], "workspace-test_123");
        assert_eq!(value["auth_cookie"], "cookie-value");

        for (workspace, cookie) in [
            ("", "cookie"),
            ("workspace", ""),
            ("workspace", "cookie; other=value"),
            ("workspace/unsafe", "cookie"),
        ] {
            assert_eq!(
                encode_go_config(workspace, cookie),
                Err("NOT_CONFIGURED".to_string())
            );
        }
    }
}
