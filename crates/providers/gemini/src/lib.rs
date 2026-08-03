//! Gemini CLI passive local collector.
//!
//! The Gemini CLI keeps its session and telemetry records under `~/.gemini`.
//! This adapter reads those files read-only and aggregates the token counts it
//! finds; it never sends anything to Google and never reads prompt or response
//! text. Because Gemini does not publish plan limits locally, quota windows
//! are usage-only: real consumption with an unknown limit.

use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch, DEFAULT_FRESHNESS};
use lnwdeck_provider_runtime::token_scan::{
    rolling_usage_windows, scan_directory, usage_events, ScanBounds, ScanReport,
};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

const PROVIDER_ID: &str = "google_gemini";
const ADAPTER_VERSION: &str = "0.2.0";
const DATA_SOURCE: &str = "local_jsonl";

pub struct GeminiAdapter {
    root: PathBuf,
    bounds: ScanBounds,
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
            bounds: ScanBounds::default(),
        }
    }

    fn scan(&self) -> ScanReport {
        scan_directory(&self.root, &self.bounds)
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
    use tempfile::tempdir;

    fn write_session(root: &std::path::Path, name: &str, content: &str) {
        std::fs::create_dir_all(root).expect("create root");
        std::fs::write(root.join(name), content).expect("write session file");
    }

    fn recent_record(minutes_ago: i64, input: u64, output: u64) -> String {
        let ts = (chrono::Utc::now() - chrono::Duration::minutes(minutes_ago)).to_rfc3339();
        format!(
            r#"{{"timestamp":"{ts}","response":{{"modelId":"gemini-3-pro","usageMetadata":{{"promptTokenCount":{input},"candidatesTokenCount":{output}}}}}}}"#
        )
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
    fn collects_real_token_records_from_local_files() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_session(
            &root.join("tmp").join("project"),
            "logs.json",
            &format!(
                "{}\n{}",
                recent_record(10, 200, 80),
                recent_record(30, 5, 5)
            ),
        );
        let adapter = GeminiAdapter::with_root(root);

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].provider_id, "google_gemini");
        assert_eq!(batch.events[0].model, "gemini-3-pro");
        let total: u64 = batch
            .events
            .iter()
            .map(|e| e.tokens_input + e.tokens_output)
            .sum();
        assert_eq!(total, 290);

        let report = adapter.collect_quota().expect("quota").expect("report");
        assert_eq!(report.provider_id, "google_gemini");
        assert_eq!(report.source, "local_estimate");
        assert_eq!(report.windows.len(), 3);
        let five_h = &report.windows[0];
        assert_eq!(five_h.used, 290);
        assert_eq!(five_h.limit, None, "Gemini publishes no local plan limit");
        assert_eq!(five_h.remaining_percent, None);

        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Healthy);
        let detection = adapter.detect().expect("detect");
        assert!(detection.detected);
        assert_eq!(detection.permission_state, "read_ok");
    }

    #[test]
    fn directory_without_token_records_reports_no_data() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_session(&root, "settings.json", r#"{"theme":"dark"}"#);
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
