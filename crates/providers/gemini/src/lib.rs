//! Gemini CLI passive local collector.
//!
//! Gemini CLI keeps one session transcript per chat under
//! `~/.gemini/tmp/<project>/chats/session-*.jsonl`. Every model message in a
//! transcript carries cumulative `tokens` counters, so the usage of one
//! message is the delta between two consecutive counters. This adapter
//! streams those transcripts read-only and aggregates the deltas it finds; it
//! never sends anything to Google and never reads prompt or response text.
//! Because Gemini does not publish plan limits locally, quota windows are
//! usage-only: real consumption with an unknown limit.

use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch, DEFAULT_FRESHNESS};
use lnwdeck_provider_runtime::token_scan::{rolling_usage_windows, usage_events, ScanReport};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

mod transcript;

pub use transcript::TranscriptBounds;

const PROVIDER_ID: &str = "google_gemini";
const ADAPTER_VERSION: &str = "0.3.0";
const DATA_SOURCE: &str = "local_transcripts";

pub struct GeminiAdapter {
    root: PathBuf,
    bounds: TranscriptBounds,
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::with_root(home.join(".gemini"))
    }

    /// Adapter pinned to an explicit root (used by tests and by a
    /// user-configured source directory).
    pub fn with_root(root: PathBuf) -> Self {
        Self {
            root,
            bounds: TranscriptBounds::default(),
        }
    }

    fn scan(&self) -> ScanReport {
        transcript::scan_transcripts(&self.root, &self.bounds)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.root.is_dir();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Gemini".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_scan".to_string(),
            source_type: DATA_SOURCE.to_string(),
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
        let report = self.scan();
        if report.is_empty() {
            result.permission_state = "no_sessions".to_string();
        } else {
            result.detected = true;
            result.permission_state = "read_ok".to_string();
        }
        result
    }
}

