//! Mimo Code adapter.
//!
//! Mimo Code is an OpenCode fork at `~/.local/share/mimocode/mimocode.db`
//! that mirrors the user's Claude Code history into its own `message` table.
//! Only turns its own runtime tagged (`providerID` `mimo` or `xiaomi`) are
//! counted; mirrored `anthropic` rows are excluded because the Claude adapter
//! already counts them, and keying off the model id would wrongly re-count
//! mimo-named models the user ran inside Claude Code.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::opencode_fork::{self, MessageSample};
use lnwdeck_provider_runtime::token_scan::{usage_events, ScanBounds};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;

const PROVIDER_ID: &str = "mimo_code";
const ADAPTER_VERSION: &str = "0.1.0";

/// Provider ids Mimo's own runtime tags its native turns with.
const NATIVE_PROVIDERS: &[&str] = &["mimo", "xiaomi"];

pub struct MimoAdapter {
    db_path: PathBuf,
}

impl Default for MimoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MimoAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let db_path = if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            xdg.join("mimocode").join("mimocode.db")
        } else {
            home.join(".local/share/mimocode/mimocode.db")
        };
        Self::with_db_path(db_path)
    }

    /// Adapter pinned to an explicit database path (used by tests).
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn samples(&self) -> Result<Vec<MessageSample>, String> {
        let all = opencode_fork::read_messages(&self.db_path, &ScanBounds::default())?;
        Ok(all
            .into_iter()
            .filter(|sample| {
                sample
                    .provider_id
                    .as_deref()
                    .map(|provider| {
                        let lower = provider.to_lowercase();
                        NATIVE_PROVIDERS.iter().any(|native| lower == *native)
                    })
                    .unwrap_or(false)
            })
            .collect())
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.db_path.is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Mimo Code".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_sqlite".to_string(),
            source_type: "sqlite".to_string(),
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
        match self.samples() {
            Ok(samples) if !samples.is_empty() => {
                result.detected = true;
                result.permission_state = "read_ok".to_string();
            }
            Ok(_) => result.permission_state = "no_sessions".to_string(),
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
            }
        }
        result
    }

    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        if !self.db_path.is_file() {
            return Ok(None);
        }
        let samples = opencode_fork::to_token_samples(&self.samples()?);
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

impl ProviderAdapter for MimoAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Mimo Code",
            vendor: "Xiaomi",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let samples = self.samples()?;
        if samples.is_empty() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", chrono::Utc::now().timestamp()),
            events: usage_events(
                PROVIDER_ID,
                "local_sqlite",
                &opencode_fork::to_token_samples(&samples),
                Confidence::Medium,
            ),
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
                message: "Mimo Code local data not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Mimo Code local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Mimo Code local data has no native turns".to_string(),
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
    use rusqlite::Connection;
    use std::path::Path;

    fn write_db(path: &Path) {
        let conn = Connection::open(path).expect("open");
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
                message("m1", "mimo", "mimo-v2.5-pro"),
                message("m2", "xiaomi", "mimo-v2.5-pro"),
                message("m3", "anthropic", "claude-3-7-sonnet"),
            ],
        )
        .expect("rows");
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(MimoAdapter::new().id(), "mimo_code");
    }

    #[test]
    fn counts_only_native_mimo_turns() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("mimocode.db");
        write_db(&path);
        let adapter = MimoAdapter::with_db_path(path);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(
            batch.events.len(),
            2,
            "mirrored anthropic turns are excluded"
        );
        assert!(batch.events.iter().all(|e| e.provider_id == PROVIDER_ID));
        assert!(
            batch.events.iter().all(|e| e.model == "mimo-v2.5-pro"),
            "a mimo model run inside Claude Code is not re-counted"
        );
    }

    #[test]
    fn missing_store_reports_no_data() {
        let dir = tempfile::tempdir().expect("temp");
        let adapter = MimoAdapter::with_db_path(dir.path().join("no.db"));
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
    }
}
