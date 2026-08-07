//! Hermes Agent adapter.
//!
//! Hermes keeps a sessions table in `state.db` (`~/.hermes/state.db`, or
//! `%LOCALAPPDATA%\hermes\state.db` on Windows). Each row carries the model
//! and the session totals for input, output, cache-read, cache-write and
//! reasoning tokens, updated in real time. Only those numeric counters and
//! the session timestamps are read; nothing else leaves the database.
//!
//! Quota is a usage-only local estimate: real consumption, unknown limit.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::token_scan::{usage_events, TokenSample};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

const PROVIDER_ID: &str = "hermes";
const ADAPTER_VERSION: &str = "0.1.0";

pub struct HermesAdapter {
    db_path: PathBuf,
}

impl Default for HermesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let db_path =
            if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
                let candidate = local_appdata.join("hermes").join("state.db");
                if candidate.is_file() {
                    candidate
                } else {
                    home.join(".hermes").join("state.db")
                }
            } else {
                home.join(".hermes").join("state.db")
            };
        Self::with_db_path(db_path)
    }

    /// Adapter pinned to an explicit database path (used by tests).
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn open_read_only(&self) -> Result<Connection, String> {
        if !self.db_path.is_file() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "SOURCE_UNAVAILABLE".to_string())
    }

    fn samples(&self) -> Result<Vec<TokenSample>, String> {
        let conn = self.open_read_only()?;
        let mut stmt = conn
            .prepare(
                "SELECT started_at, input_tokens, output_tokens, cache_read_tokens,
                        cache_write_tokens, reasoning_tokens, model
                 FROM sessions
                 WHERE (input_tokens > 0 OR output_tokens > 0 OR cache_read_tokens > 0
                        OR cache_write_tokens > 0 OR reasoning_tokens > 0)
                 ORDER BY started_at",
            )
            .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;

        let mut samples = Vec::new();
        for row in rows {
            let Ok((started_at, input, output, cache_read, cache_write, reasoning, model)) = row
            else {
                continue;
            };
            let Some(timestamp) = chrono::DateTime::from_timestamp(started_at, 0) else {
                continue;
            };
            samples.push(TokenSample {
                timestamp,
                input_tokens: input.max(0) as u64,
                output_tokens: (output + reasoning).max(0) as u64,
                model,
            });
            let _ = cache_read;
            let _ = cache_write;
        }
        Ok(samples)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.db_path.is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Hermes".to_string(),
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
        let samples = self.samples()?;
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

impl ProviderAdapter for HermesAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Hermes",
            vendor: "Nous Research",
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
            events: usage_events(PROVIDER_ID, "local_sqlite", &samples, Confidence::Medium),
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
                message: "Hermes state.db not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Hermes sessions detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Hermes state.db has no token records".to_string(),
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
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, model TEXT, started_at INTEGER, ended_at INTEGER,
                input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER,
                cache_write_tokens INTEGER, reasoning_tokens INTEGER, message_count INTEGER
             );",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO sessions VALUES
             ('s1', 'hermes-4-405b', 1700000000, 1700000100, 100, 50, 10, 2, 5, 3),
             ('s2', 'hermes-4-405b', 1700000200, 1700000300, 0, 0, 0, 0, 0, 1)",
            [],
        )
        .expect("rows");
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(HermesAdapter::new().id(), "hermes");
    }

    #[test]
    fn reads_session_totals() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("state.db");
        write_db(&path);
        let adapter = HermesAdapter::with_db_path(path);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 1, "zero-token sessions are dropped");
        assert_eq!(batch.events[0].model, "hermes-4-405b");
        assert_eq!(batch.events[0].tokens_input, 100);
        assert_eq!(batch.events[0].tokens_output, 55);
        assert!(adapter.detection().detected);
    }

    #[test]
    fn missing_store_reports_no_data() {
        let dir = tempfile::tempdir().expect("temp");
        let adapter = HermesAdapter::with_db_path(dir.path().join("no.db"));
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }

    #[test]
    fn wrong_schema_is_reported_not_guessed() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("state.db");
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch("CREATE TABLE other (id INTEGER);")
            .expect("schema");
        drop(conn);
        let adapter = HermesAdapter::with_db_path(path);
        assert_eq!(
            adapter.collect_usage(),
            Err("SOURCE_SCHEMA_MISMATCH".to_string())
        );
    }
}
