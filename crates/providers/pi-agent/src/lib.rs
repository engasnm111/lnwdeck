//! pi coding agent adapter.
//!
//! Claude-Code-fork session JSONL under `.pi/agent/sessions` is scanned read-only with
//! the shared scanner; only token counts, timestamps and model identifiers
//! are extracted. No provider-published quota source is wired to this adapter,
//! so quota stays explicitly unsupported.

use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch};
use lnwdeck_provider_runtime::token_scan::{scan_directories, usage_events, ScanBounds};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

const PROVIDER_ID: &str = "pi_agent";
const ADAPTER_VERSION: &str = "0.1.0";
const DATA_SOURCE: &str = "local_jsonl";

pub struct PiAdapter {
    roots: Vec<PathBuf>,
    bounds: ScanBounds,
}

impl Default for PiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PiAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::with_roots(vec![home.join(".pi/agent/sessions")])
    }

    /// Adapter pinned to explicit source roots (used by tests).
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            bounds: ScanBounds::default(),
        }
    }

    fn any_root_exists(&self) -> bool {
        self.roots.iter().any(|root| root.is_dir())
    }

    fn scan(&self) -> lnwdeck_provider_runtime::token_scan::ScanReport {
        scan_directories(&self.roots, &self.bounds)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.any_root_exists();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "pi".to_string(),
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
        if self.scan().is_empty() {
            result.permission_state = "no_sessions".to_string();
        } else {
            result.detected = true;
            result.permission_state = "read_ok".to_string();
        }
        result
    }
}

impl ProviderAdapter for PiAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "pi",
            vendor: "pi",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Unsupported,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        if !self.any_root_exists() {
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
        Ok(None)
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "pi local data not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "pi local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "pi local data has no token records".to_string(),
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

    fn write_session(root: &std::path::Path, name: &str) {
        let dir = root.join("projects").join("alpha");
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(
            dir.join(name),
            r#"{"type":"assistant","timestamp":"2027-01-15T10:00:00Z","message":{"usage":{"input_tokens":100,"output_tokens":50},"model":"pi-model"}}"#,
        )
        .expect("write session");
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(PiAdapter::new().id(), "pi_agent");
    }

    #[test]
    fn reads_local_sessions_and_reports_usage() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("tool");
        write_session(&root, "session.jsonl");
        let adapter = PiAdapter::with_roots(vec![root]);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].provider_id, PROVIDER_ID);
        assert_eq!(batch.events[0].tokens_input, 100);
        assert!(
            adapter.collect_quota().expect("quota").is_none(),
            "local pi records are not a published quota"
        );
        assert!(adapter.detection().detected);
    }

    #[test]
    fn missing_root_reports_no_data() {
        let dir = tempfile::tempdir().expect("temp dir");
        let adapter = PiAdapter::with_roots(vec![dir.path().join("missing")]);
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }

    #[test]
    fn requires_filesystem_permission() {
        assert!(PiAdapter::new()
            .required_permissions()
            .contains(&Permission::FileSystem));
    }
}
