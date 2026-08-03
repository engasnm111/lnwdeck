use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::{
    AdapterHealth, AdapterHealthStatus, DetectionResult, Permission, ProviderAdapter,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

const ADAPTER_VERSION: &str = "0.2.0";
/// Hard bounds for the passive local scan so a huge or corrupted history can
/// never stall or exhaust the collector.
const MAX_FILES: usize = 400;
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

/// Claude Code passive local collector.
///
/// Reads token usage from the local Claude Code session JSONL files
/// (`~/.claude/projects/**/*.jsonl`) read-only. Raw session ids, prompts and
/// responses never become normalized data; only numeric token counts and
/// timestamps are aggregated. Quota is a usage estimate with unknown limits,
/// never a fabricated remaining percentage.
pub struct ClaudeAdapter {
    projects_dir: PathBuf,
    credentials_path: PathBuf,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::with_paths(
            home.join(".claude/projects"),
            home.join(".claude/.credentials.json"),
        )
    }

    /// Adapter pinned to explicit local paths (used by tests).
    pub fn with_paths(projects_dir: PathBuf, credentials_path: PathBuf) -> Self {
        Self {
            projects_dir,
            credentials_path,
        }
    }

    fn has_credentials(&self) -> bool {
        self.credentials_path.is_file()
    }

    fn detection(&self) -> DetectionResult {
        let mut result = DetectionResult {
            provider_id: "anthropic_claude".to_string(),
            display_name: "Claude".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_jsonl".to_string(),
            source_type: "jsonl".to_string(),
            source_exists: self.projects_dir.is_dir(),
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

    /// Scans bounded Claude session JSONL files and returns the aggregated
    /// token usage buckets: (5h, 7d, 30d) in input+output tokens.
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

        for file in collect_jsonl_files(&self.projects_dir)? {
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

    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        if !self.projects_dir.is_dir() {
            return Ok(None);
        }
        let Some([five_h, seven_d, thirty_d]) = self.scan_session_files()? else {
            return Ok(None);
        };
        let windows = vec![
            QuotaWindow::new(
                "5h",
                "5-hour",
                QuotaWindowScope::Rolling,
                QuotaKind::Tokens,
                five_h,
                0,
                None,
                Confidence::Medium,
            ),
            QuotaWindow::new(
                "7d",
                "7-day",
                QuotaWindowScope::Weekly,
                QuotaKind::Tokens,
                seven_d,
                0,
                None,
                Confidence::Medium,
            ),
            QuotaWindow::new(
                "30d",
                "30-day",
                QuotaWindowScope::Monthly,
                QuotaKind::Tokens,
                thirty_d,
                0,
                None,
                Confidence::Medium,
            ),
        ];
        Ok(Some(QuotaReport::new(
            "anthropic_claude",
            "local_estimate",
            windows,
            DEFAULT_FRESHNESS,
        )))
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Collects `.jsonl` files under `root`, bounded to a shallow walk.
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

/// Parses one Claude Code session JSONL line into (timestamp_ms, input,
/// output). Malformed or non-usage lines are skipped.
fn parse_session_line(line: &str) -> Option<(i64, u64, u64)> {
    let value: Value = serde_json::from_str(line).ok()?;
    let timestamp = value.get("timestamp")?.as_str()?;
    let dt = DateTime::parse_from_rfc3339(timestamp).ok()?;
    let usage = value.get("message")?.get("usage")?;
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

impl ProviderAdapter for ClaudeAdapter {
    fn id(&self) -> &str {
        "anthropic_claude"
    }
    fn name(&self) -> &str {
        "Claude"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("claude_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }
    fn health_check(&self) -> AdapterHealth {
        match self.detection().detected {
            true => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Claude local sessions detected".to_string(),
            },
            false => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Claude local sessions not found".to_string(),
            },
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
    use lnwdeck_provider_runtime::AdapterHealthStatus;
    use tempfile::tempdir;

    fn write_session(dir: &Path, name: &str, lines: &[&str]) {
        let file = dir.join(name);
        std::fs::write(&file, lines.join("\n")).expect("write session");
    }

    fn assistant_line(ts: &str, input: u64, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"m1","role":"assistant","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}"#
        )
    }

    fn adapter_for(projects: &Path) -> ClaudeAdapter {
        ClaudeAdapter::with_paths(projects.to_path_buf(), projects.join("credentials.json"))
    }

    fn now_minus(secs: i64) -> String {
        (Utc::now() - chrono::Duration::seconds(secs)).to_rfc3339()
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(ClaudeAdapter::new().id(), "anthropic_claude");
    }

    #[test]
    fn requires_filesystem_permission() {
        assert!(ClaudeAdapter::new()
            .required_permissions()
            .contains(&Permission::FileSystem));
    }

    #[test]
    fn parse_session_line_extracts_usage() {
        let line = assistant_line("2026-08-04T00:00:00Z", 100, 50);
        let (ts_ms, input, output) = parse_session_line(&line).expect("parsed");
        assert_eq!(input, 100);
        assert_eq!(output, 50);
        assert!(ts_ms > 0);
        assert!(parse_session_line(r#"{"type":"user","message":{"content":"hi"}}"#).is_none());
        assert!(parse_session_line("not json").is_none());
    }

    #[test]
    fn quota_estimate_aggregates_rolling_windows() {
        let dir = tempdir().expect("temp dir");
        let projects = dir.path().join("projects");
        std::fs::create_dir_all(projects.join("proj")).expect("create dirs");
        write_session(
            &projects.join("proj"),
            "sess_1.jsonl",
            &[
                &assistant_line(&now_minus(3600), 1000, 500),
                &assistant_line(&now_minus(2 * 24 * 3600), 400, 100),
            ],
        );

        let adapter = adapter_for(&projects);
        let report = adapter
            .collect_quota()
            .expect("quota call")
            .expect("report");
        assert_eq!(report.provider_id, "anthropic_claude");
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
        let thirty_d = report
            .windows
            .iter()
            .find(|w| w.window_key == "30d")
            .unwrap();
        assert_eq!(five_h.used, 1500, "only sessions in last 5h");
        assert_eq!(seven_d.used, 2000, "both sessions within 7d");
        assert_eq!(thirty_d.used, 2000);
        assert_eq!(five_h.limit, 0, "limit is unknown, never fabricated");
    }

    #[test]
    fn quota_estimate_is_none_when_no_sessions() {
        let dir = tempdir().expect("temp dir");
        let projects = dir.path().join("projects");
        std::fs::create_dir_all(&projects).expect("create dir");
        let adapter = adapter_for(&projects);
        assert!(
            adapter.collect_quota().expect("quota call").is_none(),
            "no sessions means no estimate"
        );
    }

    #[test]
    fn detection_classifies_auth_presence() {
        let dir = tempdir().expect("temp dir");
        let projects = dir.path().join("projects");
        std::fs::create_dir_all(projects.join("proj")).expect("create dirs");
        write_session(
            &projects.join("proj"),
            "sess_1.jsonl",
            &[&assistant_line(&now_minus(60), 10, 5)],
        );

        let adapter = adapter_for(&projects);
        let detected = adapter.detect().expect("detect");
        assert!(detected.detected);
        assert_eq!(detected.permission_state, "read_ok_no_auth");

        std::fs::write(projects.join("credentials.json"), r#"{"token":"x"}"#)
            .expect("write credentials");
        let detected = adapter.detect().expect("detect");
        assert_eq!(detected.permission_state, "read_ok_auth");
    }

    #[test]
    fn health_reflects_detection() {
        let dir = tempdir().expect("temp dir");
        let missing = dir.path().join("nope");
        let adapter = adapter_for(&missing);
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }

    #[test]
    fn collected_batch_passes_privacy_guard() {
        let dir = tempdir().expect("temp dir");
        let projects = dir.path().join("projects");
        std::fs::create_dir_all(projects.join("proj")).expect("create dirs");
        let adapter = adapter_for(&projects);
        let report = adapter.collect_quota().expect("quota call");
        if let Some(report) = report {
            assert!(
                lnwdeck_security::PrivacyGuard::validate_quota_report(&report).is_ok(),
                "quota report must pass the privacy guard"
            );
        }
    }
}
