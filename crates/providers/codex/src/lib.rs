use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::token_scan::{scan_directory, usage_events, ScanBounds, TokenSample};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

const ADAPTER_VERSION: &str = "0.2.0";
const MAX_FILES: usize = 400;
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

/// Codex CLI passive local collector.
///
/// Reads token usage from the local Codex session JSONL files
/// (`~/.codex/sessions/**/*.jsonl`) read-only. Only numeric token counts and
/// timestamps are aggregated; raw transcripts are never normalized. Quota is
/// a usage estimate with unknown limits, never a fabricated percentage.
pub struct CodexAdapter {
    sessions_dir: PathBuf,
    auth_path: PathBuf,
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
        }
    }

    fn has_credentials(&self) -> bool {
        self.auth_path.is_file()
    }

    fn detection(&self) -> DetectionResult {
        let mut result = DetectionResult {
            provider_id: "openai_codex".to_string(),
            display_name: "Codex".to_string(),
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

    fn scan_session_files(&self) -> Result<Option<[u64; 3]>, String> {
        let now_ms = Utc::now().timestamp_millis();
        let buckets = [
            5 * 3600 * 1000i64,
            7 * 24 * 3600 * 1000i64,
            30 * 24 * 3600 * 1000i64,
        ];
        let mut sums = [0u64; 3];
        let mut any = false;
        let mut files = 0usize;
        let mut total_bytes = 0u64;

        for file in collect_jsonl_files(&self.sessions_dir)? {
            files += 1;
            if files > MAX_FILES {
                break;
            }
            let Ok(meta) = file.metadata() else {
                continue;
            };
            total_bytes += meta.len();
            if total_bytes > MAX_TOTAL_BYTES {
                break;
            }
            let Ok(content) = std::fs::read_to_string(&file) else {
                continue;
            };
            for line in content.lines() {
                let Some((ts_ms, input, output)) = parse_session_line(line) else {
                    continue;
                };
                let tokens = input + output;
                for (i, bucket_ms) in buckets.iter().enumerate() {
                    if now_ms - ts_ms <= *bucket_ms {
                        sums[i] = sums[i].saturating_add(tokens);
                        any = true;
                    }
                }
            }
        }

        Ok(any.then_some(sums))
    }

    /// Scans the bounded session files and returns every token sample found.
    ///
    /// The shared scanner is used so the sample shape, the bounds and the
    /// privacy rules are identical across the local collectors.
    fn scan_samples(&self) -> Result<Vec<TokenSample>, String> {
        let bounds = ScanBounds {
            max_files: MAX_FILES,
            max_total_bytes: MAX_TOTAL_BYTES,
            ..ScanBounds::default()
        };
        let report = scan_directory(&self.sessions_dir, &bounds);
        Ok(report.samples)
    }

    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        if !self.sessions_dir.is_dir() {
            return Ok(None);
        }
        let Some([five_h, seven_d, thirty_d]) = self.scan_session_files()? else {
            return Ok(None);
        };
        let windows = vec![
            QuotaWindow::usage_only(
                "5h",
                "5-hour",
                QuotaWindowScope::Rolling,
                QuotaKind::Tokens,
                five_h,
                None,
                Confidence::Medium,
            ),
            QuotaWindow::usage_only(
                "7d",
                "7-day",
                QuotaWindowScope::Weekly,
                QuotaKind::Tokens,
                seven_d,
                None,
                Confidence::Medium,
            ),
            QuotaWindow::usage_only(
                "30d",
                "30-day",
                QuotaWindowScope::Monthly,
                QuotaKind::Tokens,
                thirty_d,
                None,
                Confidence::Medium,
            ),
        ];
        Ok(Some(QuotaReport::new(
            "openai_codex",
            "local_estimate",
            windows,
            DEFAULT_FRESHNESS,
        )))
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_jsonl_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|_| "SOURCE_UNAVAILABLE")? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            files.push(path);
        }
    }
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Parses one Codex session JSONL line into (timestamp_ms, input, output).
/// The timestamp and usage object are read defensively from the top level or
/// the `payload` object; malformed or non-usage lines are skipped.
fn parse_session_line(line: &str) -> Option<(i64, u64, u64)> {
    let value: Value = serde_json::from_str(line).ok()?;
    let payload = value.get("payload").unwrap_or(&value);
    let timestamp = value
        .get("timestamp")
        .or_else(|| payload.get("timestamp"))
        .and_then(|v| v.as_str())?;
    let dt = DateTime::parse_from_rfc3339(timestamp).ok()?;
    let usage = value.get("usage").or_else(|| payload.get("usage"))?;
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
    Some((dt.timestamp_millis(), input, output))
}

