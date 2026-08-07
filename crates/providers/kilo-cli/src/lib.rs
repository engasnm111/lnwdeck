//! Kilo CLI adapter.
//!
//! kilo.ai's CLI is an OpenCode fork that keeps the identical OpenCode
//! SQLite schema at `~/.local/share/kilo/kilo.db`. Every assistant row in the
//! `message` table is a billed turn, so the shared OpenCode-fork reader is
//! used without a provider filter. Quota is a usage-only local estimate with
//! real consumption and an unknown limit.

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

const PROVIDER_ID: &str = "kilo_cli";
const ADAPTER_VERSION: &str = "0.1.0";

pub struct KiloCliAdapter {
    db_path: PathBuf,
}

impl Default for KiloCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiloCliAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let db_path = if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            xdg.join("kilo").join("kilo.db")
        } else {
            home.join(".local/share/kilo/kilo.db")
        };
        Self::with_db_path(db_path)
    }

    /// Adapter pinned to an explicit database path (used by tests).
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn samples(&self) -> Result<Vec<MessageSample>, String> {
        opencode_fork::read_messages(&self.db_path, &ScanBounds::default())
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.db_path.is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Kilo CLI".to_string(),
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

impl ProviderAdapter for KiloCliAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Kilo CLI",
            vendor: "kilo.ai",
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
                message: "Kilo CLI local data not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Kilo CLI local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Kilo CLI local data has no token records".to_string(),
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

    #[test]
    fn id_is_correct() {
        assert_eq!(KiloCliAdapter::new().id(), "kilo_cli");
    }

    #[test]
    fn reads_all_assistant_rows() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("kilo.db");
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY, session_id TEXT,
                time_created INTEGER, time_updated INTEGER, data TEXT
             );",
        )
        .expect("schema");
        let now_ms = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES ('m1','s',?2,?2,?1), ('m2','s',?2,?2,?1)",
            rusqlite::params![
                format!(
                    r#"{{"id":"m","providerID":"kilo","modelID":"kilo-model","role":"assistant","tokens":{{"input":100,"output":50,"reasoning":0}},"time":{{"created":{now_ms}}}}}"#
                ),
                now_ms - 3_600_000,
            ],
        )
        .expect("rows");
        drop(conn);
        let adapter = KiloCliAdapter::with_db_path(path);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert!(batch.events.iter().all(|e| e.provider_id == PROVIDER_ID));
        let report = adapter.collect_quota().expect("quota").expect("report");
        assert_eq!(report.windows.len(), 3);
        assert!(report.windows[0].used > 0);
    }

    #[test]
    fn missing_store_reports_no_data() {
        let dir = tempfile::tempdir().expect("temp");
        let adapter = KiloCliAdapter::with_db_path(dir.path().join("no.db"));
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }
}
