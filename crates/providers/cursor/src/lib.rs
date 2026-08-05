//! Cursor account collector.
//!
//! Cursor does not store per-request token usage in the local editor state;
//! usage and quota live on the account side. Cursor's own tooling stores a
//! session JWT in its local SQLite store (`state.vscdb`, key
//! `cursorAuth/accessToken`) and the account id in `~/.cursor/cli-config.json`
//! (or inside the JWT itself). This adapter reuses that local credential to
//! ask Cursor's own API for the per-request usage CSV and the usage summary,
//! exactly like TokenTracker does:
//!
//! - `GET https://cursor.com/api/dashboard/export-usage-events-csv?strategy=tokens`
//! - `GET https://cursor.com/api/usage-summary`
//!
//! The credential never leaves the machine except over HTTPS to cursor.com,
//! and it never enters lnwdeck storage, logs or the UI. Every request is
//! bounded by an explicit timeout; failures reduce to sanitized error codes.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch, UsageEvent,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::JsonRequest;
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

const PROVIDER_ID: &str = "cursor_ide";
const ADAPTER_VERSION: &str = "0.3.0";
const DATA_SOURCE: &str = "cursor_api";
const USAGE_CSV_URL: &str =
    "https://cursor.com/api/dashboard/export-usage-events-csv?strategy=tokens";
const QUOTA_URL: &str = "https://cursor.com/api/usage-summary";
/// Cursor's own session store key holding the account JWT.
const ACCESS_TOKEN_SQL: &str = "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken'";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct CursorAdapter {
    state_db: PathBuf,
    cli_config: PathBuf,
    usage_url: String,
    quota_url: String,
    timeout: Duration,
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorAdapter {
    pub fn new() -> Self {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".config"))
            })
            .unwrap_or_default();
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            state_db: base
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb"),
            cli_config: home.join(".cursor").join("cli-config.json"),
            usage_url: USAGE_CSV_URL.to_string(),
            quota_url: QUOTA_URL.to_string(),
            timeout: Duration::from_secs(15),
        }
    }

    /// Adapter pinned to explicit paths and endpoints (used by tests and by
    /// future user-configured sources).
    pub fn with_urls(
        state_db: PathBuf,
        cli_config: PathBuf,
        usage_url: impl Into<String>,
        quota_url: impl Into<String>,
    ) -> Self {
        Self {
            state_db,
            cli_config,
            usage_url: usage_url.into(),
            quota_url: quota_url.into(),
            timeout: Duration::from_secs(5),
        }
    }

    fn open_read_only(&self) -> Result<Connection, String> {
        Connection::open_with_flags(&self.state_db, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "SOURCE_UNAVAILABLE".to_string())
    }

    /// Reads Cursor's own session credential from the local editor state.
    fn session_cookie(&self) -> Result<String, String> {
        let jwt = self.access_token()?;
        let user_id = self
            .user_id_from_cli_config()
            .or_else(|| decode_jwt_sub(&jwt))
            .ok_or_else(|| "NOT_CONFIGURED".to_string())?;
        Ok(format!("WorkosCursorSessionToken={user_id}%3A%3A{jwt}"))
    }

    fn access_token(&self) -> Result<String, String> {
        let conn = self.open_read_only()?;
        let token: Option<String> = conn
            .query_row(ACCESS_TOKEN_SQL, [], |row| row.get(0))
            .optional()
            .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        match token {
            Some(token) if !token.trim().is_empty() => Ok(token),
            _ => Err("NOT_CONFIGURED".to_string()),
        }
    }

    fn user_id_from_cli_config(&self) -> Option<String> {
        let text = std::fs::read_to_string(&self.cli_config).ok()?;
        let config: Value = serde_json::from_str(&text).ok()?;
        let auth_id = config.get("authInfo")?.get("authId")?.as_str()?;
        normalize_user_id(auth_id)
    }

    fn fetch_text(&self, url: &str, cookie: &str) -> Result<String, String> {
        let (_, body) = lnwdeck_provider_http::get_text(JsonRequest {
            url,
            bearer_token: None,
            timeout: self.timeout,
            capture_headers: &[],
            extra_headers: &[
                ("cookie", cookie),
                ("referer", "https://www.cursor.com/settings"),
                ("user-agent", USER_AGENT),
            ],
        })?;
        Ok(body)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.state_db.is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Cursor".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_credential".to_string(),
            source_type: DATA_SOURCE.to_string(),
            source_exists,
            permission_state: "n/a".to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        };
        if !source_exists {
            result.permission_state = "not_found".to_string();
            return result;
        }
        match self.access_token() {
            Ok(_) => {
                result.detected = true;
                result.permission_state = "read_ok".to_string();
            }
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
            }
        }
        result
    }

    /// Downloads and parses the per-request usage CSV.
    fn fetch_usage_rows(&self) -> Result<Vec<CursorUsage>, String> {
        let cookie = self.session_cookie()?;
        let text = self.fetch_text(&self.usage_url, &cookie)?;
        parse_usage_csv(&text)
    }
}