impl ProviderAdapter for GeminiAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Gemini",
            vendor: "Google",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        if !self.root.is_dir() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        let report = self.scan();
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", chrono::Utc::now().timestamp()),
            events: usage_events(
                PROVIDER_ID,
                DATA_SOURCE,
                &report.samples,
                Confidence::Medium,
            ),
        })
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        if !self.root.is_dir() {
            return Ok(None);
        }
        let report = self.scan();
        let windows = rolling_usage_windows(&report, chrono::Utc::now(), Confidence::Medium);
        if windows.is_empty() {
            return Ok(None);
        }
        Ok(Some(QuotaReport::new(
            PROVIDER_ID,
            "local_estimate",
            windows,
            DEFAULT_FRESHNESS,
        )))
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Gemini CLI data directory not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Gemini CLI local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Gemini CLI data directory has no token records".to_string(),
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
    use std::io::Write;
    use tempfile::tempdir;

    /// Mirrors the real Gemini CLI session transcript: one JSON document per
    /// line under `~/.gemini/tmp/<project>/chats/session-*.jsonl`, with
    /// cumulative `tokens` on every model message.
    fn session_header(session_id: &str, started: &str) -> String {
        format!(
            r#"{{"sessionId":"{session_id}","projectHash":"a40828760db7f7554d0972c23f489765d3fae0e8561b922230aeb90b19c0cb7d","startTime":"{started}","lastUpdated":"{started}","kind":"main"}}"#
        )
    }

    fn user_message(id: &str, ts: &str, text: &str) -> String {
        format!(
            r#"{{"id":"{id}","timestamp":"{ts}","type":"user","content":[{{"text":"{text}"}}]}}"#
        )
    }

    fn model_message(
        id: &str,
        ts: &str,
        model: &str,
        tokens: (u64, u64, u64, u64, u64, u64),
    ) -> String {
        let (input, output, cached, thoughts, tool, total) = tokens;
        format!(
            r#"{{"id":"{id}","timestamp":"{ts}","type":"gemini","content":"","thoughts":[],"tokens":{{"input":{input},"output":{output},"cached":{cached},"thoughts":{thoughts},"tool":{tool},"total":{total}}},"model":"{model}"}}"#
        )
    }

    fn bookkeeping(ts: &str) -> String {
        format!(r#"{{"$set":{{"lastUpdated":"{ts}"}}}}"#)
    }

    fn write_transcript(root: &std::path::Path, project: &str, name: &str, lines: &[String]) {
        let chats = root.join("tmp").join(project).join("chats");
        std::fs::create_dir_all(&chats).expect("create chats dir");
        let mut file = std::fs::File::create(chats.join(name)).expect("create transcript");
        for line in lines {
            writeln!(file, "{line}").expect("write line");
        }
    }

    #[test]
    fn descriptor_declares_local_estimate_support() {
        let adapter = GeminiAdapter::with_root(PathBuf::from("Z:/missing"));
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.id, "google_gemini");
        assert_eq!(descriptor.usage_support, ChannelSupport::LocalEstimate);
        assert!(!descriptor.is_inert());
        assert!(!descriptor.needs_credentials());
    }

    #[test]
    fn missing_source_is_reported_not_faked() {
        let adapter = GeminiAdapter::with_root(PathBuf::from("Z:/definitely/missing"));
        let err = adapter
            .collect_usage()
            .expect_err("a missing source must be an error, not an empty success");
        assert_eq!(err, "SOURCE_UNAVAILABLE");
        assert!(adapter.collect_quota().expect("quota call").is_none());
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
        assert!(!adapter.detect().expect("detect").detected);
    }

    #[test]
    fn collects_cumulative_token_deltas_from_real_shaped_transcripts() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_transcript(
            &root,
            "topspex",
            "session-2026-05-24T06-55-5b589adf.jsonl",
            &[
                session_header("sess-1", "2026-05-24T06:55:16Z"),
                user_message("u1", "2026-05-24T06:56:28Z", "fix the notification bug"),
                model_message("g1", "2026-05-24T06:56:32Z", "gemini-3.1-pro-preview", (100, 20, 0, 5, 0, 125)),
                bookkeeping("2026-05-24T06:56:33Z"),
                model_message("g2", "2026-05-24T07:00:00Z", "gemini-3.1-pro-preview", (250, 60, 50, 8, 2, 320)),
                r#"{"$set":{"summary":"debug notifications","memoryScratchpad":{"touchedPaths":["public/social/notifications.php"]}}}"#.to_string(),
            ],
        );
        let adapter = GeminiAdapter::with_root(root);

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(
            batch.events.len(),
            1,
            "the first token-bearing message is a baseline, not an event"
        );
        let event = &batch.events[0];
        assert_eq!(event.provider_id, "google_gemini");
        assert_eq!(event.model, "gemini-3.1-pro-preview");
        assert_eq!(event.tokens_input, 150, "delta of cumulative input");
        assert_eq!(
            event.tokens_output, 45,
            "delta of output plus tool and thoughts deltas"
        );
        assert_eq!(event.timestamp.to_rfc3339(), "2026-05-24T07:00:00+00:00");

        let report = adapter.collect_quota().expect("quota").expect("report");
        assert_eq!(report.provider_id, "google_gemini");
        assert_eq!(report.windows.len(), 3);
        for window in &report.windows {
            assert_eq!(window.limit, None, "Gemini publishes no local plan limit");
            assert_eq!(window.remaining_percent, None);
        }
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Healthy);
        assert!(adapter.detect().expect("detect").detected);
    }

    #[test]
    fn token_reset_restarts_the_baseline_instead_of_a_negative_delta() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_transcript(
            &root,
            "p",
            "session-a.jsonl",
            &[
                session_header("s", "2026-05-24T06:55:16Z"),
                model_message(
                    "g1",
                    "2026-05-24T07:00:00Z",
                    "gemini-x",
                    (1000, 200, 0, 0, 0, 1200),
                ),
                model_message(
                    "g2",
                    "2026-05-24T08:00:00Z",
                    "gemini-x",
                    (300, 50, 0, 0, 0, 350),
                ),
            ],
        );
        let adapter = GeminiAdapter::with_root(root);

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(
            batch.events[0].tokens_input, 300,
            "reset restarts the baseline"
        );
        assert_eq!(batch.events[0].tokens_output, 50);
    }

    #[test]
    fn single_document_format_with_messages_array_is_supported() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        let chats = root.join("tmp").join("p").join("chats");
        std::fs::create_dir_all(&chats).expect("create chats dir");
        std::fs::write(
            chats.join("session-old.json"),
            format!(
                r#"{{"sessionId":"s","messages":[{},{}]}}"#,
                model_message(
                    "a",
                    "2026-05-24T07:00:00Z",
                    "gemini-x",
                    (100, 20, 0, 0, 0, 120),
                ),
                model_message(
                    "b",
                    "2026-05-24T07:30:00Z",
                    "gemini-x",
                    (150, 40, 0, 0, 0, 190),
                ),
            ),
        )
        .expect("write single-doc transcript");
        let adapter = GeminiAdapter::with_root(root);

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].tokens_input, 50);
        assert_eq!(batch.events[0].tokens_output, 20);
    }

    #[test]
    fn prompts_responses_and_paths_never_leave_the_transcript() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_transcript(
            &root,
            "p",
            "session-a.jsonl",
            &[
                session_header("s", "2026-05-24T06:55:16Z"),
                user_message("u1", "2026-05-24T07:00:00Z", "my secret prompt with api key sk-abc"),
                model_message("g1", "2026-05-24T07:00:01Z", "gemini-x", (100, 20, 0, 0, 0, 120)),
                model_message("g2", "2026-05-24T07:01:00Z", "gemini-x", (200, 40, 0, 0, 0, 240)),
                r#"{"$set":{"memoryScratchpad":{"touchedPaths":["C:\\Users\\person\\secret-project\\config.php"]}}}"#.to_string(),
            ],
        );
        let adapter = GeminiAdapter::with_root(root);

        let batch = adapter.collect_usage().expect("usage");
        assert!(lnwdeck_security::PrivacyGuard::validate_usage_batch(&batch).is_ok());
        let text = serde_json::to_string(&batch).expect("serialize");
        assert!(!text.contains("secret prompt"));
        assert!(!text.contains("sk-abc"));
        assert!(!text.contains("secret-project"));
        assert!(!text.contains("config.php"));
    }

    #[test]
    fn oversized_transcripts_are_scanned_from_the_tail_and_marked_truncated() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        let chats = root.join("tmp").join("p").join("chats");
        std::fs::create_dir_all(&chats).expect("create chats dir");
        let path = chats.join("session-big.jsonl");
        let mut file = std::fs::File::create(&path).expect("create");
        writeln!(file, "{}", session_header("s", "2026-05-24T06:55:16Z")).expect("header");
        let blob = "x".repeat(2 * 1024 * 1024);
        writeln!(
            file,
            "{}",
            user_message("u1", "2026-05-24T07:00:00Z", &blob)
        )
        .expect("big user message");
        for (i, tokens) in [(100u64, 20u64), (150, 40), (200, 60)].iter().enumerate() {
            let ts = format!("2026-05-24T{:02}:00:00Z", 7 + i as u64);
            writeln!(
                file,
                "{}",
                model_message(
                    &format!("g{i}"),
                    &ts,
                    "gemini-x",
                    (tokens.0, tokens.1, 0, 0, 0, tokens.0 + tokens.1),
                )
            )
            .expect("model line");
        }
        drop(file);

        let mut adapter = GeminiAdapter::with_root(root.clone());
        adapter.bounds = TranscriptBounds {
            max_bytes_per_file: 1024 * 1024,
            ..TranscriptBounds::default()
        };
        let batch = adapter.collect_usage().expect("usage");
        assert!(
            !batch.events.is_empty(),
            "recent usage survives in the read tail"
        );
        let report = adapter.scan();
        assert!(report.truncated, "hitting a bound must be reported");
    }

    #[test]
    fn directory_without_session_transcripts_reports_no_data() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        std::fs::create_dir_all(root.join("tmp").join("p")).expect("create tmp dir");
        std::fs::write(
            root.join("tmp").join("p").join("logs.json"),
            r#"[{"sessionId":"s","messageId":0,"type":"user","message":"/model"}]"#,
        )
        .expect("write logs.json");
        let adapter = GeminiAdapter::with_root(root);

        let batch = adapter.collect_usage().expect("usage");
        assert!(batch.events.is_empty());
        assert!(
            adapter.collect_quota().expect("quota").is_none(),
            "no records must not become zeroed windows"
        );
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }
}
