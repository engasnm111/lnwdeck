use rusqlite::Connection;
use serde::Serialize;

/// Persisted provider detection state. Never contains paths, file names or
/// credentials.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderStateRow {
    pub provider_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub detected: bool,
    pub detection_method: String,
    pub source_type: String,
    pub source_exists: bool,
    pub permission_state: String,
    pub adapter_version: String,
    pub last_detection_at: Option<String>,
    pub detection_error_code: String,
}

/// Persisted collector run evidence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectorRunRow {
    pub id: u64,
    pub provider_id: String,
    pub collector_mode: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
    pub source_records_seen: u64,
    pub records_parsed: u64,
    pub events_normalized: u64,
    pub events_rejected: u64,
    pub duplicates_skipped: u64,
    pub events_inserted: u64,
    pub quota_snapshots_inserted: u64,
    pub warning_codes: Vec<String>,
    pub error_code: String,
    pub next_retry_at: Option<String>,
}

/// Pipeline-wide aggregation for the System > Data Pipeline screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PipelineTotals {
    pub events_seen: u64,
    pub events_parsed: u64,
    pub events_normalized: u64,
    pub events_rejected: u64,
    pub duplicates_skipped: u64,
    pub events_inserted: u64,
    pub quota_snapshots_inserted: u64,
    pub privacy_rejections: u64,
    pub last_successful_sync: Option<String>,
    pub next_retry_at: Option<String>,
}

pub struct DiagnosticsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> DiagnosticsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn upsert_provider_state(&self, state: &ProviderStateRow) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO provider_states (
                provider_id, display_name, enabled, detected, detection_method,
                source_type, source_exists, permission_state, adapter_version,
                last_detection_at, detection_error_code
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(provider_id) DO UPDATE SET
                display_name = excluded.display_name,
                enabled = excluded.enabled,
                detected = excluded.detected,
                detection_method = excluded.detection_method,
                source_type = excluded.source_type,
                source_exists = excluded.source_exists,
                permission_state = excluded.permission_state,
                adapter_version = excluded.adapter_version,
                last_detection_at = excluded.last_detection_at,
                detection_error_code = excluded.detection_error_code",
            rusqlite::params![
                state.provider_id,
                state.display_name,
                state.enabled,
                state.detected,
                state.detection_method,
                state.source_type,
                state.source_exists,
                state.permission_state,
                state.adapter_version,
                state.last_detection_at,
                state.detection_error_code,
            ],
        )?;
        Ok(())
    }

    pub fn insert_collector_run(&self, run: &CollectorRunRow) -> Result<(), rusqlite::Error> {
        let warnings = run.warning_codes.join(",");
        self.conn.execute(
            "INSERT INTO collector_runs (
                provider_id, collector_mode, started_at, finished_at, duration_ms,
                source_records_seen, records_parsed, events_normalized, events_rejected,
                duplicates_skipped, events_inserted, quota_snapshots_inserted,
                warning_codes, error_code, next_retry_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                run.provider_id,
                run.collector_mode,
                run.started_at,
                run.finished_at,
                run.duration_ms as i64,
                run.source_records_seen as i64,
                run.records_parsed as i64,
                run.events_normalized as i64,
                run.events_rejected as i64,
                run.duplicates_skipped as i64,
                run.events_inserted as i64,
                run.quota_snapshots_inserted as i64,
                warnings,
                run.error_code,
                run.next_retry_at,
            ],
        )?;
        Ok(())
    }

    pub fn provider_states(&self) -> Result<Vec<ProviderStateRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_id, display_name, enabled, detected, detection_method,
                    source_type, source_exists, permission_state, adapter_version,
                    last_detection_at, detection_error_code
             FROM provider_states
             ORDER BY display_name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProviderStateRow {
                provider_id: row.get(0)?,
                display_name: row.get(1)?,
                enabled: row.get(2)?,
                detected: row.get(3)?,
                detection_method: row.get(4)?,
                source_type: row.get(5)?,
                source_exists: row.get(6)?,
                permission_state: row.get(7)?,
                adapter_version: row.get(8)?,
                last_detection_at: row.get(9)?,
                detection_error_code: row.get(10)?,
            })
        })?;
        let mut states = Vec::new();
        for row in rows {
            states.push(row?);
        }
        Ok(states)
    }

    /// Newest collector run per provider.
    pub fn latest_runs(&self) -> Result<Vec<CollectorRunRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.provider_id, r.collector_mode, r.started_at, r.finished_at,
                    r.duration_ms, r.source_records_seen, r.records_parsed,
                    r.events_normalized, r.events_rejected, r.duplicates_skipped,
                    r.events_inserted, r.quota_snapshots_inserted, r.warning_codes,
                    r.error_code, r.next_retry_at
             FROM collector_runs r
             JOIN (SELECT provider_id, MAX(id) AS max_id
                   FROM collector_runs GROUP BY provider_id) m
               ON r.id = m.max_id
             ORDER BY r.provider_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let warnings: String = row.get(13)?;
            Ok(CollectorRunRow {
                id: row.get(0)?,
                provider_id: row.get(1)?,
                collector_mode: row.get(2)?,
                started_at: row.get(3)?,
                finished_at: row.get(4)?,
                duration_ms: row.get(5)?,
                source_records_seen: row.get(6)?,
                records_parsed: row.get(7)?,
                events_normalized: row.get(8)?,
                events_rejected: row.get(9)?,
                duplicates_skipped: row.get(10)?,
                events_inserted: row.get(11)?,
                quota_snapshots_inserted: row.get(12)?,
                warning_codes: warnings
                    .split(',')
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
                    .collect(),
                error_code: row.get(14)?,
                next_retry_at: row.get(15)?,
            })
        })?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        Ok(runs)
    }

    pub fn pipeline_totals(&self) -> Result<PipelineTotals, rusqlite::Error> {
        let (
            seen,
            parsed,
            normalized,
            rejected,
            duplicates,
            inserted,
            quota,
            last_sync,
            next_retry,
        ): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<String>,
            Option<String>,
        ) = self.conn.query_row(
            "SELECT COALESCE(SUM(source_records_seen), 0),
                    COALESCE(SUM(records_parsed), 0),
                    COALESCE(SUM(events_normalized), 0),
                    COALESCE(SUM(events_rejected), 0),
                    COALESCE(SUM(duplicates_skipped), 0),
                    COALESCE(SUM(events_inserted), 0),
                    COALESCE(SUM(quota_snapshots_inserted), 0),
                    (SELECT MAX(finished_at) FROM collector_runs WHERE error_code = ''),
                    (SELECT MIN(next_retry_at) FROM collector_runs
                     WHERE next_retry_at IS NOT NULL AND next_retry_at > datetime('now'))
             FROM collector_runs",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )?;
        Ok(PipelineTotals {
            events_seen: seen.max(0) as u64,
            events_parsed: parsed.max(0) as u64,
            events_normalized: normalized.max(0) as u64,
            events_rejected: rejected.max(0) as u64,
            duplicates_skipped: duplicates.max(0) as u64,
            events_inserted: inserted.max(0) as u64,
            quota_snapshots_inserted: quota.max(0) as u64,
            privacy_rejections: rejected.max(0) as u64,
            last_successful_sync: last_sync,
            next_retry_at: next_retry,
        })
    }

    /// Number of applied schema migrations.
    pub fn migration_version(&self) -> Result<i64, rusqlite::Error> {
        self.conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
    }
}