/// One row of Cursor's usage export CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorUsage {
    pub date: String,
    pub model: String,
    pub input_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost: f64,
}

/// Normalizes Cursor's account subject into the id used inside its session
/// cookie: native `auth0|user_XXXXX` becomes `user_XXXXX`; WorkOS-bridged
/// OAuth subjects (`google-oauth2|…`, `github|…`, `oidc|…`) stay verbatim.
pub fn normalize_user_id(subject: &str) -> Option<String> {
    let trimmed = subject.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("auth0|") {
        if rest.starts_with("user_") && !rest.is_empty() {
            return Some(rest.to_string());
        }
        return None;
    }
    for prefix in ["google-oauth2|", "github|", "oidc|"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if !rest.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Decodes the `sub` claim of a Cursor session JWT. Used when
/// `~/.cursor/cli-config.json` is absent.
pub fn decode_jwt_sub(jwt: &str) -> Option<String> {
    use base64::Engine;
    let mut parts = jwt.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.as_bytes())
        .ok()?;
    let body: Value = serde_json::from_slice(&bytes).ok()?;
    let sub = body.get("sub")?.as_str()?;
    normalize_user_id(sub)
}

/// Parses the usage export CSV, resolving columns by header name because
/// Cursor inserts and reorders columns between releases. Returns an error
/// when a required column is missing; rows with all-zero usage are dropped.
pub fn parse_usage_csv(text: &str) -> Result<Vec<CursorUsage>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();
    let find = |name: &str| headers.iter().position(|h| h == name);

    let date_idx = find("Date").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let model_idx = find("Model").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let input_with_idx = find("Input (w/ Cache Write)").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let input_without_idx = find("Input (w/o Cache Write)").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let cache_read_idx = find("Cache Read").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let output_idx = find("Output Tokens").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let total_idx = find("Total Tokens").ok_or("SOURCE_SCHEMA_MISMATCH")?;
    let cost_idx = find("Cost").ok_or("SOURCE_SCHEMA_MISMATCH")?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        let field = |idx: usize| record.get(idx).unwrap_or("").trim();
        let input_with = parse_num(field(input_with_idx));
        let input_without = parse_num(field(input_without_idx));
        let output = parse_num(field(output_idx));
        let total = parse_num(field(total_idx));
        if total == 0 && input_without == 0 && output == 0 {
            continue;
        }
        rows.push(CursorUsage {
            date: field(date_idx).to_string(),
            model: field(model_idx).to_string(),
            input_tokens: input_without,
            cache_write_tokens: input_with.saturating_sub(input_without),
            cache_read_tokens: parse_num(field(cache_read_idx)),
            output_tokens: output,
            total_tokens: total,
            cost: parse_cost(field(cost_idx)),
        });
    }
    Ok(rows)
}

fn parse_num(value: &str) -> u64 {
    value
        .trim()
        .replace(',', "")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n.floor() as u64)
        .unwrap_or(0)
}

fn parse_cost(value: &str) -> f64 {
    let cleaned = value.trim().replace(['$', ','], "");
    cleaned
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .unwrap_or(0.0)
}

/// Parses the CSV date column, which Cursor has shipped as a full ISO
/// timestamp, a `YYYY-MM-DD` day, and `MM/DD/YYYY` forms across releases.
pub fn parse_csv_date(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim();
    if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(parsed.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y/%m/%d %H:%M:%S"] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, format) {
            return Some(parsed.and_utc());
        }
    }
    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(parsed) = NaiveDate::parse_from_str(trimmed, format) {
            return parsed.and_hms_opt(0, 0, 0).map(|d| d.and_utc());
        }
    }
    for format in ["%m/%d/%Y", "%m-%d-%Y"] {
        if let Ok(parsed) = NaiveDate::parse_from_str(trimmed, format) {
            return parsed.and_hms_opt(0, 0, 0).map(|d| d.and_utc());
        }
    }
    None
}

