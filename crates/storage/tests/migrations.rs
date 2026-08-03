use lnwdeck_storage::{migrations::apply_all, Storage};
use std::fs;
use tempfile::tempdir;

#[test]
fn round_trip_empty_database() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");

    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("apply migrations");

    let tables = storage.list_tables().expect("list tables");
    assert!(
        tables.contains(&"usage_events".to_string()),
        "usage_events table must exist"
    );
    assert!(
        tables.contains(&"quota_snapshots".to_string()),
        "quota_snapshots table must exist"
    );
    assert!(
        tables.contains(&"quota_reports".to_string()),
        "quota_reports table must exist"
    );
    assert!(
        tables.contains(&"quota_windows".to_string()),
        "quota_windows table must exist"
    );
    assert!(
        tables.contains(&"sync_cursors".to_string()),
        "sync_cursors table must exist"
    );
}

#[test]
fn integrity_check_passes() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");

    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("apply migrations");

    storage.integrity_check().expect("integrity check");
}

#[test]
fn create_backup_of_existing_database() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let backup_path = dir.path().join("test_backup.db");

    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("apply migrations");

    let conn = &storage.conn;
    conn.execute(
        "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, data_source, cost)
         VALUES ('evt_1', 'batch_1', '2025-01-01T00:00:00Z', 'openai', 'gpt-4o', 100, 50, 'High', 'web', '0.005')",
        [],
    )
    .expect("insert");

    storage.backup_to(&backup_path).expect("backup");

    assert!(backup_path.exists(), "backup file must exist");
    assert!(
        fs::metadata(&backup_path).unwrap().len() > 0,
        "backup file must not be empty"
    );
}

#[test]
fn migrations_are_idempotent() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");

    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("first apply");
    apply_all(&storage.conn).expect("second apply");
}

/// Upgrade path: a database created before migration 004 stored an unknown
/// quota limit as 0 with remaining_percent = 100. After migrating, those rows
/// must read as unknown (NULL), never as "full quota remaining".
#[test]
fn migration_004_converts_zero_limits_to_unknown() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("legacy.db");
    let storage = Storage::open(&db_path).expect("open");

    let migrations_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for name in [
        "000_schema_tracking.sql",
        "001_initial.sql",
        "002_collector_diagnostics.sql",
        "003_quota_reports.sql",
    ] {
        let sql = std::fs::read_to_string(migrations_dir.join(name)).expect("read migration");
        storage.conn.execute_batch(&sql).expect("apply legacy sql");
        storage
            .conn
            .execute(
                "INSERT OR REPLACE INTO schema_migrations (version) VALUES (?1)",
                [name.trim_end_matches(".sql")],
            )
            .expect("record legacy version");
    }

    storage
        .conn
        .execute(
            "INSERT INTO quota_reports
                (provider_id, account_fingerprint, plan, status, source, collected_at, stale_at, error_code)
             VALUES ('legacy', '', '', 'fresh', 'legacy_api', '2026-01-01T00:00:00+00:00', '2026-01-01T01:00:00+00:00', NULL)",
            [],
        )
        .expect("legacy report");
    storage
        .conn
        .execute(
            "INSERT INTO quota_windows
                (provider_id, window_key, label, scope, kind, used, quota_limit, remaining,
                 used_percent, remaining_percent, reset_at, is_unlimited, confidence, collected_at)
             VALUES ('legacy', '5h', '5-hour', 'rolling', 'tokens', 500, 0, 0, 0.0, 100.0, NULL, 0, 'low', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .expect("legacy window with fabricated percentage");
    storage
        .conn
        .execute(
            "INSERT INTO quota_windows
                (provider_id, window_key, label, scope, kind, used, quota_limit, remaining,
                 used_percent, remaining_percent, reset_at, is_unlimited, confidence, collected_at)
             VALUES ('legacy', '7d', '7-day', 'weekly', 'tokens', 200, 1000, 800, 20.0, 80.0, NULL, 0, 'high', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .expect("legacy window with a real limit");

    apply_all(&storage.conn).expect("upgrade to latest schema");

    let unknown: (Option<i64>, Option<i64>, Option<f64>, Option<f64>, i64) = storage
        .conn
        .query_row(
            "SELECT quota_limit, remaining, used_percent, remaining_percent, used
             FROM quota_windows WHERE window_key = '5h'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("legacy unknown-limit row survived");
    assert_eq!(
        unknown,
        (None, None, None, None, 500),
        "a zero limit must become unknown while the recorded usage is preserved"
    );

    let known: (Option<i64>, Option<i64>, Option<f64>, Option<f64>) = storage
        .conn
        .query_row(
            "SELECT quota_limit, remaining, used_percent, remaining_percent
             FROM quota_windows WHERE window_key = '7d'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy real-limit row survived");
    assert_eq!(known, (Some(1000), Some(800), Some(20.0), Some(80.0)));

    let report_count: i64 = storage
        .conn
        .query_row("SELECT COUNT(*) FROM quota_reports", [], |row| row.get(0))
        .expect("count reports");
    assert_eq!(report_count, 1, "existing reports are preserved");

    storage
        .integrity_check()
        .expect("database stays consistent");
}
