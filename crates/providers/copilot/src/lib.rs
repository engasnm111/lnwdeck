//! GitHub Copilot passive local collector.
//!
//! Reads token counts from the Copilot CLI and editor log directories present
//! on the machine (`~/.copilot`, VS Code logs, Copilot application data).
//! Logs are opened read-only and only numeric token counts, timestamps and
//! model identifiers are extracted; log message text is never carried out.
//! Subscription quota is collected separately from Copilot's authenticated
//! account endpoint using the OAuth session already stored by Copilot.

use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch};
use lnwdeck_provider_runtime::token_scan::{
    scan_directories, usage_events, ScanBounds, ScanReport,
};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

mod quota_api;

const PROVIDER_ID: &str = "github_copilot";
const ADAPTER_VERSION: &str = "0.3.0";
const DATA_SOURCE: &str = "local_log";

pub struct CopilotAdapter {
    roots: Vec<PathBuf>,
    home: PathBuf,
    bounds: ScanBounds,
}

impl Default for CopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CopilotAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let mut roots = vec![home.join(".copilot")];
        if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
            roots.push(appdata.join("Code").join("logs"));
            roots.push(appdata.join("GitHub Copilot"));
        }
        Self::with_paths(roots, home)
    }

    /// Adapter pinned to explicit source roots (used by tests and by a
    /// user-configured source directory).
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        let home = roots
            .first()
            .and_then(|root| root.parent())
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::with_paths(roots, home)
    }

    fn with_paths(roots: Vec<PathBuf>, home: PathBuf) -> Self {
        Self {
            roots,
            home,
            bounds: ScanBounds::default(),
        }
    }

    fn any_root_exists(&self) -> bool {
        self.roots.iter().any(|root| root.is_dir())
    }

    fn scan(&self) -> ScanReport {
        scan_directories(&self.roots, &self.bounds)
    }

    fn detection(&self) -> DetectionResult {
        let usage_source_exists = self.any_root_exists();
        let has_auth = quota_api::read_oauth_token(&self.home).is_some();
        let source_exists = usage_source_exists || has_auth;
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Copilot".to_string(),
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
        if !usage_source_exists {
            result.detected = has_auth;
            result.permission_state = "auth_only".to_string();
        } else if self.scan().is_empty() {
            result.detected = has_auth;
            result.permission_state = if has_auth {
                "read_ok_auth".to_string()
            } else {
                "no_sessions".to_string()
            };
        } else {
            result.detected = true;
            result.permission_state = if has_auth {
                "read_ok_auth".to_string()
            } else {
                "read_ok".to_string()
            };
        }
        result
    }
}

impl ProviderAdapter for CopilotAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Copilot",
            vendor: "GitHub",
            source_kind: SourceKind::LocalLog,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Native,
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
        quota_api::fetch_report(&self.home, std::time::Duration::from_secs(10))
    }

    fn account_identity(&self) -> Option<String> {
        quota_api::read_oauth_token(&self.home)
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Copilot local data not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Copilot local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Copilot local data has no token records".to_string(),
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
    use tempfile::tempdir;

    fn recent_record(minutes_ago: i64, input: u64, output: u64) -> String {
        let ts = (chrono::Utc::now() - chrono::Duration::minutes(minutes_ago)).to_rfc3339();
        format!(
            r#"{{"timestamp":"{ts}","model":"copilot-chat","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}"#
        )
    }

    #[test]
    fn descriptor_is_consistent_and_declares_native_quota_support() {
        let adapter = CopilotAdapter::with_roots(vec![PathBuf::from("Z:/missing")]);
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.id, PROVIDER_ID);
        assert_eq!(descriptor.usage_support, ChannelSupport::LocalEstimate);
        assert_eq!(descriptor.quota_support, ChannelSupport::Native);
        assert!(!descriptor.is_inert());
    }

    #[test]
    fn missing_source_reports_an_error_instead_of_empty_success() {
        let adapter = CopilotAdapter::with_roots(vec![PathBuf::from("Z:/definitely/missing")]);
        assert_eq!(
            adapter
                .collect_usage()
                .expect_err("missing source must fail"),
            "SOURCE_UNAVAILABLE"
        );
        assert!(adapter.collect_quota().expect("quota call").is_none());
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }

    #[test]
    fn auth_only_source_does_not_fake_local_usage() {
        let dir = tempdir().expect("temp dir");
        let auth_dir = dir.path().join(".config").join("github-copilot");
        std::fs::create_dir_all(&auth_dir).expect("create auth dir");
        std::fs::write(
            auth_dir.join("apps.json"),
            r#"{"github.com:copilot":{"oauth_token":"gho_123456789012345678901234567890"}}"#,
        )
        .expect("write auth");
        let adapter = CopilotAdapter::with_paths(
            vec![PathBuf::from("Z:/definitely/missing")],
            dir.path().to_path_buf(),
        );

        assert!(adapter.detect().expect("detect").detected);
        assert_eq!(
            adapter
                .collect_usage()
                .expect_err("auth alone is not a local usage source"),
            "SOURCE_UNAVAILABLE"
        );
    }

    #[test]
    fn collects_real_records_from_local_files() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("data");
        std::fs::create_dir_all(root.join("sessions")).expect("create dirs");
        std::fs::write(
            root.join("sessions").join("history.jsonl"),
            format!(
                "{}\n{}",
                recent_record(5, 300, 100),
                recent_record(20, 10, 5)
            ),
        )
        .expect("write history");

        let adapter = CopilotAdapter::with_roots(vec![root]);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].provider_id, PROVIDER_ID);
        assert_eq!(batch.events[0].model, "copilot-chat");

        assert!(
            adapter.collect_quota().expect("quota").is_none(),
            "local Copilot records are not a published quota"
        );
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Healthy);
    }

    #[test]
    fn source_without_token_records_reports_no_data() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("data");
        std::fs::create_dir_all(&root).expect("create dir");
        std::fs::write(root.join("config.json"), r#"{{"enabled":true}}"#).expect("write config");

        let adapter = CopilotAdapter::with_roots(vec![root]);
        assert!(adapter.collect_usage().expect("usage").events.is_empty());
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }
}