impl ProviderAdapter for CodexAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: "openai_codex",
            display_name: "Codex",
            vendor: "OpenAI",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        if !self.sessions_dir.is_dir() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        let samples = self.scan_samples()?;
        Ok(UsageBatch {
            batch_id: format!("codex_{}", chrono::Utc::now().timestamp()),
            events: usage_events("openai_codex", "local_jsonl", &samples, Confidence::Medium),
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
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

    fn response_item(ts: &str, input: u64, output: u64) -> String {
        format!(
            r#"{{"type":"response_item","timestamp":"{ts}","payload":{{"type":"message","role":"assistant","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}"#
        )
    }

    fn adapter_for(sessions: &Path) -> CodexAdapter {
        CodexAdapter::with_paths(sessions.to_path_buf(), sessions.join("auth.json"))
    }

    fn now_minus(secs: i64) -> String {
        (Utc::now() - chrono::Duration::seconds(secs)).to_rfc3339()
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(CodexAdapter::new().id(), "openai_codex");
    }

    #[test]
    fn requires_filesystem_and_credential() {
        let permissions = CodexAdapter::new().required_permissions();
        assert!(permissions.contains(&Permission::FileSystem));
        assert!(permissions.contains(&Permission::Credential));
    }

    #[test]
    fn parse_session_line_extracts_usage() {
        let line = response_item("2026-08-04T00:00:00Z", 120, 60);
        let (ts_ms, input, output) = parse_session_line(&line).expect("parsed");
        assert_eq!(input, 120);
        assert_eq!(output, 60);
        assert!(ts_ms > 0);
        assert!(parse_session_line("not json").is_none());
        assert!(
            parse_session_line(r#"{"type":"response_item","payload":{"type":"reasoning"}}"#)
                .is_none()
        );
    }

    #[test]
    fn quota_estimate_aggregates_rolling_windows() {
        let dir = tempdir().expect("temp dir");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(sessions.join("sub")).expect("create dirs");
        write_session(
            &sessions.join("sub"),
            "sess_1.jsonl",
            &[
                &response_item(&now_minus(3600), 800, 200),
                &response_item(&now_minus(2 * 24 * 3600), 300, 100),
            ],
        );

        let adapter = adapter_for(&sessions);
        let report = adapter
            .collect_quota()
            .expect("quota call")
            .expect("report");
        assert_eq!(report.provider_id, "openai_codex");
        assert_eq!(report.source, "local_estimate");
        assert_eq!(report.windows.len(), 3);
        let five_h = report
            .windows
            .iter()
            .find(|w| w.window_key == "5h")
            .unwrap();
        let seven_d = report
            .windows
            .iter()
            .find(|w| w.window_key == "7d")
            .unwrap();
        assert_eq!(five_h.used, 1000, "only sessions in last 5h");
        assert_eq!(seven_d.used, 1400, "both sessions within 7d");
        assert_eq!(five_h.limit, None, "limit is unknown, never fabricated");
        assert_eq!(five_h.remaining_percent, None);
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
        std::fs::create_dir_all(sessions.join("sub")).expect("create dirs");
        write_session(
            &sessions.join("sub"),
            "sess_1.jsonl",
            &[&response_item(&now_minus(60), 10, 5)],
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
    fn collect_usage_emits_real_events_from_local_sessions() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("sessions");
        std::fs::create_dir_all(&root).expect("create root");
        let recent = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        std::fs::write(
            root.join("session.jsonl"),
            format!(
                r#"{{"type":"assistant","timestamp":"{recent}","message":{{"id":"m1","role":"assistant","model":"codex-e2e","usage":{{"input_tokens":300,"output_tokens":100}}}}}}"#
            ),
        )
        .expect("write session");

        let adapter = CodexAdapter::with_paths(root.clone(), root.join("auth.json"));
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(
            batch.events.len(),
            1,
            "a real session record must become a usage event, not an empty success"
        );
        assert_eq!(batch.events[0].provider_id, "openai_codex");
        assert_eq!(batch.events[0].tokens_input, 300);
        assert_eq!(batch.events[0].tokens_output, 100);
        assert_eq!(batch.events[0].model, "codex-e2e");
        assert!(
            batch.events[0].cost.is_empty(),
            "a collector must not invent a cost"
        );
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
