//! Z.AI (GLM) adapter.
//!
//! The Z.AI GLM Coding Plan is used from Claude Code, OpenCode, Cline and
//! Kilo Code with Z.AI endpoints, and Z.AI publishes no server-side usage
//! API for plain API keys (its OpenAPI exposes only model endpoints). This
//! adapter therefore estimates Z.AI usage from the local session stores of
//! the tools it runs inside:
//!
//! - Claude Code session JSONL (`~/.claude/projects/**/*.jsonl`) โ€” GLM models
//!   only appear there when Claude Code is pointed at a Z.AI gateway;
//! - the OpenCode message table (`~/.local/share/opencode/opencode.db`) โ€”
//!   GLM turns whose provider is not `opencode-go` (OpenCode's own
//!   subscription, counted by the OpenCode adapter).
//!
//! Only GLM model identifiers are counted, so a Claude or Gemini model in the
//! same files is never misattributed to Z.AI. This adapter exposes usage only;
//! it does not claim a quota source for plain API keys. When the ZCode adapter
//! is installed it reports the real coding-plan quota for the same account.

use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch};
use lnwdeck_provider_runtime::opencode_fork;
use lnwdeck_provider_runtime::token_scan::{scan_directory, usage_events, ScanBounds, TokenSample};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

const PROVIDER_ID: &str = "zai_glm";
const ADAPTER_VERSION: &str = "0.1.0";

/// OpenCode's own managed subscription. Its GLM turns are billed to
/// opencode-go, not to the Z.AI plan, and the OpenCode adapter already
/// counts them, so they are excluded here.
const OPENCODE_GO_PROVIDER: &str = "opencode-go";

/// True when the model identifier belongs to the Z.AI GLM family. The JSON
/// descriptor form (`{"id":"glm-5.2","providerID":"..."}`) is normalized by
/// the shared scanner before this check runs.
pub fn is_glm_model(model: &str) -> bool {
    model.to_lowercase().starts_with("glm-")
}

pub struct ZaiAdapter {
    projects_dir: PathBuf,
    opencode_db: PathBuf,
}

impl Default for ZaiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZaiAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let opencode_db = if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            xdg.join("opencode").join("opencode.db")
        } else {
            home.join(".local/share/opencode/opencode.db")
        };
        Self::with_paths(home.join(".claude/projects"), opencode_db)
    }

    /// Adapter pinned to explicit local paths (used by tests).
    pub fn with_paths(projects_dir: PathBuf, opencode_db: PathBuf) -> Self {
        Self {
            projects_dir,
            opencode_db,
        }
    }

    fn any_source_exists(&self) -> bool {
        self.projects_dir.is_dir() || self.opencode_db.is_file()
    }

    /// GLM samples from Claude Code session JSONL, filtered by model family.
    fn claude_samples(&self) -> Vec<TokenSample> {
        if !self.projects_dir.is_dir() {
            return Vec::new();
        }
        let report = scan_directory(&self.projects_dir, &ScanBounds::default());
        report
            .samples
            .into_iter()
            .filter(|sample| sample.model.as_deref().map(is_glm_model).unwrap_or(false))
            .collect()
    }

    /// GLM samples from the OpenCode message table. opencode-go turns are
    /// excluded so they are not double-counted against the OpenCode adapter.
    fn opencode_samples(&self) -> Vec<TokenSample> {
        if !self.opencode_db.is_file() {
            return Vec::new();
        }
        let Ok(messages) = opencode_fork::read_messages(&self.opencode_db, &ScanBounds::default())
        else {
            return Vec::new();
        };
        messages
            .into_iter()
            .filter(|sample| {
                let provider_ok = sample
                    .provider_id
                    .as_deref()
                    .map(|provider| !provider.eq_ignore_ascii_case(OPENCODE_GO_PROVIDER))
                    .unwrap_or(true);
                let model_ok = sample.model.as_deref().map(is_glm_model).unwrap_or(false);
                provider_ok && model_ok
            })
            .map(|sample| TokenSample {
                timestamp: sample.timestamp,
                input_tokens: sample.input_tokens,
                output_tokens: sample.output_tokens,
                model: sample.model,
            })
            .collect()
    }

    fn samples(&self) -> Vec<TokenSample> {
        let mut samples = self.claude_samples();
        samples.extend(self.opencode_samples());
        samples
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.any_source_exists();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Z.AI".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_scan".to_string(),
            source_type: "local_jsonl_sqlite".to_string(),
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
}

