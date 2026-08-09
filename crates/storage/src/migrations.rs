use rusqlite::Connection;
use std::path::Path;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "000_schema_tracking",
        include_str!("../migrations/000_schema_tracking.sql"),
    ),
    ("001_initial", include_str!("../migrations/001_initial.sql")),
    (
        "002_collector_diagnostics",
        include_str!("../migrations/002_collector_diagnostics.sql"),
    ),
    (
        "003_quota_reports",
        include_str!("../migrations/003_quota_reports.sql"),
    ),
    (
        "004_quota_window_nullable",
        include_str!("../migrations/004_quota_window_nullable.sql"),
    ),
    (
        "005_budgets_alerts_settings",
        include_str!("../migrations/005_budgets_alerts_settings.sql"),
    ),
    (
        "006_sessions",
        include_str!("../migrations/006_sessions.sql"),
    ),
    (
        "007_usage_breakdown",
        include_str!("../migrations/007_usage_breakdown.sql"),
    ),
    (
        "008_account_identity",
        include_str!("../migrations/008_account_identity.sql"),
    ),
];

/// Names of every migration known to this build, oldest first.
pub fn known_migrations() -> Vec<&'static str> {
    MIGRATIONS.iter().map(|(name, _)| *name).collect()
}

/// Versions already recorded in `schema_migrations`.
fn applied_versions(conn: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

/// Applies pending migrations only. Each migration runs in a transaction and
/// its version is recorded in the same transaction, so a failure leaves the
/// database on the previous version instead of half-migrated. Re-running is a
/// no-op; migrations that rebuild tables are therefore never replayed.
pub fn apply_all(conn: &Connection) -> Result<(), rusqlite::Error> {
    // The tracking table itself must exist before it can be queried; its
    // migration is written to be idempotent.
    let (_, tracking_sql) = MIGRATIONS[0];
    conn.execute_batch(tracking_sql)?;

    let applied = applied_versions(conn)?;
    for (name, sql) in MIGRATIONS {
        if applied.iter().any(|version| version == name) {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT OR REPLACE INTO schema_migrations (version) VALUES (?1)",
            [*name],
        )?;
        tx.commit()?;
    }
    Ok(())
}

pub fn migrate_with_backup(conn: &Connection, backup_dir: &Path) -> Result<(), rusqlite::Error> {
    let (_, tracking_sql) = MIGRATIONS[0];
    conn.execute_batch(tracking_sql)?;
    let applied = applied_versions(conn)?;

    let pending: Vec<(&str, &str)> = MIGRATIONS
        .iter()
        .filter(|(name, _)| !applied.contains(&name.to_string()))
        .cloned()
        .collect();

    if pending.is_empty() {
        return Ok(());
    }

    let backup_path = backup_dir.join("pre_migration_backup.db");
    if let Ok(mut dest) = Connection::open(&backup_path) {
        let backup = rusqlite::backup::Backup::new(conn, &mut dest)?;
        backup.step(-1)?;
    }

    for (name, sql) in pending {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT OR REPLACE INTO schema_migrations (version) VALUES (?1)",
            [name],
        )?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn backup_before_migration_preserves_data() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("test.db");
        let backup_dir = tempdir().expect("backup dir");

        let conn = Connection::open(&db_path).expect("open");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("pragma");
        conn.execute_batch(include_str!("../migrations/000_schema_tracking.sql"))
            .expect("schema tracking");

        conn.execute(
            "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, data_source, cost)
             VALUES ('evt_x', 'batch_x', '2025-01-01T00:00:00Z', 'openai', 'gpt-4o', 100, 50, 'High', 'web', '0.005')",
            [],
        )
        .expect_err("table should not exist yet");

        migrate_with_backup(&conn, backup_dir.path()).expect("migrate with backup");

        let backup_file = backup_dir.path().join("pre_migration_backup.db");
        assert!(backup_file.exists(), "pre-migration backup must exist");
    }
}
