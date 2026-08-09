//! ZCode (Z.AI) adapter.
//!
//! ZCode is Z.ai's coding agent. It keeps an OpenCode-fork SQLite store at
//! `~/.zcode/cli/db/db.sqlite` whose `message` table records every assistant
//! turn. Only turns that belong to ZCode itself are counted: bundled
//! Claude/Codex/Gemini sub-agents (providerID `anthropic` / `openai` /
//! `google`) are excluded because the dedicated adapters already count them.
//!
//! Quota is the real GLM Coding Plan quota:
//!
//! - When `~/.zcode/v2/config.json` holds a plaintext API key for the
//!   built-in coding-plan provider, the Z.AI (or BigModel) monitor API
//!   (`/api/monitor/usage/quota/limit`) publishes the used percentage and
//!   reset time of the 5-hour, weekly and tool-call windows.
//! - Otherwise the local ZCode logs (`~/.zcode/v2/logs/*.log`) carry the
//!   `billing/balance` records ZCode itself fetched, with real credit totals,
//!   used and remaining amounts plus the period end.
//! - When neither is available the adapter reports no quota. Token totals from
//!   the local message store remain available through the separate usage
//!   channel and are never presented as a subscription limit.
//!
//! The API key is read from ZCode's own config file and sent only over HTTPS
//! to the declared Z.AI / BigModel endpoints; it never appears in errors or
//! diagnostics.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, JsonRequest};
use lnwdeck_provider_runtime::opencode_fork::{self, MessageSample};
use lnwdeck_provider_runtime::token_scan::{usage_events, ScanBounds};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

const PROVIDER_ID: &str = "zcode_ai";
const ADAPTER_VERSION: &str = "0.1.0";

const ZAI_MONITOR_QUOTA_URL: &str = "https://api.z.ai/api/monitor/usage/quota/limit";
const BIGMODEL_MONITOR_QUOTA_URL: &str = "https://bigmodel.cn/api/monitor/usage/quota/limit";

/// ZCode config entries, tried in order. Coding-plan entries expose the
/// monitor quota API; start-plan entries only publish balances in logs.
const CONFIG_CANDIDATES: &[&str] = &[
    "builtin:zai-coding-plan",
    "builtin:bigmodel-coding-plan",
    "builtin:zai-start-plan",
    "builtin:bigmodel-start-plan",
];

/// Provider ids of bundled sub-agents whose turns are counted by their own
/// adapters and must not be double-counted here.
const EXCLUDED_PROVIDERS: &[&str] = &["anthropic", "openai", "google"];

struct MonitorCredential {
    endpoint: &'static str,
    key: String,
}

pub struct ZCodeAdapter {
    home: PathBuf,
    timeout: Duration,
}

