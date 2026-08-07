//! Kilo Code adapter.
//!
//! Kilo Code (the VS Code extension) persists one Cline-derived
//! `ui_messages.json` per task under the IDE's
//! `User/globalStorage/kilocode.kilo-code/tasks/<uuid>/`. Every
//! `api_req_started` / `api_req_deleted` record carries the billed token
//! counts and the inference provider. The provider is surfaced as the model
//! label (`provider:<slug>`), because Kilo Code persists only the provider
//! per turn — not a model id that could be mapped reliably.
//!
//! Quota is a usage-only local estimate: real consumption, unknown limit.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::token_scan::{usage_events, TokenSample};
use lnwdeck_provider_runtime::ui_messages;
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

const PROVIDER_ID: &str = "kilo_code";
const ADAPTER_VERSION: &str = "0.1.0";
const EXTENSION: &str = "kilocode.kilo-code";
/// A single task history file is capped at 8 MB, matching the shared scanner.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

pub struct KiloCodeAdapter {
    roots: Vec<PathBuf>,
}

impl Default for KiloCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiloCodeAdapter {
    pub fn new() -> Self {
        Self::with_roots(default_ide_roots())
    }

    /// Adapter pinned to explicit IDE roots (used by tests).
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    fn any_root_exists(&self) -> bool {
        self.roots.iter().any(|root| root.is_dir())
    }

    fn samples(&self) -> Vec<TokenSample> {
        let mut samples = Vec::new();
        for path in ui_messages::task_files(&self.roots, EXTENSION) {
            let Ok(content) = ui_messages::read_file(&path, MAX_FILE_BYTES) else {
                continue;
            };
            samples.extend(ui_messages::samples_from_content(&content, &|payload| {
                ui_messages::provider_label(
                    payload.get("inferenceProvider").and_then(|v| v.as_str()),
                )
            }));
        }
        samples
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.any_root_exists();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Kilo Code".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_scan".to_string(),
            source_type: "ui_messages_json".to_string(),
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

/// Default VS Code-family roots on Windows; other platforms fall back to the
/// per-user config directory.
fn default_ide_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        for ide in [
            "Code",
            "Code - Insiders",
            "Cursor",
            "CodeBuddy",
            "Windsurf",
            "VSCodium",
        ] {
            roots.push(appdata.join(ide));
        }
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    for ide in ["Code", "Cursor", "CodeBuddy", "Windsurf", "VSCodium"] {
        roots.push(home.join(".config").join(ide));
    }
    roots
}

impl ProviderAdapter for KiloCodeAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Kilo Code",
            vendor: "kilo.ai",
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
                message: "Kilo Code task history not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Kilo Code task records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Kilo Code task history has no token records".to_string(),
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

    fn fixture_roots() -> (tempfile::TempDir, Vec<PathBuf>) {
        let dir = tempfile::tempdir().expect("temp");
        let ts = chrono::Utc::now().timestamp_millis();
        let tasks = dir
            .path()
            .join("User")
            .join("globalStorage")
            .join(EXTENSION)
            .join("tasks")
            .join("task-1");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        std::fs::write(
            tasks.join("ui_messages.json"),
            format!(
                r#"[
                {{"say":"ask_user","text":"hi","ts":{ts}}},
                {{"say":"api_req_started","ts":{ts},"text":"{{\"tokensIn\":120,\"tokensOut\":30,\"cacheReads\":10,\"cacheWrites\":2,\"inferenceProvider\":\"Moonshot AI\"}}"}}
            ]"#
            ),
        )
        .expect("write");
        let root = dir.path().to_path_buf();
        (dir, vec![root])
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(KiloCodeAdapter::new().id(), "kilo_code");
    }

    #[test]
    fn reads_billed_requests_and_labels_provider() {
        let (_dir, roots) = fixture_roots();
        let adapter = KiloCodeAdapter::with_roots(roots);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].model, "provider:moonshot-ai");
        assert_eq!(batch.events[0].tokens_input, 132);
        assert_eq!(batch.events[0].tokens_output, 30);
        let report = adapter.collect_quota().expect("quota").expect("report");
        assert!(report.windows[0].used > 0);
        assert!(adapter.detection().detected);
    }

    #[test]
    fn missing_roots_report_no_data() {
        let dir = tempfile::tempdir().expect("temp");
        let empty = dir.path().join("empty");
        let adapter = KiloCodeAdapter::with_roots(vec![empty]);
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }
}
