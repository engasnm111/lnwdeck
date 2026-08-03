//! Cursor passive local collector.
//!
//! Cursor stores its editor state in a local SQLite database
//! (`%APPDATA%/Cursor/User/globalStorage/state.vscdb`) whose `ItemTable` holds
//! JSON blobs, some of which carry per-request token counts. This adapter
//! opens that database read-only, walks the stored JSON, and aggregates only
//! numeric token counts, timestamps and model identifiers. Chat text, file
//! contents and paths are never carried out.
//!
//! Cursor does not expose plan limits locally, so quota windows are
//! usage-only: real consumption with an unknown limit.

use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch, DEFAULT_FRESHNESS};
use lnwdeck_provider_runtime::token_scan::{
    extract_from_text, rolling_usage_windows, usage_events, ScanReport, TokenSample,
};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

const PROVIDER_ID: &str = "cursor_ide";
const ADAPTER_VERSION: &str = "0.2.0";
const DATA_SOURCE: &str = "local_sqlite";
/// Upper bound on rows inspected, so a very large state store cannot stall
/// collection.
const MAX_ROWS: usize = 5_000;
/// Upper bound on the size of a single stored blob.
const MAX_VALUE_BYTES: usize = 2 * 1024 * 1024;

pub struct CursorAdapter {
    db_path: PathBuf,
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorAdapter {
    pub fn new() -> Self {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".config"))
            })
            .unwrap_or_default();
        Self::with_db_path(
            base.join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb"),
        )
    }

    /// Adapter pinned to an explicit database path (used by tests and by a
    /// user-configured source).
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn open_read_only(&self) -> Result<Connection, String> {
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "SOURCE_UNAVAILABLE".to_string())
    }

    /// Reads the state store and extracts token samples from the stored JSON
    /// values. A missing `ItemTable` is a schema mismatch, not an empty
    /// result, and is reported as such.
    fn scan(&self) -> Result<ScanReport, String> {
        let conn = self.open_read_only()?;
        let mut stmt = conn
            .prepare("SELECT value FROM ItemTable LIMIT ?1")
            .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        let rows = stmt
            .query_map([MAX_ROWS as i64], |row| row.get::<_, Option<String>>(0))
            .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;

        let mut report = ScanReport::default();
        for row in rows {
            let Ok(Some(value)) = row else {
                continue;
            };
            report.files_seen += 1;
            if value.len() > MAX_VALUE_BYTES {
                report.truncated = true;
                continue;
            }
            report.bytes_read += value.len() as u64;
            let before = report.samples.len();
            extract_from_text(&value, &mut report.samples);
            if report.samples.len() > before {
                report.files_parsed += 1;
            }
        }
        dedupe(&mut report.samples);
        Ok(report)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.db_path.is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Cursor".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_sqlite".to_string(),
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
        match self.scan() {
            Ok(report) if !report.is_empty() => {
                result.detected = true;
                result.permission_state = "read_ok".to_string();
            }
            Ok(_) => {
                result.permission_state = "no_sessions".to_string();
            }
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
            }
        }
        result
    }
}

/// Cursor keeps overlapping copies of the same conversation state, so the same
/// request can appear in several rows. Identical samples are collapsed to
/// avoid double counting real usage.
fn dedupe(samples: &mut Vec<TokenSample>) {
    let mut seen = std::collections::HashSet::new();
    samples.retain(|sample| {
        seen.insert((
            sample.timestamp.timestamp_millis(),
            sample.input_tokens,
            sample.output_tokens,
            sample.model.clone(),
        ))
    });
}

impl ProviderAdapter for CursorAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Cursor",
            vendor: "Anysphere",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let report = self.scan()?;
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
        if !self.db_path.is_file() {
            return Ok(None);
        }
        let report = self.scan()?;
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
                message: "Cursor state store not found".to_string(),
            };
        }
        if !detection.detection_error_code.is_empty() {
            return AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!(
                    "Cursor state store unreadable ({})",
                    detection.detection_error_code
                ),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Cursor state store detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Cursor state store has no token records".to_string(),
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

    fn make_state_db(path: &std::path::Path, values: &[&str]) {
        let conn = Connection::open(path).expect("create db");
        conn.execute(
            "CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value BLOB)",
            [],
        )
        .expect("create table");
        for (index, value) in values.iter().enumerate() {
            conn.execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                rusqlite::params![format!("key_{index}"), value],
            )
            .expect("insert value");
        }
    }

    fn record(minutes_ago: i64, input: u64, output: u64) -> String {
        let ts = (chrono::Utc::now() - chrono::Duration::minutes(minutes_ago)).to_rfc3339();
        format!(
            r#"{{"timestamp":"{ts}","model":"cursor-fast","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}"#
        )
    }

    #[test]
    fn descriptor_is_consistent() {
        let adapter = CursorAdapter::with_db_path(PathBuf::from("Z:/missing.vscdb"));
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.id, "cursor_ide");
        assert_eq!(descriptor.source_kind, SourceKind::LocalSqlite);
        assert!(!descriptor.is_inert());
    }

    #[test]
    fn missing_state_store_is_reported_not_faked() {
        let adapter = CursorAdapter::with_db_path(PathBuf::from("Z:/missing.vscdb"));
        assert_eq!(
            adapter.collect_usage().expect_err("must fail"),
            "SOURCE_UNAVAILABLE"
        );
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }

    #[test]
    fn collects_token_records_from_the_state_store() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        make_state_db(
            &db_path,
            &[
                &record(10, 400, 100),
                r#"{"theme":"dark"}"#,
                &record(45, 20, 5),
            ],
        );
        let adapter = CursorAdapter::with_db_path(db_path);

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].provider_id, "cursor_ide");
        assert_eq!(batch.events[0].model, "cursor-fast");

        let report = adapter.collect_quota().expect("quota").expect("report");
        assert_eq!(report.windows[0].used, 525);
        assert_eq!(report.windows[0].limit, None);
        assert_eq!(report.windows[0].remaining_percent, None);
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Healthy);
    }

    #[test]
    fn duplicate_rows_are_not_counted_twice() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        let entry = record(5, 100, 50);
        make_state_db(&db_path, &[&entry, &entry]);
        let adapter = CursorAdapter::with_db_path(db_path);

        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(
            batch.events.len(),
            1,
            "the same request stored twice must be counted once"
        );
        let report = adapter.collect_quota().expect("quota").expect("report");
        assert_eq!(report.windows[0].used, 150);
    }

    #[test]
    fn unexpected_schema_is_reported_as_a_mismatch() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        let conn = Connection::open(&db_path).expect("create db");
        conn.execute("CREATE TABLE Unrelated (a TEXT)", [])
            .expect("create table");
        drop(conn);

        let adapter = CursorAdapter::with_db_path(db_path);
        assert_eq!(
            adapter.collect_usage().expect_err("must fail"),
            "SOURCE_SCHEMA_MISMATCH"
        );
        assert_eq!(
            adapter.health_check().status,
            AdapterHealthStatus::Unhealthy
        );
    }

    #[test]
    fn state_store_without_token_records_reports_no_data() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("state.vscdb");
        make_state_db(&db_path, &[r#"{"theme":"dark"}"#]);
        let adapter = CursorAdapter::with_db_path(db_path);

        assert!(adapter.collect_usage().expect("usage").events.is_empty());
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Degraded);
    }
}
