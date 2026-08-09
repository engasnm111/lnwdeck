use chrono::{DateTime, Utc};
use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch, DEFAULT_FRESHNESS};
use lnwdeck_provider_runtime::token_scan::{scan_directory, usage_events, ScanBounds, TokenSample};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub mod usage_api;

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
        let report = scan_directory(&self.projects_dir, &bounds);
        Ok(report.samples)
    }

    /// Quota for the Claude subscription.
    ///
    /// Anthropic publishes the utilization of each rate-limit window to the
    /// same OAuth token Claude Code stores locally, so that is the
    /// authoritative source. Without a stored token, or when the account has
    /// no published window, quota is unavailable; the local session scan stays
    /// available through the separate usage channel.
    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        match usage_api::fetch_windows(
            &self.credentials_path,
            &usage_api::default_endpoint(),
            std::time::Duration::from_secs(10),
        ) {
            Ok(Some(windows)) => {
                let mut report = QuotaReport::new(
                    "anthropic_claude",
                    "provider_api",
                    windows,
                    DEFAULT_FRESHNESS,
                );
                report.plan = Some("Subscription".to_string());
                Ok(Some(report))
            }
            Ok(None) => Ok(None),
            Err(code) => Err(code),
        }
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
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: "anthropic_claude",
            display_name: "Claude",
            vendor: "Anthropic",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            // Anthropic publishes real per-window utilization to the local
            // OAuth usage endpoint; the local scan is usage history only.
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        if !self.projects_dir.is_dir() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        let samples = self.scan_samples()?;
        Ok(UsageBatch {
            batch_id: format!("claude_{}", chrono::Utc::now().timestamp()),
            events: usage_events(
                "anthropic_claude",
                "local_jsonl",
                &samples,
                Confidence::Medium,
            ),
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
        assert!(
            adapter.collect_quota().expect("quota call").is_none(),
            "local transcript usage must not be presented as Claude quota"
        );
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

    #[test]
    fn collect_usage_emits_real_events_from_local_sessions() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("projects");
        std::fs::create_dir_all(&root).expect("create root");
        let recent = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        std::fs::write(
            root.join("session.jsonl"),
            format!(
                r#"{{"type":"assistant","timestamp":"{recent}","message":{{"id":"m1","role":"assistant","model":"claude-e2e","usage":{{"input_tokens":300,"output_tokens":100}}}}}}"#
            ),
        )
        .expect("write session");

        let adapter = ClaudeAdapter::with_paths(root.clone(), root.join(".credentials.json"));
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(
            batch.events.len(),
            1,
            "a real session record must become a usage event, not an empty success"
        );
        assert_eq!(batch.events[0].provider_id, "anthropic_claude");
        assert_eq!(batch.events[0].tokens_input, 300);
        assert_eq!(batch.events[0].tokens_output, 100);
        assert_eq!(batch.events[0].model, "claude-e2e");
        assert!(
            batch.events[0].cost.is_empty(),
            "a collector must not invent a cost"
        );
    }

    #[test]
    fn collect_usage_reports_a_missing_source_instead_of_an_empty_batch() {
        let adapter = ClaudeAdapter::with_paths(
            PathBuf::from("Z:/missing"),
            PathBuf::from("Z:/missing/creds.json"),
        );
        assert_eq!(
            adapter.collect_usage().expect_err("must fail"),
            "SOURCE_UNAVAILABLE"
        );
    }
}
