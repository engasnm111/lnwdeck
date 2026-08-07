//! Kimi Code / Kimi CLI adapter.
//!
//! Kimi sessions are written as `wire.jsonl` in two layouts:
//!
//! - legacy Kimi CLI: `~/.kimi/sessions/<ws>/<session>/wire.jsonl` with
//!   `{"message":{"type":"StatusUpdate","payload":{"token_usage":
//!   {"input_other":N,"output":N,"input_cache_read":N,
//!   "input_cache_creation":N},"message_id":"...","timestamp":<epoch sec>}},
//!   "timestamp":<epoch sec>}`;
//! - official Kimi Code: `~/.kimi-code/sessions/<wd>/<session>/agents/<name>/
//!   wire.jsonl` with `{"type":"context.append_loop_event","event":
//!   {"type":"step.end","usage":{"input_tokens":N,"output_tokens":N,
//!   "cache_read_input_tokens":N,"cache_creation_input_tokens":N}},
//!   "time":<epoch ms>}` and per-session `config.update` events carrying the
//!   `modelAlias`.
//!
//! Both layouts are parsed here; token usage and timestamps are the only
//! things carried out. Quota is a usage-only local estimate.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::token_scan::{usage_events, TokenSample};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::path::PathBuf;

const PROVIDER_ID: &str = "kimi_code";
const ADAPTER_VERSION: &str = "0.1.0";

/// Hard bounds for the wire log scan, matching the shared scanner.
const MAX_FILES: usize = 400;
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

pub struct KimiAdapter {
    roots: Vec<PathBuf>,
}

impl Default for KimiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KimiAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::with_roots(vec![home.join(".kimi"), home.join(".kimi-code")])
    }

    /// Adapter pinned to explicit source roots (used by tests).
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    fn any_root_exists(&self) -> bool {
        self.roots.iter().any(|root| root.is_dir())
    }

    /// Every `wire.jsonl` file under the roots, bounded and sorted.
    fn wire_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack: Vec<PathBuf> = self
            .roots
            .iter()
            .filter(|root| root.is_dir())
            .cloned()
            .collect();
        while let Some(dir) = stack.pop() {
            if files.len() >= MAX_FILES {
                break;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.file_name().and_then(|name| name.to_str()) == Some("wire.jsonl") {
                    files.push(path);
                    if files.len() >= MAX_FILES {
                        break;
                    }
                }
            }
        }
        files.sort();
        files
    }

    fn samples(&self) -> Vec<TokenSample> {
        let mut samples = Vec::new();
        let mut bytes_read = 0u64;
        for path in self.wire_files() {
            let Ok(meta) = path.metadata() else {
                continue;
            };
            if meta.len() > MAX_FILE_BYTES || bytes_read + meta.len() > MAX_TOTAL_BYTES {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            bytes_read += meta.len();
            extract_wire_samples(&content, &mut samples);
        }
        samples
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.any_root_exists();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Kimi Code".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_scan".to_string(),
            source_type: "wire_jsonl".to_string(),
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
        if self.samples().is_empty() {
            result.permission_state = "no_sessions".to_string();
        } else {
            result.detected = true;
            result.permission_state = "read_ok".to_string();
        }
        result
    }

    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        let samples = self.samples();
        if samples.is_empty() {
            return Ok(None);
        }
        let now = chrono::Utc::now();
        let windows = [
            ("5h", "5-hour", QuotaWindowScope::Rolling, 5 * 3600i64),
            ("7d", "7-day", QuotaWindowScope::Weekly, 7 * 24 * 3600),
            ("30d", "30-day", QuotaWindowScope::Monthly, 30 * 24 * 3600),
        ]
        .into_iter()
        .map(|(key, label, scope, seconds)| {
            let used = samples
                .iter()
                .filter(|sample| {
                    sample.timestamp > now - chrono::Duration::seconds(seconds)
                        && sample.timestamp <= now
                })
                .fold(0u64, |acc, sample| {
                    acc.saturating_add(sample.input_tokens)
                        .saturating_add(sample.output_tokens)
                });
            QuotaWindow::usage_only(
                key,
                label,
                scope,
                QuotaKind::Tokens,
                used,
                None,
                Confidence::Medium,
            )
        })
        .collect();
        Ok(Some(QuotaReport::new(
            PROVIDER_ID,
            "local_estimate",
            windows,
            DEFAULT_FRESHNESS,
        )))
    }
}

/// Parses one wire.jsonl document into token samples, covering the legacy
/// StatusUpdate and the official step.end layouts. The model alias from
/// `config.update` events applies to subsequent records in the same file.
fn extract_wire_samples(content: &str, out: &mut Vec<TokenSample>) {
    let mut model: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(alias) = entry
            .get("message")
            .and_then(|message| message.get("payload"))
            .and_then(|payload| payload.get("modelAlias"))
            .and_then(Value::as_str)
        {
            let alias = alias.trim();
            if !alias.is_empty() {
                model = Some(alias.to_string());
            }
            continue;
        }
        if entry.get("type").and_then(Value::as_str) == Some("config.update") {
            if let Some(alias) = entry
                .get("payload")
                .and_then(|payload| payload.get("modelAlias"))
                .and_then(Value::as_str)
            {
                let alias = alias.trim();
                if !alias.is_empty() {
                    model = Some(alias.to_string());
                }
            }
            continue;
        }

        if let Some(sample) = parse_status_update(&entry, model.as_deref()) {
            out.push(sample);
        } else if let Some(sample) = parse_step_end(&entry, model.as_deref()) {
            out.push(sample);
        }
    }
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|v| match v {
        Value::Number(number) => number.as_u64(),
        _ => None,
    })
}