impl Default for ZCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZCodeAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".zcode");
        Self::with_home(home)
    }

    /// Adapter pinned to an explicit ZCode home (used by tests).
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            home,
            timeout: Duration::from_secs(10),
        }
    }

    fn db_path(&self) -> PathBuf {
        self.home.join("cli").join("db").join("db.sqlite")
    }

    fn config_path(&self) -> PathBuf {
        self.home.join("v2").join("config.json")
    }

    fn logs_dir(&self) -> PathBuf {
        self.home.join("v2").join("logs")
    }

    /// Reads the first usable coding-plan API key from ZCode's config.
    ///
    /// Encrypted keys (`enc:v1:`) are not supported and count as absent, so
    /// the adapter falls back to the local log balance instead of attempting a
    /// doomed request. The endpoint is selected from the config entry so a
    /// BigModel credential is never sent to the Z.AI endpoint.
    fn api_key(&self) -> Option<MonitorCredential> {
        let raw = std::fs::read_to_string(self.config_path()).ok()?;
        let config: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let providers = config.get("provider")?;
        for candidate in CONFIG_CANDIDATES {
            let Some(entry) = providers.get(*candidate) else {
                continue;
            };
            let Some(key) = entry
                .get("options")
                .and_then(|options| options.get("apiKey"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() || key.starts_with("enc:v1:") || !candidate.ends_with("coding-plan") {
                continue;
            }
            let endpoint = if candidate.starts_with("builtin:zai-") {
                ZAI_MONITOR_QUOTA_URL
            } else {
                BIGMODEL_MONITOR_QUOTA_URL
            };
            return Some(MonitorCredential {
                endpoint,
                key: key.to_string(),
            });
        }
        None
    }

    /// True when the provider id belongs to a bundled sub-agent that the
    /// dedicated adapters already count.
    fn is_excluded_provider(provider: Option<&str>) -> bool {
        let Some(provider) = provider else {
            return true;
        };
        EXCLUDED_PROVIDERS
            .iter()
            .any(|excluded| provider.to_lowercase().contains(excluded))
    }

    fn samples(&self) -> Result<Vec<MessageSample>, String> {
        let bounds = ScanBounds::default();
        let all = opencode_fork::read_messages(&self.db_path(), &bounds)?;
        Ok(all
            .into_iter()
            .filter(|sample| !Self::is_excluded_provider(sample.provider_id.as_deref()))
            .collect())
    }

    /// Fetches the coding-plan quota from the monitor API.
    ///
    /// `Ok(None)` means the payload carried no usable windows; an expired
    /// credential or a rate limit is reported so the UI does not mask a real
    /// signal behind a local estimate.
    fn monitor_quota(&self, endpoint: &str, key: &str) -> Result<Option<QuotaReport>, String> {
        let response = get_json(JsonRequest {
            timeout: self.timeout,
            ..JsonRequest::new(endpoint).raw_auth(key)
        })?;
        let Some((windows, plan)) = windows_from_monitor_payload(&response.body)? else {
            return Ok(None);
        };
        let mut report = QuotaReport::new(PROVIDER_ID, "provider_api", windows, DEFAULT_FRESHNESS);
        report.plan = plan;
        Ok(Some(report))
    }

    /// Reads the most recent `billing/balance` records ZCode wrote to its own
    /// logs and turns them into credit windows with real limits.
    fn balance_from_logs(&self) -> Result<Option<QuotaReport>, String> {
        let logs_dir = self.logs_dir();
        let mut windows = Vec::new();
        for offset_days in [0i64, -1] {
            let date = chrono::Local::now() + chrono::Duration::days(offset_days);
            let path = logs_dir.join(format!("{}.log", date.format("%Y-%m-%d")));
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            windows.extend(balance_windows_from_log(&content));
        }
        if windows.is_empty() {
            return Ok(None);
        }
        windows.sort_by_key(|window| window.limit.unwrap_or(0));
        windows.reverse();
        windows.truncate(3);
        Ok(Some(QuotaReport::new(
            PROVIDER_ID,
            "local_log",
            windows,
            DEFAULT_FRESHNESS,
        )))
    }

    /// Quota with the best evidence first: monitor API, then the local log
    /// balance. Local token totals are usage history, not quota evidence.
    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        if let Some(credential) = self.api_key() {
            match self.monitor_quota(credential.endpoint, &credential.key) {
                Ok(Some(report)) => return Ok(Some(report)),
                Ok(None) => {}
                Err(code) if code == "AUTH_EXPIRED" || code == "RATE_LIMITED" => return Err(code),
                Err(_) => {}
            }
        }
        if let Some(report) = self.balance_from_logs()? {
            return Ok(Some(report));
        }
        Ok(None)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.db_path().is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "ZCode".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_sqlite".to_string(),
            source_type: "sqlite".to_string(),
            source_exists,
            permission_state: "n/a".to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(chrono::Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        };
        if !source_exists {
            result.permission_state = "not_found".to_string();
            return result;
        }
        match self.samples() {
            Ok(samples) if !samples.is_empty() => {
                result.detected = true;
                result.permission_state = "read_ok".to_string();
            }
            Ok(_) => result.permission_state = "no_sessions".to_string(),
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
            }
        }
        result
    }
}

/// Quota windows parsed from a monitor payload, with an optional plan label.
pub type MonitorWindows = (Vec<QuotaWindow>, Option<String>);

