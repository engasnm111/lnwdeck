use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::token_scan::{
    usage_events_with_breakdown, TokenSample, UsageBreakdownSample,
};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub mod usage_api;

const ADAPTER_VERSION: &str = "0.3.0";
const MAX_FILES: usize = 2000;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// Codex CLI passive local collector.
///
/// Reads token usage from the local Codex session JSONL files
/// (`~/.codex/sessions/**/*.jsonl`) read-only. Modern Codex CLI rollouts
/// publish one `event_msg/token_count` per turn with both the session's
/// CUMULATIVE token totals and the per-turn `last_token_usage` breakdown. The
/// adapter uses the per-turn counters for usage and keeps cumulative totals for
/// the rolling quota estimate. Raw transcripts are never normalized.
pub struct CodexAdapter {
    sessions_dir: PathBuf,
    auth_path: PathBuf,
    /// Parsed session files keyed by their (size, mtime) at parse time.
    /// Codex histories grow to gigabytes; re-reading every file on every
    /// refresh saturated the disk/CPU for ~20s and froze the UI. Session
    /// files are append-only, so an unchanged (size, mtime) is a sound
    /// freshness check and the cached records are reused.
    cache: Mutex<HashMap<PathBuf, CachedSession>>,
}

/// A parsed session file plus the metadata it was parsed from.
struct CachedSession {
    size: u64,
    modified: Option<SystemTime>,
    records: Vec<TokenCountRecord>,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::with_paths(home.join(".codex/sessions"), home.join(".codex/auth.json"))
    }

    /// Adapter pinned to explicit local paths (used by tests).
    pub fn with_paths(sessions_dir: PathBuf, auth_path: PathBuf) -> Self {
        Self {
            sessions_dir,
            auth_path,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn has_credentials(&self) -> bool {
        self.auth_path.is_file()
    }

    fn detection(&self) -> DetectionResult {
        let mut result = DetectionResult {
            provider_id: "openai_codex".to_string(),
            display_name: "OpenAI Codex".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_jsonl".to_string(),
            source_type: "jsonl".to_string(),
            source_exists: self.sessions_dir.is_dir(),
            permission_state: "n/a".to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        };
        if !result.source_exists {
            result.permission_state = "not_found".to_string();
            return result;
        }
        match self.scan_session_files() {
            Ok(Some(_)) => {
                result.detected = true;
                result.permission_state = if self.has_credentials() {
                    "read_ok_auth".to_string()
                } else {
                    "read_ok_no_auth".to_string()
                };
                result
            }
            Ok(None) => {
                result.permission_state = "no_sessions".to_string();
                result
            }
            Err(_) => {
                result.detection_error_code = "INVALID_PROVIDER_DATA".to_string();
                result.permission_state = "permission_required".to_string();
                result
            }
        }
    }

    /// Scans the session files and returns the rolling token sums
    /// (5h / 7d / 30d) computed from each session's final cumulative total.
    fn scan_session_files(&self) -> Result<Option<[u64; 3]>, String> {
        let now_ms = Utc::now().timestamp_millis();
        let buckets = [
            5 * 3600 * 1000i64,
            7 * 24 * 3600 * 1000i64,
            30 * 24 * 3600 * 1000i64,
        ];
        let mut sums = [0u64; 3];
        let mut any = false;

        for sample in self.scan_samples()? {
            for (i, bucket_ms) in buckets.iter().enumerate() {
                if now_ms - sample.timestamp.timestamp_millis() <= *bucket_ms {
                    sums[i] = sums[i]
                        .saturating_add(sample.input_tokens)
                        .saturating_add(sample.output_tokens);
                    any = true;
                }
            }
        }

        Ok(any.then_some(sums))
    }

    /// One usage sample per session file: the last cumulative `token_count`
    /// record of that session. Summing every `token_count` line would count
    /// each turn's cumulative total multiple times.
    fn scan_token_records(&self) -> Result<Vec<Vec<TokenCountRecord>>, String> {
        let files = collect_jsonl_files(&self.sessions_dir)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| "codex cache poisoned".to_string())?;

        // Sessions that no longer exist stop contributing. The cached records
        // themselves are kept as parsed history only while the file is live.
        let live: HashSet<&Path> = files.iter().map(|path| path.as_path()).collect();
        cache.retain(|path, _| live.contains(path.as_path()));

        let mut sessions = Vec::new();
        for file in files {
            let Ok(meta) = file.metadata() else {
                continue;
            };
            let size = meta.len();
            if size > MAX_TOTAL_BYTES {
                continue;
            }
            let modified = meta.modified().ok();
            let records = match cache.get(&file) {
                Some(entry) if entry.size == size && entry.modified == modified => {
                    entry.records.clone()
                }
                _ => {
                    let Ok(content) = std::fs::read_to_string(&file) else {
                        continue;
                    };
                    let mut current_model = None;
                    let mut records = Vec::new();
                    for line in content.lines() {
                        if let Some(model) = model_from_metadata_line(line) {
                            current_model = Some(model);
                        }
                        if let Some(mut record) = parse_token_count_line(line) {
                            record.model = current_model.clone();
                            records.push(record);
                        }
                    }
                    cache.insert(
                        file,
                        CachedSession {
                            size,
                            modified,
                            records: records.clone(),
                        },
                    );
                    records
                }
            };
            if !records.is_empty() {
                sessions.push(records);
            }
        }
        Ok(sessions)
    }

    /// Scans every token record and converts it into one sample per turn.
    /// Modern Codex publishes `last_token_usage`; older fixtures are supported
    /// by reconstructing a delta from cumulative input/output counters.
    fn scan_usage_samples(&self) -> Result<Vec<UsageBreakdownSample>, String> {
        let mut samples = Vec::new();
        for records in self.scan_token_records()? {
            let mut previous: Option<&TokenCountRecord> = None;
            for record in &records {
                let usage = record.last_usage.clone().unwrap_or_else(|| {
                    let (input_tokens, output_tokens) = cumulative_delta(record, previous);
                    UsageCounters {
                        input: input_tokens,
                        cached: 0,
                        cache_write: 0,
                        output: output_tokens,
                        reasoning: 0,
                    }
                });
                let input_tokens = usage.input.saturating_sub(usage.cached);
                if input_tokens > 0 || usage.cached > 0 || usage.cache_write > 0 || usage.output > 0
                {
                    samples.push(UsageBreakdownSample {
                        timestamp: record.timestamp,
                        input_tokens,
                        cached_tokens: usage.cached,
                        cache_write_tokens: usage.cache_write,
                        output_tokens: usage.output,
                        reasoning_tokens: usage.reasoning,
                        model: record.model.clone(),
                    });
                }
                previous = Some(record);
            }
        }
        samples.sort_by_key(|sample| sample.timestamp);
        Ok(samples)
    }

    fn scan_samples(&self) -> Result<Vec<TokenSample>, String> {
        let mut samples = Vec::new();
        for records in self.scan_token_records()? {
            // Track the largest cumulative totals and the newest timestamp.
            let mut best: Option<(DateTime<Utc>, u64, u64)> = None;
            for record in records {
                match &mut best {
                    Some((ts, input, output)) => {
                        if record.input > *input {
                            *input = record.input;
                        }
                        if record.output > *output {
                            *output = record.output;
                        }
                        if record.timestamp > *ts {
                            *ts = record.timestamp;
                        }
                    }
                    None => {
                        best = Some((record.timestamp, record.input, record.output));
                    }
                }
            }
            if let Some((timestamp, input, output)) = best {
                if input > 0 || output > 0 {
                    samples.push(TokenSample {
                        timestamp,
                        input_tokens: input,
                        output_tokens: output,
                        model: None,
                    });
                }
            }
        }
        samples.sort_by_key(|sample| sample.timestamp);
        Ok(samples)
    }

    /// The rate-limit windows Codex published in a local `token_count` record.
    /// The newest local snapshot wins; it is only used when the live API is
    /// unavailable.
    fn local_rate_limits(&self) -> Result<Option<QuotaReport>, String> {
        let mut newest: Option<(DateTime<Utc>, Value)> = None;
        for records in self.scan_token_records()? {
            for record in records {
                let Some(limits) = record.rate_limits else {
                    continue;
                };
                if limits.get("primary").is_none() {
                    continue;
                }
                if newest.as_ref().is_none_or(|(ts, _)| record.timestamp > *ts) {
                    newest = Some((record.timestamp, limits));
                }
            }
        }
        let Some((_, limits)) = newest else {
            return Ok(None);
        };
        let windows = rate_limit_windows(&limits);
        if windows.is_empty() {
            return Ok(None);
        }
        let mut report =
            QuotaReport::new("openai_codex", "local_jsonl", windows, DEFAULT_FRESHNESS);
        if let Some(plan) = limits.get("plan_type").and_then(|v| v.as_str()) {
            if !plan.is_empty() {
                report.plan = Some(plan.to_string());
            }
        }
        Ok(Some(report))
    }

    /// Quota for the Codex subscription.
    ///
    /// Order: the OpenAI usage API via the stored OAuth token, then the
    /// latest local CLI rate-limit snapshot. A local token total is usage
    /// history, not a quota, so it is never used as a final fallback.
    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate_with_fetch(|auth_path| {
            usage_api::fetch_windows(
                auth_path,
                &usage_api::default_endpoint(),
                std::time::Duration::from_secs(10),
            )
        })
    }

    /// Collects quota with an injected fetch operation so source precedence
    /// and sanitized error handling can be tested without network requests.
    fn quota_estimate_with_fetch<F>(&self, fetch: F) -> Result<Option<QuotaReport>, String>
    where
        F: FnOnce(&Path) -> Result<Option<Vec<QuotaWindow>>, String>,
    {
        match fetch(&self.auth_path) {
            Ok(Some(windows)) => {
                let mut report =
                    QuotaReport::new("openai_codex", "provider_api", windows, DEFAULT_FRESHNESS);
                report.plan = Some("Subscription".to_string());
                Ok(Some(report))
            }
            Ok(None) => self.local_fallback(),
            Err(code) if code == "AUTH_EXPIRED" || code == "RATE_LIMITED" => Err(code),
            Err(_) => self.local_fallback(),
        }
    }

    fn local_fallback(&self) -> Result<Option<QuotaReport>, String> {
        match self.local_rate_limits() {
            Ok(Some(report)) => Ok(Some(report)),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// One parsed `token_count` record: the session's cumulative totals at that
/// moment, plus the rate-limit windows when the CLI published them.
#[derive(Debug, Clone)]
struct UsageCounters {
    input: u64,
    cached: u64,
    cache_write: u64,
    output: u64,
    reasoning: u64,
}

#[derive(Debug, Clone)]
struct TokenCountRecord {
    timestamp: DateTime<Utc>,
    input: u64,
    output: u64,
    last_usage: Option<UsageCounters>,
    rate_limits: Option<Value>,
    model: Option<String>,
}

fn parse_usage_counters(value: Option<&Value>) -> Option<UsageCounters> {
    let usage = value?.as_object()?;
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached = usage
        .get("cached_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_write = usage
        .get("cache_write_input_tokens")
        .or_else(|| usage.get("cache_creation_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning = usage
        .get("reasoning_output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if input == 0 && cached == 0 && cache_write == 0 && output == 0 && reasoning == 0 {
        return None;
    }
    Some(UsageCounters {
        input,
        cached,
        cache_write,
        output,
        reasoning,
    })
}

fn non_empty_model(value: Option<&Value>) -> Option<String> {
    let model = value?.as_str()?.trim();
    (!model.is_empty()).then(|| model.to_string())
}

/// Extracts the effective model from Codex metadata without reading or
/// retaining prompt/response content. `turn_context` is authoritative for
/// the model used by the following turn; the other paths cover older/newer
/// metadata shapes that expose the same identifier.
fn model_from_metadata_line(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    let kind = value.get("type").and_then(Value::as_str)?;
    if !matches!(kind, "turn_context" | "session_meta") {
        return None;
    }
    let payload = value.get("payload")?;
    ["model", "model_slug", "model_id", "model_name"]
        .iter()
        .find_map(|key| non_empty_model(payload.get(*key)))
        .or_else(|| {
            payload
                .get("collaboration_mode")
                .and_then(|value| value.get("settings"))
                .and_then(|settings| non_empty_model(settings.get("model")))
        })
        .or_else(|| {
            payload
                .get("model_settings")
                .and_then(|settings| non_empty_model(settings.get("model")))
        })
}

/// Parses one Codex session JSONL line carrying `event_msg/token_count`.
/// Any other line shape is skipped.
fn parse_token_count_line(line: &str) -> Option<TokenCountRecord> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(|v| v.as_str()) != Some("event_msg") {
        return None;
    }
    let payload = value.get("payload")?;
    if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
        return None;
    }
    let timestamp = value.get("timestamp").and_then(|v| v.as_str())?;
    let dt = DateTime::parse_from_rfc3339(timestamp)
        .ok()?
        .with_timezone(&Utc);
    let info = payload.get("info")?;
    let usage = info.get("total_token_usage")?;
    let input = usage
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if input == 0 && output == 0 {
        return None;
    }
    Some(TokenCountRecord {
        timestamp: dt,
        input,
        output,
        last_usage: parse_usage_counters(info.get("last_token_usage")),
        rate_limits: payload.get("rate_limits").cloned(),
        model: None,
    })
}

fn cumulative_delta(current: &TokenCountRecord, previous: Option<&TokenCountRecord>) -> (u64, u64) {
    let Some(previous) = previous else {
        return (current.input, current.output);
    };
    if current.input < previous.input || current.output < previous.output {
        return (current.input, current.output);
    }
    (
        current.input - previous.input,
        current.output - previous.output,
    )
}

/// Window duration classification from the minutes Codex publishes.
fn classify_minutes(minutes: Option<u64>) -> (&'static str, &'static str, QuotaWindowScope) {
    match minutes {
        Some(300) => ("session", "Session", QuotaWindowScope::Rolling),
        Some(10_080) => ("weekly", "Weekly", QuotaWindowScope::Weekly),
        Some(43_200) => ("monthly", "Monthly", QuotaWindowScope::Monthly),
        _ => ("window", "Rate limit", QuotaWindowScope::Other),
    }
}

/// Converts the `rate_limits` object of a `token_count` record into quota
/// windows. The primary slot always exists; the secondary slot is used when
/// it declares a different duration.
fn rate_limit_windows(limits: &Value) -> Vec<QuotaWindow> {
    let mut windows = Vec::new();
    for slot in ["primary", "secondary"] {
        let Some(window) = limits.get(slot) else {
            continue;
        };
        let Some(used_percent) = window.get("used_percent").and_then(|v| v.as_f64()) else {
            continue;
        };
        let minutes = window.get("window_minutes").and_then(|v| v.as_u64());
        let (key, label, scope) = classify_minutes(minutes);
        let reset_at = window
            .get("resets_at")
            .and_then(|v| v.as_i64())
            .and_then(|secs| DateTime::from_timestamp(secs, 0));
        if windows
            .iter()
            .any(|existing: &QuotaWindow| existing.window_key == key)
        {
            continue;
        }
        windows.push(QuotaWindow::from_percent(
            key,
            label,
            scope,
            QuotaKind::Requests,
            used_percent,
            reset_at,
            Confidence::High,
        ));
    }
    windows
}

/// Collects every `.jsonl` session file under `root`, recursively.
///
/// Codex stores rollouts as `sessions/<year>/<month>/<day>/*.jsonl`, so a
/// two-level walk would find nothing; the shared collector handles arbitrary
/// depth with the adapter's file/byte bounds.
fn collect_jsonl_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.is_dir() {
        return Err("SOURCE_UNAVAILABLE".to_string());
    }
    let bounds = lnwdeck_provider_runtime::token_scan::ScanBounds {
        max_files: MAX_FILES,
        max_total_bytes: MAX_TOTAL_BYTES,
        ..lnwdeck_provider_runtime::token_scan::ScanBounds::default()
    };
    Ok(lnwdeck_provider_runtime::token_scan::collect_files(
        root, &bounds,
    ))
}

impl ProviderAdapter for CodexAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: "openai_codex",
            display_name: "OpenAI Codex",
            vendor: "OpenAI",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            // The CLI publishes real per-window usage into session files.
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        if !self.sessions_dir.is_dir() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        let samples = self.scan_usage_samples()?;
        Ok(UsageBatch {
            batch_id: format!("codex_{}", chrono::Utc::now().timestamp()),
            events: usage_events_with_breakdown(
                "openai_codex",
                "local_jsonl_v2",
                &samples,
                Confidence::Medium,
            ),
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }
    fn account_identity(&self) -> Option<String> {
        let auth = usage_api::read_auth(&self.auth_path)?;
        auth.account_id.or(Some(auth.access_token))
    }
    fn health_check(&self) -> AdapterHealth {
        match self.detection().detected {
            true => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Codex local sessions detected".to_string(),
            },
            false => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Codex local sessions not found".to_string(),
            },
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem, Permission::Credential]
    }
    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(self.detection())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_provider_runtime::AdapterHealthStatus;
    use tempfile::tempdir;

    fn write_session(dir: &Path, name: &str, lines: &[&str]) {
        let file = dir.join(name);
        std::fs::write(&file, lines.join("\n")).expect("write session");
    }

    /// A `token_count` record with cumulative totals and optional rate limits.
    fn token_count(ts: &str, input: u64, output: u64, limits: Option<&str>) -> String {
        let limits_part = limits
            .map(|raw| format!(r#","rate_limits":{raw}"#))
            .unwrap_or_default();
        format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"output_tokens":{output},"total_tokens":{}}}}},"model_context_window":258400{limits_part}}}}}"#,
            input + output
        )
    }

    fn token_count_with_last_usage(
        ts: &str,
        total_input: u64,
        total_output: u64,
        last_input: u64,
        last_cached: u64,
        last_output: u64,
        last_reasoning: u64,
    ) -> String {
        serde_json::json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "last_token_usage": {
                        "input_tokens": last_input,
                        "cached_input_tokens": last_cached,
                        "cache_write_input_tokens": 0,
                        "output_tokens": last_output,
                        "reasoning_output_tokens": last_reasoning,
                        "total_tokens": last_input + last_output,
                    },
                    "total_token_usage": {
                        "input_tokens": total_input,
                        "output_tokens": total_output,
                        "total_tokens": total_input + total_output,
                    },
                },
            },
        })
        .to_string()
    }

    fn turn_context(ts: &str, model: &str) -> String {
        serde_json::json!({
            "timestamp": ts,
            "type": "turn_context",
            "payload": { "model": model }
        })
        .to_string()
    }

    fn response_item(ts: &str) -> String {
        format!(
            r#"{{"type":"response_item","timestamp":"{ts}","payload":{{"type":"message","role":"assistant"}}}}"#
        )
    }

    fn adapter_for(sessions: &Path) -> CodexAdapter {
        CodexAdapter::with_paths(sessions.to_path_buf(), sessions.join("auth.json"))
    }

    fn now_minus(secs: i64) -> String {
        (Utc::now() - chrono::Duration::seconds(secs)).to_rfc3339()
    }

    fn rate_limits_json(used_percent: f64, minutes: u64, plan: &str) -> String {
        format!(
            r#"{{"limit_id":"codex","primary":{{"used_percent":{used_percent},"window_minutes":{minutes},"resets_at":4102444800}},"plan_type":"{plan}"}}"#
        )
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(CodexAdapter::new().id(), "openai_codex");
    }

    #[test]
    fn descriptor_uses_the_full_openai_codex_name() {
        assert_eq!(
            CodexAdapter::new().descriptor().display_name,
            "OpenAI Codex"
        );
    }

    #[test]
    fn requires_filesystem_and_credential() {
        let permissions = CodexAdapter::new().required_permissions();
        assert!(permissions.contains(&Permission::FileSystem));
        assert!(permissions.contains(&Permission::Credential));
    }

    #[test]
    fn token_count_lines_are_parsed_and_other_lines_are_skipped() {
        let record = parse_token_count_line(&token_count("2026-08-04T00:00:00Z", 8771, 84, None))
            .expect("parsed");
        assert_eq!(record.input, 8771);
        assert_eq!(record.output, 84);
        assert!(record.rate_limits.is_none());

        assert!(parse_token_count_line("not json").is_none());
        assert!(parse_token_count_line(&response_item("2026-08-04T00:00:00Z")).is_none());
        assert!(parse_token_count_line(&token_count("2026-08-04T00:00:00Z", 0, 0, None)).is_none());
    }

    #[test]
    fn rate_limit_windows_come_from_the_published_record() {
        let limits: Value =
            serde_json::from_str(&rate_limits_json(98.0, 10_080, "plus")).expect("json");
        let windows = rate_limit_windows(&limits);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window_key, "weekly");
        assert_eq!(windows[0].label, "Weekly");
        assert_eq!(windows[0].used_percent, Some(98.0));
        assert_eq!(windows[0].remaining_percent, Some(2.0));
        assert_eq!(windows[0].scope, QuotaWindowScope::Weekly);
        assert!(windows[0].reset_at.is_some());
        windows[0].check_invariants().expect("consistent");
    }

    #[test]
    fn cumulative_totals_are_not_presented_as_quota() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dirs");
        // Three turns with cumulative totals: 1000, 1800, 2400. The session
        // total is 2400 โ€” summing every record would give 5200.
        write_session(
            &sessions,
            "sess_1.jsonl",
            &[
                &token_count(&now_minus(3600), 800, 200, None),
                &token_count(&now_minus(1800), 1500, 300, None),
                &token_count(&now_minus(600), 2100, 300, None),
            ],
        );

        let adapter = adapter_for(&sessions);
        assert_eq!(
            adapter.collect_quota().expect("quota call"),
            None,
            "cumulative local totals are usage history, not a subscription quota"
        );
    }

    #[test]
    fn quota_estimate_prefers_the_published_rate_limits() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dirs");
        write_session(
            &sessions,
            "sess_1.jsonl",
            &[&token_count(
                &now_minus(60),
                100,
                20,
                Some(&rate_limits_json(98.0, 10_080, "plus")),
            )],
        );

        let adapter = adapter_for(&sessions);
        let report = adapter
            .collect_quota()
            .expect("quota call")
            .expect("report");
        assert_eq!(report.source, "local_jsonl");
        assert_eq!(report.plan.as_deref(), Some("plus"));
        let weekly = report
            .windows
            .iter()
            .find(|w| w.window_key == "weekly")
            .unwrap();
        assert_eq!(weekly.used_percent, Some(98.0));
    }

    #[test]
    fn quota_estimate_prefers_live_windows_over_local_snapshot() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dirs");
        write_session(
            &sessions,
            "sess_1.jsonl",
            &[&token_count(
                &now_minus(60),
                100,
                20,
                Some(&rate_limits_json(98.0, 10_080, "plus")),
            )],
        );

        let adapter = adapter_for(&sessions);
        let report = adapter
            .quota_estimate_with_fetch(|_| {
                Ok(Some(vec![QuotaWindow::from_percent(
                    "weekly",
                    "Weekly",
                    QuotaWindowScope::Weekly,
                    QuotaKind::Requests,
                    12.0,
                    None,
                    Confidence::High,
                )]))
            })
            .expect("quota call")
            .expect("report");

        assert_eq!(report.source, "provider_api");
        assert_eq!(report.plan.as_deref(), Some("Subscription"));
        assert_eq!(report.windows[0].used_percent, Some(12.0));
    }

    #[test]
    fn non_hard_live_failure_falls_back_to_local_snapshot() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dirs");
        write_session(
            &sessions,
            "sess_1.jsonl",
            &[&token_count(
                &now_minus(60),
                100,
                20,
                Some(&rate_limits_json(98.0, 10_080, "plus")),
            )],
        );

        let adapter = adapter_for(&sessions);
        let report = adapter
            .quota_estimate_with_fetch(|_| Err("PROVIDER_UNAVAILABLE".to_string()))
            .expect("quota call")
            .expect("report");

        assert_eq!(report.source, "local_jsonl");
        assert_eq!(report.windows[0].used_percent, Some(98.0));
    }

    #[test]
    fn hard_live_failures_are_not_hidden_by_local_snapshot() {
        for code in ["AUTH_EXPIRED", "RATE_LIMITED"] {
            let dir = tempdir().expect("temp dir");
            let sessions = dir.path().join("sessions");
            std::fs::create_dir_all(&sessions).expect("create dirs");
            write_session(
                &sessions,
                "sess_1.jsonl",
                &[&token_count(
                    &now_minus(60),
                    100,
                    20,
                    Some(&rate_limits_json(98.0, 10_080, "plus")),
                )],
            );

            let adapter = adapter_for(&sessions);
            assert_eq!(
                adapter.quota_estimate_with_fetch(|_| Err(code.to_string())),
                Err(code.to_string())
            );
        }
    }

    #[test]
    fn quota_estimate_does_not_fallback_to_usage_only_without_rate_limits() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dirs");
        write_session(
            &sessions,
            "sess_1.jsonl",
            &[&token_count(&now_minus(3600), 800, 200, None)],
        );

        let adapter = adapter_for(&sessions);
        assert!(
            adapter.collect_quota().expect("quota call").is_none(),
            "local token totals are usage history, not a published quota"
        );
    }

    #[test]
    fn quota_estimate_is_none_when_no_sessions() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dir");
        let adapter = adapter_for(&sessions);
        assert!(
            adapter.collect_quota().expect("quota call").is_none(),
            "no sessions means no estimate"
        );
    }

    #[test]
    fn detection_classifies_auth_presence() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).expect("create dirs");
        write_session(
            &sessions,
            "sess_1.jsonl",
            &[&token_count(&now_minus(60), 10, 5, None)],
        );

        let adapter = adapter_for(&sessions);
        assert_eq!(
            adapter.detect().expect("detect").permission_state,
            "read_ok_no_auth"
        );

        std::fs::write(sessions.join("auth.json"), r#"{"tokens":{"id_token":"x"}}"#)
            .expect("write auth");
        assert_eq!(
            adapter.detect().expect("detect").permission_state,
            "read_ok_auth"
        );
    }

    #[test]
    fn health_reflects_detection() {
        let dir = tempdir().expect("temp dir");
        let missing = dir.path().join("nope");
        let adapter = adapter_for(&missing);
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }

    #[test]
    fn unchanged_sessions_are_reused_and_new_sessions_are_picked_up() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("sessions");
        std::fs::create_dir_all(&root).expect("create root");
        write_session(
            &root,
            "old.jsonl",
            &[&token_count(&now_minus(600), 300, 100, None)],
        );

        let adapter = CodexAdapter::with_paths(root.clone(), root.join("auth.json"));
        let first = adapter.collect_usage().expect("first usage");
        assert_eq!(first.events.len(), 1);

        // A new session appears: the cached file is reused and only the new
        // file is parsed, and both end up in the snapshot.
        write_session(
            &root,
            "new.jsonl",
            &[&token_count(&now_minus(300), 700, 250, None)],
        );
        let second = adapter.collect_usage().expect("second usage");
        assert_eq!(
            second.events.len(),
            2,
            "cached session plus the newly added session"
        );
        assert!(second.events.iter().any(|event| event.tokens_input == 300));
        assert!(second.events.iter().any(|event| event.tokens_input == 700));

        // An existing session gains another turn: its size/mtime changed, so
        // the file is re-read and the turn is reconstructed from the delta.
        let old_path = root.join("old.jsonl");
        let mut content = std::fs::read_to_string(&old_path).expect("read old");
        content.push('\n');
        content.push_str(&token_count(&now_minus(60), 900, 350, None));
        std::fs::write(&old_path, content).expect("append old");

        let third = adapter.collect_usage().expect("third usage");
        assert_eq!(
            third.events.len(),
            3,
            "the changed session's new turn is added to the snapshot"
        );
    }

    #[test]
    fn collect_usage_emits_per_turn_deltas_for_a_session() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("sessions");
        std::fs::create_dir_all(&root).expect("create root");
        write_session(
            &root,
            "session.jsonl",
            &[
                &token_count(&now_minus(600), 300, 100, None),
                &token_count(&now_minus(300), 700, 250, None),
            ],
        );

        let adapter = CodexAdapter::with_paths(root.clone(), root.join("auth.json"));
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2, "each cumulative change is one turn");
        assert_eq!(batch.events[0].provider_id, "openai_codex");
        assert_eq!(batch.events[0].tokens_input, 300);
        assert_eq!(batch.events[0].tokens_output, 100);
        assert_eq!(batch.events[1].tokens_input, 400);
        assert_eq!(batch.events[1].tokens_output, 150);
        assert!(
            batch.events[0].cost.is_empty(),
            "a collector must not invent a cost"
        );
    }

    #[test]
    fn collect_usage_emits_cumulative_deltas_for_each_turn() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("sessions");
        std::fs::create_dir_all(&root).expect("create root");
        write_session(
            &root,
            "session.jsonl",
            &[
                &token_count(&now_minus(900), 1_000, 100, None),
                &token_count(&now_minus(600), 1_600, 150, None),
                &token_count(&now_minus(300), 2_200, 200, None),
            ],
        );

        let adapter = CodexAdapter::with_paths(root, dir.path().join("auth.json"));
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 3, "each cumulative change is one turn");
        assert_eq!(
            batch
                .events
                .iter()
                .map(|event| event.tokens_input)
                .sum::<u64>(),
            2_200,
            "input totals are reconstructed from cumulative deltas"
        );
        assert_eq!(
            batch
                .events
                .iter()
                .map(|event| event.tokens_output)
                .sum::<u64>(),
            200,
            "output totals are reconstructed from cumulative deltas"
        );
    }

    #[test]
    fn collect_usage_uses_per_turn_breakdown_when_codex_publishes_it() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("sessions");
        std::fs::create_dir_all(&root).expect("create root");
        write_session(
            &root,
            "session.jsonl",
            &[&token_count_with_last_usage(
                &now_minus(60),
                10_000,
                400,
                10_000,
                9_000,
                400,
                120,
            )],
        );

        let adapter = CodexAdapter::with_paths(root, dir.path().join("auth.json"));
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].tokens_input, 1_000);
        assert_eq!(batch.events[0].tokens_cached, 9_000);
        assert_eq!(batch.events[0].tokens_output, 400);
        assert_eq!(batch.events[0].tokens_reasoning, 120);
    }

    #[test]
    fn collect_usage_keeps_the_effective_model_from_turn_context() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("sessions");
        std::fs::create_dir_all(&root).expect("create root");
        write_session(
            &root,
            "session.jsonl",
            &[
                &turn_context(&now_minus(120), "gpt-5.3-codex"),
                &token_count_with_last_usage(&now_minus(60), 10_000, 400, 10_000, 9_000, 400, 120),
            ],
        );

        let adapter = CodexAdapter::with_paths(root, dir.path().join("auth.json"));
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events[0].model, "gpt-5.3-codex");
    }

    #[test]
    fn collect_usage_reports_a_missing_source_instead_of_an_empty_batch() {
        let adapter = CodexAdapter::with_paths(
            PathBuf::from("Z:/missing"),
            PathBuf::from("Z:/missing/auth.json"),
        );
        assert_eq!(
            adapter.collect_usage().expect_err("must fail"),
            "SOURCE_UNAVAILABLE"
        );
    }
}