/// Deterministic FNV-1a fingerprint of one CSV row, so re-fetching the same
/// export is recognized as duplicate instead of counted again. The cost is
/// part of the fingerprint because two identical requests in one day would
/// otherwise collide.
fn row_fingerprint(row: &CursorUsage) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in format!(
        "{PROVIDER_ID}|{}|{}|{}|{}|{:.4}",
        row.date, row.model, row.input_tokens, row.output_tokens, row.cost
    )
    .as_bytes()
    {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{PROVIDER_ID}_{hash:016x}")
}

/// Maps parsed CSV rows to normalized usage events. Rows whose date does not
/// parse are skipped rather than guessed.
pub fn events_from_rows(rows: &[CursorUsage]) -> Vec<UsageEvent> {
    rows.iter()
        .filter_map(|row| {
            let timestamp = parse_csv_date(&row.date)?;
            Some(UsageEvent {
                id: row_fingerprint(row),
                timestamp,
                provider_id: PROVIDER_ID.to_string(),
                model: if row.model.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    row.model.clone()
                },
                tokens_input: row.input_tokens,
                tokens_output: row.output_tokens,
                confidence: Confidence::High,
                data_source: DATA_SOURCE.to_string(),
                cost: format!("{:.4}", row.cost),
            })
        })
        .collect()
}

/// Converts the `/api/usage-summary` payload into quota windows.
///
/// Cursor publishes per-lane utilization percentages plus the billing cycle
/// end; absolute plan limits are not published per window, so every window is
/// percent-only (`used` stays zero, nothing is invented).
pub fn windows_from_summary(body: &Value) -> Result<Vec<QuotaWindow>, String> {
    let individual = body
        .get("individualUsage")
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let plan = individual.get("plan");
    let on_demand = individual.get("onDemand");
    let team_on_demand = body.get("teamUsage").and_then(|t| t.get("onDemand"));
    let billing_cycle_end = body
        .get("billingCycleEnd")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc));

    let pct = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_f64())
            .filter(|n| n.is_finite() && *n >= 0.0)
    };
    let cents_pct = |used: Option<f64>, limit: Option<f64>| match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some((used / limit * 100.0).min(100.0)),
        _ => None,
    };

    let auto = plan
        .and_then(|p| pct(p, "autoPercentUsed"))
        .map(|n| n.min(100.0));
    let api = plan
        .and_then(|p| pct(p, "apiPercentUsed"))
        .map(|n| n.min(100.0));
    let plan_percent = plan
        .and_then(|p| pct(p, "totalPercentUsed"))
        .map(|n| n.min(100.0))
        .or_else(|| match (auto, api) {
            (Some(a), Some(b)) => Some((a + b) / 2.0),
            (None, Some(b)) => Some(b),
            (Some(a), None) => Some(a),
            (None, None) => None,
        });
    let plan_percent = plan_percent.or_else(|| {
        cents_pct(
            plan.and_then(|p| pct(p, "used")),
            plan.and_then(|p| pct(p, "limit")),
        )
    });
    let plan_percent = plan_percent.or_else(|| {
        cents_pct(
            on_demand.and_then(|o| pct(o, "used")),
            on_demand.and_then(|o| pct(o, "limit")),
        )
    });
    let plan_percent = plan_percent.or_else(|| {
        cents_pct(
            team_on_demand.and_then(|o| pct(o, "used")),
            team_on_demand.and_then(|o| pct(o, "limit")),
        )
    });

    let plan_percent = match plan_percent {
        Some(p) => p,
        None => return Err("SOURCE_SCHEMA_MISMATCH".to_string()),
    };

    let mut windows = vec![QuotaWindow::from_percent(
        "plan",
        "Plan",
        QuotaWindowScope::Monthly,
        QuotaKind::Credits,
        plan_percent,
        billing_cycle_end,
        Confidence::High,
    )];
    if let Some(p) = auto {
        windows.push(QuotaWindow::from_percent(
            "auto",
            "Auto",
            QuotaWindowScope::Other,
            QuotaKind::Credits,
            p,
            billing_cycle_end,
            Confidence::High,
        ));
    }
    if let Some(p) = api {
        windows.push(QuotaWindow::from_percent(
            "api",
            "API",
            QuotaWindowScope::Other,
            QuotaKind::Credits,
            p,
            billing_cycle_end,
            Confidence::High,
        ));
    }
    Ok(windows)
}