impl ProviderAdapter for ZaiAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Z.AI",
            vendor: "Zhipu AI",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Unsupported,
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
        Ok(None)
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Z.AI local sources not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "GLM local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "No GLM records found in local sessions".to_string(),
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
        assert_eq!(ZaiAdapter::new().id(), "zai_glm");
    }

    #[test]
    fn glm_model_detection_is_case_insensitive() {
        assert!(is_glm_model("glm-5.2"));
        assert!(is_glm_model("GLM-4.7"));
        assert!(is_glm_model("glm-4.5-air"));
        assert!(!is_glm_model("claude-3-7-sonnet"));
        assert!(!is_glm_model("gemini-3-pro"));
        assert!(!is_glm_model("glm"));
    }

    #[test]
    fn claude_sessions_count_only_glm_models() {
        let dir = tempfile::tempdir().expect("temp dir");
        let projects = dir.path().join("projects").join("alpha");
        std::fs::create_dir_all(&projects).expect("create");
        let line = |model: &str, ts: &str, input: u64, output: u64| {
            format!(
                r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"m","role":"assistant","model":"{model}","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}"#
            )
        };
        std::fs::write(
            projects.join("session.jsonl"),
            [
                line("glm-5.2", "2026-08-08T00:00:00Z", 100, 50),
                line("claude-3-7-sonnet", "2026-08-08T00:01:00Z", 999, 999),
                line("glm-4.7", "2026-08-08T00:02:00Z", 10, 5),
            ]
            .join("\n"),
        )
        .expect("write");

        let adapter =
            ZaiAdapter::with_paths(dir.path().join("projects"), dir.path().join("missing.db"));
        let samples = adapter.claude_samples();
        assert_eq!(samples.len(), 2, "claude model rows are not Z.AI usage");
        assert!(samples.iter().all(
            |s| s.model.as_deref() == Some("glm-5.2") || s.model.as_deref() == Some("glm-4.7")
        ));

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert!(batch.events.iter().all(|e| e.provider_id == PROVIDER_ID));
    }

    #[test]
    fn opencode_sessions_exclude_opencode_go_turns() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).expect("open");
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY, session_id TEXT,
                time_created INTEGER, time_updated INTEGER, data TEXT
             );",
        )
        .expect("schema");
        let message = |id: &str, provider: &str, model: &str| {
            format!(
                r#"{{"id":"{id}","providerID":"{provider}","modelID":"{model}","role":"assistant","tokens":{{"input":10,"output":5,"reasoning":0}},"time":{{"created":1700000000000}}}}"#
            )
        };
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES ('m1','s',1700000000000,1700000000000,?1),
                    ('m2','s',1700000000000,1700000001000,?2),
                    ('m3','s',1700000000000,1700000002000,?3)",
            rusqlite::params![
                message("m1", "zai", "glm-5.2"),
                message("m2", "opencode-go", "glm-5.2"),
                message("m3", "anthropic", "claude-3-7-sonnet"),
            ],
        )
        .expect("rows");
        drop(conn);

        let adapter = ZaiAdapter::with_paths(dir.path().join("no-projects"), db_path);
        let samples = adapter.opencode_samples();
        assert_eq!(samples.len(), 1, "go-plan and claude turns are excluded");
        assert_eq!(samples[0].model.as_deref(), Some("glm-5.2"));
    }

    #[test]
    fn missing_sources_report_no_data() {
        let dir = tempfile::tempdir().expect("temp dir");
        let adapter =
            ZaiAdapter::with_paths(dir.path().join("no-projects"), dir.path().join("no.db"));
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }

    #[test]
    fn quota_windows_are_usage_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let projects = dir.path().join("projects");
        std::fs::create_dir_all(&projects).expect("create");
        let ts = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        std::fs::write(
            projects.join("s.jsonl"),
            format!(
                r#"{{"type":"assistant","timestamp":"{ts}","message":{{"usage":{{"input_tokens":100,"output_tokens":50}},"model":"glm-5.2"}}}}"#
            ),
        )
        .expect("write");
        let adapter = ZaiAdapter::with_paths(projects, dir.path().join("no.db"));
        assert!(
            adapter.collect_quota().expect("quota").is_none(),
            "local Z.AI records are not a published quota"
        );
    }
}