/// Parses the monitor API payload into quota windows plus an optional plan
/// label. Returns `Ok(None)` when the payload has no usable window.
pub fn windows_from_monitor_payload(
    payload: &serde_json::Value,
) -> Result<Option<MonitorWindows>, String> {
    let code = payload
        .get("code")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    if code != 0 && code != 200 {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    if payload.get("success").and_then(|value| value.as_bool()) == Some(false) {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    let data = payload
        .get("data")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let limits = data
        .get("limits")
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let named = [
        window_from_limit(
            find_limit(limits, "TOKENS_LIMIT", 3, Some(5)),
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Credits,
        ),
        window_from_limit(
            find_limit(limits, "TOKENS_LIMIT", 6, None),
            "7d",
            "Weekly",
            QuotaWindowScope::Weekly,
            QuotaKind::Credits,
        ),
        window_from_limit(
            find_limit(limits, "TIME_LIMIT", 5, Some(1)),
            "tools",
            "Tool calls",
            QuotaWindowScope::Other,
            QuotaKind::Requests,
        ),
    ];
    let named: Vec<QuotaWindow> = named.into_iter().flatten().collect();
    let windows = if named.is_empty() {
        limits
            .iter()
            .filter_map(|limit| {
                window_from_limit(
                    Some(limit),
                    "quota",
                    "Quota",
                    QuotaWindowScope::Other,
                    QuotaKind::Credits,
                )
            })
            .collect()
    } else {
        named
    };
    if windows.is_empty() {
        return Ok(None);
    }
    let plan = data
        .get("level")
        .and_then(|value| value.as_str())
        .map(plan_label)
        .or_else(|| Some("Coding".to_string()));
    Ok(Some((windows, plan)))
}

fn find_limit<'a>(
    limits: &'a [serde_json::Value],
    limit_type: &str,
    unit: i64,
    number: Option<i64>,
) -> Option<&'a serde_json::Value> {
    limits.iter().find(|limit| {
        limit.get("type").and_then(|value| value.as_str()) == Some(limit_type)
            && limit.get("unit").and_then(|value| value.as_i64()) == Some(unit)
            && (number.is_none() || limit.get("number").and_then(|value| value.as_i64()) == number)
    })
}

fn as_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    value?.as_f64().filter(|number| number.is_finite())
}

/// Builds one percent-only window from a quota limit row. The published
/// `percentage` is authoritative; otherwise it is derived from
/// usage/remaining against the total. A row with no percentage and no usable
/// totals produces no window.
fn window_from_limit(
    limit: Option<&serde_json::Value>,
    key: &str,
    label: &str,
    scope: QuotaWindowScope,
    kind: QuotaKind,
) -> Option<QuotaWindow> {
    let limit = limit?;
    let total = as_f64(limit.get("number"));
    let used = as_f64(limit.get("usage")).or_else(|| as_f64(limit.get("currentValue")));
    let remaining = as_f64(limit.get("remaining"));
    let raw_percent = as_f64(limit.get("percentage"));
    let percent = raw_percent.or_else(|| match (total, used) {
        (Some(total), Some(used)) if total > 0.0 => Some(used / total * 100.0),
        _ => match (total, remaining) {
            (Some(total), Some(remaining)) if total > 0.0 && remaining <= total => {
                Some((total - remaining) / total * 100.0)
            }
            _ => None,
        },
    })?;
    Some(QuotaWindow::from_percent(
        key,
        label,
        scope,
        kind,
        percent,
        parse_reset(limit.get("nextResetTime")),
        Confidence::High,
    ))
}

fn parse_reset(value: Option<&serde_json::Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    match value? {
        serde_json::Value::String(text) => chrono::DateTime::parse_from_rfc3339(text.trim())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok(),
        serde_json::Value::Number(number) => {
            let value = number.as_i64()?;
            if value <= 0 {
                return None;
            }
            if value > 10_000_000_000 {
                chrono::DateTime::from_timestamp_millis(value)
            } else {
                chrono::DateTime::from_timestamp(value, 0)
            }
        }
        _ => None,
    }
}

/// Human label from a plan level id (`zcode-v3-start-plan-0615` -> `Start`).
fn plan_label(level: &str) -> String {
    let lower = level.to_lowercase();
    for tier in ["start", "lite", "pro", "max", "team", "enterprise"] {
        if lower.contains(tier) {
            let mut chars = tier.chars();
            return chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_else(|| tier.to_string());
        }
    }
    level.to_string()
}

/// Extracts credit windows from ZCode's local `billing/balance` log records.
///
/// A record is a line containing `[usage-stats] billing/balance` followed by
/// a JSON object whose `payload.data.balances` array carries
/// `total_units` / `used_units` / `remaining_units` / `period_end`.
pub fn balance_windows_from_log(content: &str) -> Vec<QuotaWindow> {
    let mut windows = Vec::new();
    for line in content.lines() {
        if !line.contains("[usage-stats] billing/balance") {
            continue;
        }
        let Some(json_start) = line.find('{') else {
            continue;
        };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line[json_start..]) else {
            continue;
        };
        if entry.get("success").and_then(|value| value.as_bool()) != Some(true) {
            continue;
        }
        let Some(balances) = entry
            .get("payload")
            .and_then(|payload| payload.get("data"))
            .and_then(|data| data.get("balances"))
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for balance in balances {
            let Some(total) = as_f64(balance.get("total_units")) else {
                continue;
            };
            let Some(used) = as_f64(balance.get("used_units")) else {
                continue;
            };
            let Some(total_units) = NonZeroU64::new(total.round().max(0.0) as u64) else {
                continue;
            };
            let used_units = used.round().max(0.0) as u64;
            let label = balance
                .get("show_name")
                .and_then(|value| value.as_str())
                .filter(|label| !label.is_empty())
                .unwrap_or("Credits")
                .to_string();
            let reset = balance
                .get("period_end")
                .or_else(|| balance.get("expires_at"))
                .and_then(|value| value.as_i64())
                .and_then(|seconds| {
                    if seconds > 0 {
                        chrono::DateTime::from_timestamp(seconds, 0)
                    } else {
                        None
                    }
                });
            windows.push(QuotaWindow::with_limit(
                &label,
                &label,
                QuotaWindowScope::Other,
                QuotaKind::Credits,
                used_units,
                total_units,
                reset,
                Confidence::High,
            ));
        }
    }
    windows
}