/// Legacy Kimi CLI StatusUpdate records; timestamps are epoch seconds.
fn parse_status_update(entry: &Value, model: Option<&str>) -> Option<TokenSample> {
    let message = entry.get("message")?;
    if message.get("type").and_then(Value::as_str) != Some("StatusUpdate") {
        return None;
    }
    let payload = message.get("payload")?;
    let usage = payload.get("token_usage")?;
    let input = u64_field(usage, "input_other").unwrap_or(0);
    let output = u64_field(usage, "output").unwrap_or(0);
    let cache_read = u64_field(usage, "input_cache_read").unwrap_or(0);
    let cache_write = u64_field(usage, "input_cache_creation").unwrap_or(0);
    if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
        return None;
    }
    let timestamp = payload
        .get("timestamp")
        .or_else(|| entry.get("timestamp"))
        .and_then(Value::as_i64)
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))?;
    Some(TokenSample {
        timestamp,
        input_tokens: input.saturating_add(cache_read).saturating_add(cache_write),
        output_tokens: output,
        model: model.map(str::to_string),
    })
}

/// Official Kimi Code step.end records; timestamps are epoch milliseconds.
fn parse_step_end(entry: &Value, model: Option<&str>) -> Option<TokenSample> {
    if entry.get("type").and_then(Value::as_str) != Some("context.append_loop_event") {
        return None;
    }
    let event = entry.get("event")?;
    if event.get("type").and_then(Value::as_str) != Some("step.end") {
        return None;
    }
    let usage = event.get("usage")?;
    let input = u64_field(usage, "input_tokens").unwrap_or(0);
    let output = u64_field(usage, "output_tokens").unwrap_or(0);
    let cache_read = u64_field(usage, "cache_read_input_tokens").unwrap_or(0);
    let cache_write = u64_field(usage, "cache_creation_input_tokens").unwrap_or(0);
    if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
        return None;
    }
    let timestamp = entry
        .get("time")
        .and_then(Value::as_i64)
        .and_then(chrono::DateTime::from_timestamp_millis)?;
    Some(TokenSample {
        timestamp,
        input_tokens: input.saturating_add(cache_read).saturating_add(cache_write),
        output_tokens: output,
        model: model.map(str::to_string),
    })
}

impl ProviderAdapter for KimiAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Kimi Code",
            vendor: "Moonshot AI",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let samples = self.samples();
        if samples.is_empty() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", chrono::Utc::now().timestamp()),
            events: usage_events(PROVIDER_ID, "local_scan", &samples, Confidence::Medium),
        })
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Kimi sessions not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Kimi wire logs detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Kimi wire logs have no token records".to_string(),
            }
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(self.detection())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_correct() {
        assert_eq!(KimiAdapter::new().id(), "kimi_code");
    }

    #[test]
    fn parses_legacy_status_update_layout() {
        let mut samples = Vec::new();
        extract_wire_samples(
            r#"{"timestamp":1700000100,"message":{"type":"StatusUpdate","payload":{"token_usage":{"input_other":100,"output":50,"input_cache_read":10,"input_cache_creation":2},"message_id":"m1","timestamp":1700000100}}}
{"timestamp":1700000200,"message":{"type":"StatusUpdate","payload":{"token_usage":{"input_other":0,"output":0},"message_id":"m2","timestamp":1700000200}}}"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].input_tokens, 112);
        assert_eq!(samples[0].output_tokens, 50);
        assert_eq!(samples[0].timestamp.timestamp(), 1_700_000_100);
    }

    #[test]
    fn parses_official_step_end_layout_with_model_alias() {
        let mut samples = Vec::new();
        extract_wire_samples(
            r#"{"type":"config.update","payload":{"modelAlias":"kimi-code/kimi-k2.6"}}
{"type":"context.append_loop_event","event":{"type":"step.end","usage":{"input_tokens":120,"output_tokens":30,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}},"time":1700000001000}
{"type":"context.append_loop_event","event":{"type":"step.end","usage":{"input_tokens":0,"output_tokens":0}},"time":1700000002000}"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].input_tokens, 135);
        assert_eq!(samples[0].output_tokens, 30);
        assert_eq!(samples[0].model.as_deref(), Some("kimi-code/kimi-k2.6"));
    }

    #[test]
    fn unparsable_lines_are_ignored() {
        let mut samples = Vec::new();
        extract_wire_samples("not json\n{\"type\":\"other\"}\n", &mut samples);
        assert!(samples.is_empty());
    }

    #[test]
    fn scans_roots_and_reports_no_data_when_missing() {
        let dir = tempfile::tempdir().expect("temp");
        let sessions = dir
            .path()
            .join(".kimi")
            .join("sessions")
            .join("ws")
            .join("s1");
        std::fs::create_dir_all(&sessions).expect("create");
        std::fs::write(
            sessions.join("wire.jsonl"),
            r#"{"timestamp":1700000100,"message":{"type":"StatusUpdate","payload":{"token_usage":{"input_other":10,"output":5},"message_id":"m","timestamp":1700000100}}}"#,
        )
        .expect("write");

        let adapter = KimiAdapter::with_roots(vec![dir.path().join(".kimi")]);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert!(adapter.detection().detected);

        let empty = KimiAdapter::with_roots(vec![dir.path().join("missing")]);
        assert_eq!(empty.collect_usage(), Err("SOURCE_UNAVAILABLE".to_string()));
        assert!(empty.collect_quota().expect("quota").is_none());
    }
}