impl ProviderAdapter for CursorAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Cursor",
            vendor: "Anysphere",
            source_kind: SourceKind::RemoteApi,
            usage_support: ChannelSupport::Native,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let rows = self.fetch_usage_rows()?;
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", Utc::now().timestamp()),
            events: events_from_rows(&rows),
        })
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        if !self.state_db.is_file() {
            return Ok(None);
        }
        let cookie = self.session_cookie()?;
        let response = lnwdeck_provider_http::get_json(JsonRequest {
            url: &self.quota_url,
            bearer_token: None,
            timeout: self.timeout,
            capture_headers: &[],
            extra_headers: &[
                ("cookie", &cookie),
                ("referer", "https://www.cursor.com/settings"),
                ("user-agent", USER_AGENT),
            ],
        })?;
        let windows = windows_from_summary(&response.body)?;
        if windows.is_empty() {
            return Err("SOURCE_SCHEMA_MISMATCH".to_string());
        }
        let mut report = QuotaReport::new(PROVIDER_ID, "provider_api", windows, DEFAULT_FRESHNESS);
        if let Some(membership) = response.body.get("membershipType").and_then(|v| v.as_str()) {
            report.plan = Some(membership.to_string());
        }
        Ok(Some(report))
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Cursor state store not found".to_string(),
            };
        }
        if !detection.detected {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Cursor session credential not found".to_string(),
            };
        }
        match self.collect_quota() {
            Ok(Some(_)) => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Cursor account API reachable".to_string(),
            },
            Ok(None) => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Cursor account API returned no data".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("Cursor account API failed ({code})"),
            },
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(self.detection())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn make_state_db(path: &std::path::Path, token: Option<&str>) {
        let conn = Connection::open(path).expect("create db");
        conn.execute(
            "CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value BLOB)",
            [],
        )
        .expect("create table");
        if let Some(token) = token {
            conn.execute(
                "INSERT INTO ItemTable (key, value) VALUES ('cursorAuth/accessToken', ?1)",
                [token],
            )
            .expect("insert token");
        }
        drop(conn);
    }

    fn sample_jwt(sub: &str) -> String {
        use base64::Engine;
        let header =
            base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE
            .encode(format!(r#"{{"sub":"{sub}","exp":1999999999}}"#));
        format!("{header}.{payload}.signature")
    }

    #[test]
    fn descriptor_declares_native_api_support() {
        let adapter = CursorAdapter::with_urls(
            PathBuf::from("Z:/missing.vscdb"),
            PathBuf::from("Z:/missing.json"),
            "https://example.invalid/usage",
            "https://example.invalid/quota",
        );
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.id, "cursor_ide");
        assert_eq!(descriptor.usage_support, ChannelSupport::Native);
        assert_eq!(descriptor.quota_support, ChannelSupport::Native);
        assert_eq!(descriptor.source_kind, SourceKind::RemoteApi);
        assert!(!descriptor.is_inert());
        assert!(
            !descriptor.needs_credentials(),
            "Cursor reuses the credential its own tooling stores locally"
        );
    }

    #[test]
    fn missing_state_store_is_reported_not_faked() {
        let adapter = CursorAdapter::with_urls(
            PathBuf::from("Z:/missing.vscdb"),
            PathBuf::from("Z:/missing.json"),
            "https://example.invalid/usage",
            "https://example.invalid/quota",
        );
        assert_eq!(
            adapter.collect_usage().expect_err("must fail"),
            "SOURCE_UNAVAILABLE"
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
        let detection = adapter.detect().expect("detect");
        assert!(!detection.detected);
        assert_eq!(detection.permission_state, "not_found");
    }

    #[test]
    fn missing_session_token_is_not_configured() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        make_state_db(&db_path, None);
        let adapter = CursorAdapter::with_urls(
            db_path.clone(),
            dir.path().join("missing.json"),
            "https://example.invalid/usage",
            "https://example.invalid/quota",
        );
        assert_eq!(
            adapter.collect_usage().expect_err("must fail"),
            "NOT_CONFIGURED"
        );
        let detection = adapter.detect().expect("detect");
        assert!(detection.source_exists);
        assert!(!detection.detected);
        assert_eq!(detection.permission_state, "permission_required");
        assert_eq!(detection.detection_error_code, "NOT_CONFIGURED");
    }

    #[test]
    fn session_cookie_uses_cli_config_user_id_when_present() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        make_state_db(&db_path, Some("jwt-value-123"));
        let cli_config = dir.path().join("cli-config.json");
        std::fs::write(
            &cli_config,
            r#"{"authInfo":{"authId":"auth0|user_abc123"},"other":1}"#,
        )
        .expect("write cli config");
        let adapter = CursorAdapter::with_urls(
            db_path,
            cli_config,
            "https://example.invalid/usage",
            "https://example.invalid/quota",
        );
        assert_eq!(
            adapter.session_cookie().expect("cookie"),
            "WorkosCursorSessionToken=user_abc123%3A%3Ajwt-value-123"
        );
    }

    #[test]
    fn session_cookie_falls_back_to_jwt_sub_claim() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        let jwt = sample_jwt("google-oauth2|user_01KKTZPKKQFRZJM6BWT8RCT76M");
        make_state_db(&db_path, Some(&jwt));
        let adapter = CursorAdapter::with_urls(
            db_path,
            dir.path().join("missing.json"),
            "https://example.invalid/usage",
            "https://example.invalid/quota",
        );
        let cookie = adapter.session_cookie().expect("cookie");
        assert!(
            cookie.starts_with(
                "WorkosCursorSessionToken=google-oauth2|user_01KKTZPKKQFRZJM6BWT8RCT76M%3A%3A"
            ),
            "cookie: {cookie}"
        );
        assert!(cookie.ends_with(&jwt));
    }

    #[test]
    fn user_id_normalization() {
        assert_eq!(
            normalize_user_id("auth0|user_abc123"),
            Some("user_abc123".to_string())
        );
        assert_eq!(
            normalize_user_id("google-oauth2|123456"),
            Some("google-oauth2|123456".to_string())
        );
        assert_eq!(
            normalize_user_id("github|octo"),
            Some("github|octo".to_string())
        );
        assert_eq!(
            normalize_user_id("auth0|user_1"),
            Some("user_1".to_string())
        );
        assert_eq!(normalize_user_id(""), None);
        assert_eq!(normalize_user_id("auth0|not-user"), None);
        assert_eq!(normalize_user_id("plain"), None);
    }

    #[test]
    fn csv_rows_are_parsed_by_header_name_regardless_of_order() {
        let csv = concat!(
            "Kind,Cost,Date,Input (w/o Cache Write),Model,Output Tokens,Input (w/ Cache Write),Cache Read,Total Tokens,Max Mode\n",
            "chat,\"$1.23\",2026-05-24,100,cursor-fast,20,120,5,125,No\n",
            "chat,0.00,2026-05-25,200,cursor-fast,40,200,10,240,No\n",
            "chat,0.00,2026-05-26,0,cursor-fast,0,0,0,0,No\n",
        );
        let rows = parse_usage_csv(csv).expect("parse");
        assert_eq!(rows.len(), 2, "all-zero row is dropped");
        assert_eq!(rows[0].model, "cursor-fast");
        assert_eq!(rows[0].input_tokens, 100);
        assert_eq!(rows[0].cache_write_tokens, 20, "with-minus-without");
        assert_eq!(rows[0].cache_read_tokens, 5);
        assert_eq!(rows[0].output_tokens, 20);
        assert_eq!(rows[0].total_tokens, 125);
        assert!((rows[0].cost - 1.23).abs() < 1e-9, "cost {}", rows[0].cost);
    }

    #[test]
    fn csv_without_required_columns_is_a_schema_mismatch() {
        let csv = "Date,Model,Something\n2026-05-24,cursor-fast,1\n";
        assert_eq!(
            parse_usage_csv(csv).expect_err("must fail"),
            "SOURCE_SCHEMA_MISMATCH"
        );
    }

    #[test]
    fn csv_dates_parse_across_shipped_formats() {
        assert_eq!(
            parse_csv_date("2026-05-24").expect("iso day").to_rfc3339(),
            "2026-05-24T00:00:00+00:00"
        );
        assert!(parse_csv_date("2026-05-24T06:56:28.714Z").is_some());
        assert!(parse_csv_date("2026-05-24 06:56:28").is_some());
        assert!(parse_csv_date("05/24/2026").is_some());
        assert!(parse_csv_date("not a date").is_none());
    }

    #[test]
    fn events_are_derived_from_rows_with_stable_ids() {
        let rows = vec![CursorUsage {
            date: "2026-05-24".to_string(),
            model: "cursor-fast".to_string(),
            input_tokens: 100,
            cache_write_tokens: 20,
            cache_read_tokens: 5,
            output_tokens: 20,
            total_tokens: 125,
            cost: 1.23,
        }];
        let events = events_from_rows(&rows);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].provider_id, "cursor_ide");
        assert_eq!(events[0].model, "cursor-fast");
        assert_eq!(events[0].tokens_input, 100);
        assert_eq!(events[0].tokens_output, 20);
        assert_eq!(events[0].cost, "1.2300");
        assert_eq!(events_from_rows(&rows)[0].id, events[0].id, "stable ids");
    }

    #[test]
    fn quota_summary_builds_percent_windows_from_lanes() {
        let body = serde_json::json!({
            "individualUsage": {
                "plan": {
                    "totalPercentUsed": 42.0,
                    "autoPercentUsed": 30.0,
                    "apiPercentUsed": 12.0,
                    "used": 8400,
                    "limit": 20000
                },
                "onDemand": { "used": 5, "limit": 100 }
            },
            "teamUsage": { "onDemand": { "used": 0, "limit": 100 } },
            "billingCycleEnd": "2026-08-31T23:59:59Z",
            "membershipType": "individual"
        });
        let windows = windows_from_summary(&body).expect("windows");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window_key, "plan");
        assert_eq!(windows[0].used_percent, Some(42.0));
        assert_eq!(windows[0].remaining_percent, Some(58.0));
        assert_eq!(windows[0].kind, QuotaKind::Credits);
        assert_eq!(windows[0].limit, None, "no invented limit");
        assert_eq!(windows[0].used, 0, "no invented count");
        assert!(windows[0]
            .reset_at
            .expect("reset")
            .to_rfc3339()
            .starts_with("2026-08-31"));
        assert_eq!(windows[1].window_key, "auto");
        assert_eq!(windows[1].used_percent, Some(30.0));
        assert_eq!(windows[2].window_key, "api");
        assert_eq!(windows[2].used_percent, Some(12.0));
        for window in &windows {
            window.check_invariants().expect("consistent window");
        }
    }

    #[test]
    fn quota_summary_falls_back_to_cents_and_on_demand() {
        let cents_only = serde_json::json!({
            "individualUsage": {
                "plan": { "used": 1000, "limit": 4000 },
                "onDemand": { "used": 0, "limit": 100 }
            },
            "billingCycleEnd": "2026-08-31T23:59:59Z"
        });
        let windows = windows_from_summary(&cents_only).expect("windows");
        assert_eq!(windows[0].used_percent, Some(25.0));

        let on_demand_only = serde_json::json!({
            "individualUsage": { "onDemand": { "used": 30, "limit": 300 } }
        });
        let windows = windows_from_summary(&on_demand_only).expect("windows");
        assert_eq!(windows[0].used_percent, Some(10.0));
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn quota_summary_without_any_usage_is_a_schema_mismatch() {
        let body = serde_json::json!({ "individualUsage": { "plan": {} } });
        assert_eq!(
            windows_from_summary(&body).expect_err("must fail"),
            "SOURCE_SCHEMA_MISMATCH"
        );
        assert!(windows_from_summary(&serde_json::json!({})).is_err());
    }

    #[test]
    fn network_failures_reduce_to_sanitized_codes_without_leaking_the_cookie() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        make_state_db(&db_path, Some("secret-jwt-value-xyz"));
        let cli_config = dir.path().join("cli-config.json");
        std::fs::write(
            &cli_config,
            r#"{"authInfo":{"authId":"auth0|user_abc123"}}"#,
        )
        .expect("cli config");
        let adapter = CursorAdapter::with_urls(
            db_path,
            cli_config,
            "https://lnwdeck-nonexistent-host.invalid/usage",
            "https://lnwdeck-nonexistent-host.invalid/quota",
        );
        let error = adapter.collect_usage().expect_err("must fail");
        assert!(
            error == "SOURCE_UNAVAILABLE" || error == "PROVIDER_TIMEOUT",
            "sanitized code: {error}"
        );
        assert!(
            !error.contains("secret-jwt-value-xyz"),
            "the credential must never leak into an error"
        );
        assert!(adapter.health_check().status != AdapterHealthStatus::Healthy);
    }

    #[test]
    fn required_permissions_cover_filesystem_and_network() {
        let adapter = CursorAdapter::new();
        assert!(adapter
            .required_permissions()
            .contains(&Permission::FileSystem));
        assert!(adapter
            .required_permissions()
            .contains(&Permission::Network));
    }
}