impl ProviderAdapter for ZCodeAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "ZCode",
            vendor: "Z.AI",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let samples = self.samples()?;
        if samples.is_empty() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", chrono::Utc::now().timestamp()),
            events: usage_events(
                PROVIDER_ID,
                "local_sqlite",
                &opencode_fork::to_token_samples(&samples),
                Confidence::Medium,
            ),
        })
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }

    fn account_identity(&self) -> Option<String> {
        self.api_key().map(|credential| credential.key)
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "ZCode local data not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "ZCode local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "ZCode local data has no GLM token records".to_string(),
            }
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
    use std::path::Path;

    fn write_db(home: &Path) {
        let db_dir = home.join("cli").join("db");
        std::fs::create_dir_all(&db_dir).expect("db dir");
        let conn = Connection::open(db_dir.join("db.sqlite")).expect("open");
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY, session_id TEXT,
                time_created INTEGER, time_updated INTEGER, data TEXT
             );",
        )
        .expect("schema");
        let message = |id: &str, provider: &str, model: &str, input: u64, output: u64| {
            format!(
                r#"{{"id":"{id}","providerID":"{provider}","modelID":"{model}","role":"assistant","tokens":{{"input":{input},"output":{output},"reasoning":0}},"time":{{"created":1700000000000}}}}"#
            )
        };
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES ('z1','s1',1700000000000,1700000000000,?1),
                    ('z2','s1',1700000000000,1700000001000,?2),
                    ('c1','s1',1700000000000,1700000002000,?3)",
            rusqlite::params![
                message("z1", "builtin:zai-coding-plan", "glm-5.2", 100, 50),
                message("z2", "builtin:zai-start-plan", "glm-4.7", 10, 10),
                message("c1", "anthropic", "claude-3-7-sonnet", 999, 999),
            ],
        )
        .expect("rows");
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(ZCodeAdapter::new().id(), "zcode_ai");
    }

    #[test]
    fn counts_only_zcode_turns() {
        let home = tempfile::tempdir().expect("temp");
        write_db(home.path());
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let samples = adapter.samples().expect("samples");
        assert_eq!(samples.len(), 2, "anthropic sub-agent turns are excluded");
        assert!(
            samples.iter().all(|sample| {
                sample
                    .model
                    .as_deref()
                    .map(|model| model.starts_with("glm-"))
                    .unwrap_or(false)
            }),
            "only GLM turns survive: {samples:?}"
        );
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert!(batch.events.iter().all(|e| e.provider_id == PROVIDER_ID));
    }

    #[test]
    fn local_usage_is_not_presented_as_quota() {
        let home = tempfile::tempdir().expect("temp");
        write_db(home.path());
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());

        assert!(
            adapter.collect_quota().expect("quota").is_none(),
            "local token totals do not prove a subscription quota"
        );
    }

    #[test]
    fn missing_home_reports_no_data() {
        let home = tempfile::tempdir().expect("temp");
        let adapter = ZCodeAdapter::with_home(home.path().join("missing"));
        assert_eq!(adapter.samples(), Err("SOURCE_UNAVAILABLE".to_string()));
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }

    #[test]
    fn monitor_payload_builds_named_windows() {
        let payload = serde_json::json!({
            "code": 0,
            "data": {
                "level": "zcode-v3-pro-plan-202608",
                "limits": [
                    {"type":"TOKENS_LIMIT","unit":3,"number":5,"usage":40,"percentage":66.7,"nextResetTime":1780000000,"usageDetails":[{"displayName":"5h","modelCode":"glm-5.2"}]},
                    {"type":"TOKENS_LIMIT","unit":6,"usage":300,"percentage":10.5,"nextResetTime":"2026-08-15T00:00:00Z"},
                    {"type":"TIME_LIMIT","unit":5,"number":1,"usage":1,"percentage":0.0}
                ]
            }
        });
        let (windows, plan) = windows_from_monitor_payload(&payload)
            .expect("windows")
            .expect("non-empty");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window_key, "5h");
        assert_eq!(windows[0].used_percent, Some(66.7));
        assert_eq!(windows[0].remaining_percent, Some(33.3));
        assert!(windows[0].reset_at.is_some());
        assert_eq!(windows[1].window_key, "7d");
        assert_eq!(windows[1].remaining_percent, Some(89.5));
        assert_eq!(windows[2].window_key, "tools");
        assert_eq!(windows[2].remaining_percent, Some(100.0));
        assert_eq!(plan.as_deref(), Some("Pro"));
        for window in &windows {
            window.check_invariants().expect("consistent window");
        }
    }

    #[test]
    fn monitor_payload_without_named_windows_falls_back_to_all_limits() {
        let payload = serde_json::json!({
            "code": 0,
            "data": { "limits": [
                {"type":"TOKENS_LIMIT","unit":9,"number":100,"usage":25}
            ]}
        });
        let (windows, _) = windows_from_monitor_payload(&payload)
            .expect("windows")
            .expect("non-empty");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, Some(25.0));
    }

    #[test]
    fn monitor_payload_errors_are_reported() {
        for payload in [
            serde_json::json!({"code": 400}),
            serde_json::json!({"success": false}),
        ] {
            assert_eq!(
                windows_from_monitor_payload(&payload),
                Err("SOURCE_SCHEMA_MISMATCH".to_string()),
                "payload must be rejected: {payload}"
            );
        }
        for payload in [
            serde_json::json!({"data": {}}),
            serde_json::json!({"code": 0, "data": {"limits": []}}),
        ] {
            assert_eq!(
                windows_from_monitor_payload(&payload).expect("parses"),
                None,
                "no windows must be reported, not guessed: {payload}"
            );
        }
    }

    #[test]
    fn balance_log_records_become_credit_windows() {
        let line = format!(
            "[2026-08-08 10:00:00.123] [usage-stats] billing/balance ok {}",
            serde_json::json!({
                "success": true, "code": 0, "providerId": "builtin:zai-start-plan",
                "payload": {"data": {"balances": [
                    {"show_name": "GLM-5.2", "total_units": 100, "used_units": 40,
                     "remaining_units": 60, "period_end": 1780000000}
                ]}}
            })
        );
        let windows = balance_windows_from_log(&line);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used, 40);
        assert_eq!(windows[0].limit, Some(100));
        assert_eq!(windows[0].remaining, Some(60));
        assert!(windows[0].reset_at.is_some());
        windows[0].check_invariants().expect("consistent window");
        assert!(balance_windows_from_log("no records here").is_empty());
    }

    #[test]
    fn api_key_prefers_coding_plan_entries() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:zai-start-plan": {"options": {"apiKey": "sk-start"}},
                    "builtin:zai-coding-plan": {"options": {"apiKey": "sk-coding"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let credential = adapter.api_key().expect("key");
        assert_eq!(credential.endpoint, ZAI_MONITOR_QUOTA_URL);
        assert_eq!(credential.key, "sk-coding");
    }

    #[test]
    fn api_key_selects_bigmodel_endpoint_when_zai_entry_is_absent() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:bigmodel-coding-plan": {"options": {"apiKey": "sk-bigmodel"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let credential = adapter.api_key().expect("key");
        assert_eq!(credential.endpoint, BIGMODEL_MONITOR_QUOTA_URL);
        assert_eq!(credential.key, "sk-bigmodel");
    }

    #[test]
    fn encrypted_keys_count_as_absent() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:zai-coding-plan": {"options": {"apiKey": "enc:v1:xyz"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        assert!(adapter.api_key().is_none());
    }

    #[test]
    fn without_any_source_quota_reports_none() {
        let home = tempfile::tempdir().expect("temp");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        assert!(adapter.collect_quota().expect("quota").is_none());
    }

    #[test]
    fn plan_labels_are_human_readable() {
        assert_eq!(plan_label("zcode-v3-start-plan-0615"), "Start");
        assert_eq!(plan_label("zcode-v3-pro-plan-202608"), "Pro");
        assert_eq!(plan_label("something-else"), "something-else");
    }

    #[test]
    fn unsupported_quota_channel_is_never_success() {
        let adapter = ZCodeAdapter::with_home(PathBuf::from("Z:/not/here"));
        let result = adapter.collect_usage_with_cursor(None);
        assert!(result.batch.is_none());
        assert_eq!(result.outcome.error_code, "SOURCE_UNAVAILABLE");
    }
}
