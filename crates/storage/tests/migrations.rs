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
