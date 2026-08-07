//! Roo Code adapter.
//!
//! Roo Code (rooveterinaryinc.roo-cline) is a Cline fork that writes one
//! `ui_messages.json` per task, exactly like Kilo Code. Unlike Kilo Code it
//! does not persist the model per turn; the sibling
//! `api_conversation_history.json` carries the most recent `<model>` tag
//! inside `<environment_details>` blocks, so the last-seen model in that file
//! is used for the whole task, falling back to `protocol:<apiProtocol>`.
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
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "roo_code";
const ADAPTER_VERSION: &str = "0.1.0";
const EXTENSION: &str = "rooveterinaryinc.roo-cline";
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

pub struct RooAdapter {
    roots: Vec<PathBuf>,
}

impl Default for RooAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RooAdapter {
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

    /// The most recent `<model>` tag in the task's conversation history.
    fn model_from_history(ui_messages_path: &Path) -> Option<String> {
        let history = ui_messages_path
            .parent()?
            .join("api_conversation_history.json");
        let raw = std::fs::read_to_string(history).ok()?;
        if raw.len() > 1_048_576 {
            let naive = &raw[raw.len().saturating_sub(1_048_576)..];
            let block_start = naive.find("<environment_details>").unwrap_or(0);
            return last_model_tag(&naive[block_start..]);
        }
        last_model_tag(&raw)
    }

    fn samples(&self) -> Vec<TokenSample> {
        let mut samples = Vec::new();
        for path in ui_messages::task_files(&self.roots, EXTENSION) {
            let Ok(content) = ui_messages::read_file(&path, MAX_FILE_BYTES) else {
                continue;
            };
            let task_model = Self::model_from_history(&path);
            samples.extend(ui_messages::samples_from_content(&content, &|payload| {
                if let Some(model) = &task_model {
                    return Some(model.clone());
                }
                payload
                    .get("apiProtocol")
                    .and_then(|value| value.as_str())
                    .filter(|protocol| !protocol.trim().is_empty())
                    .map(|protocol| {
                        let slug: String = protocol
                            .to_lowercase()
                            .chars()
                            .filter(|c| {
                                c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_'
                            })
                            .collect();
                        format!("protocol:{slug}")
                    })
            }));
        }
        samples
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.any_root_exists();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Roo Code".to_string(),
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

/// Extracts the last non-empty `<model>...</model>` tag from a history file.
fn last_model_tag(raw: &str) -> Option<String> {
    let mut last = None;
    let mut rest = raw;
    while let Some(start) = rest.find("<model>") {
        let after = &rest[start + "<model>".len()..];
        let Some(end) = after.find("</model>") else {
            break;
        };
        let value = after[..end].trim();
        if !value.is_empty() {
            last = Some(value.to_string());
        }
        rest = &after[end..];
    }
    last
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

impl ProviderAdapter for RooAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Roo Code",
            vendor: "Roo",
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
                message: "Roo Code task history not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Roo Code task records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Roo Code task history has no token records".to_string(),
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
        let task_dir = dir
            .path()
            .join("User")
            .join("globalStorage")
            .join(EXTENSION)
            .join("tasks")
            .join("task-1");
        std::fs::create_dir_all(&task_dir).expect("tasks dir");
        std::fs::write(
            task_dir.join("ui_messages.json"),
            format!(
                r#"[
                {{"say":"api_req_started","ts":{ts},"text":"{{\"tokensIn\":120,\"tokensOut\":30,\"apiProtocol\":\"anthropic\"}}"}}
            ]"#
            ),
        )
        .expect("write");
        std::fs::write(
            task_dir.join("api_conversation_history.json"),
            "<environment_details>\n<model>claude-3-7-sonnet-20250219</model>\n</environment_details>",
        )
        .expect("write history");
        let root = dir.path().to_path_buf();
        (dir, vec![root])
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(RooAdapter::new().id(), "roo_code");
    }

    #[test]
    fn reads_tasks_and_uses_history_model() {
        let (_dir, roots) = fixture_roots();
        let adapter = RooAdapter::with_roots(roots);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].model, "claude-3-7-sonnet-20250219");
        assert_eq!(batch.events[0].tokens_input, 120);
        let report = adapter.collect_quota().expect("quota").expect("report");
        assert!(report.windows[0].used > 0);
    }

    #[test]
    fn missing_history_falls_back_to_protocol() {
        let dir = tempfile::tempdir().expect("temp");
        let task_dir = dir
            .path()
            .join("User")
            .join("globalStorage")
            .join(EXTENSION)
            .join("tasks")
            .join("task-1");
        std::fs::create_dir_all(&task_dir).expect("tasks dir");
        std::fs::write(
            task_dir.join("ui_messages.json"),
            r#"[{"say":"api_req_started","ts":1700000001000,"text":"{\"tokensIn\":1,\"tokensOut\":1,\"apiProtocol\":\"Anthropic\"}"}]"#,
        )
        .expect("write");
        let adapter = RooAdapter::with_roots(vec![dir.path().to_path_buf()]);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events[0].model, "protocol:anthropic");
    }

    #[test]
    fn missing_roots_report_no_data() {
        let dir = tempfile::tempdir().expect("temp");
        let empty = dir.path().join("empty");
        let adapter = RooAdapter::with_roots(vec![empty]);
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
    }

    #[test]
    fn last_model_tag_wins() {
        let raw = "<environment_details><model>claude-3-5-sonnet</model></environment_details>
<environment_details><model>claude-3-7-sonnet</model></environment_details>";
        assert_eq!(last_model_tag(raw).as_deref(), Some("claude-3-7-sonnet"));
        assert_eq!(last_model_tag("no tags"), None);
    }
}
