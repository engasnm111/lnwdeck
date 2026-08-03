use rusqlite::Connection;

pub struct RetentionService<'a> {
    conn: &'a Connection,
}

impl<'a> RetentionService<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn delete_by_provider(&self, provider_id: &str) -> Result<usize, rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let count = tx.execute(
            "DELETE FROM usage_events WHERE provider_id = ?1",
            [provider_id],
        )?;
        tx.execute(
            "DELETE FROM quota_snapshots WHERE provider_id = ?1",
            [provider_id],
        )?;
        tx.execute(
            "DELETE FROM sync_cursors WHERE provider_id = ?1",
            [provider_id],
        )?;
        tx.commit()?;
        Ok(count)
    }

    pub fn delete_before_date(&self, before_date: &str) -> Result<usize, rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let count = tx.execute(
            "DELETE FROM usage_events WHERE timestamp < ?1",
            [before_date],
        )?;
        tx.execute(
            "DELETE FROM quota_snapshots WHERE recorded_at < ?1",
            [before_date],
        )?;
        tx.commit()?;
        Ok(count)
    }

    pub fn delete_all(&self) -> Result<usize, rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let count = tx.execute("DELETE FROM usage_events", [])?;
        tx.execute("DELETE FROM quota_snapshots", [])?;
        tx.execute("DELETE FROM sync_cursors", [])?;
        tx.commit()?;
        Ok(count)
    }

    pub fn vacuum(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch("VACUUM")
    }
}

pub fn database_diagnostics(conn: &Connection) -> Result<DiagnosticsReport, rusqlite::Error> {
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;

    let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;

    let db_size_bytes = page_count * page_size;
    let free_bytes = freelist_count * page_size;

    let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    let wal_size: i64 = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
        .unwrap_or(0);

    let event_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))?;

    Ok(DiagnosticsReport {
        integrity_ok: integrity == "ok",
        db_size_bytes: db_size_bytes as u64,
        free_bytes: free_bytes as u64,
        journal_mode,
        wal_checkpoint_result: wal_size,
        event_count: event_count as u64,
    })
}

#[derive(Debug, Clone)]
pub struct DiagnosticsReport {
    pub integrity_ok: bool,
    pub db_size_bytes: u64,
    pub free_bytes: u64,
    pub journal_mode: String,
    pub wal_checkpoint_result: i64,
    pub event_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::apply_all;
    use crate::Storage;
    use tempfile::tempdir;

    #[test]
    fn delete_by_provider_removes_only_matching() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = Storage::open(&db_path).unwrap();
        apply_all(&storage.conn).unwrap();

        storage.conn.execute(
            "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, data_source, cost)
             VALUES ('e1', 'b1', '2025-01-01T00:00:00Z', 'openai', 'gpt-4o', 100, 50, 'High', 'web', '0.005')",
            [],
        ).unwrap();
        storage.conn.execute(
            "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, data_source, cost)
             VALUES ('e2', 'b1', '2025-01-02T00:00:00Z', 'anthropic', 'claude-3', 200, 100, 'High', 'web', '0.010')",
            [],
        ).unwrap();

        let retention = RetentionService::new(&storage.conn);
        let deleted = retention.delete_by_provider("openai").unwrap();
        assert_eq!(deleted, 1);

        let count: i64 = storage
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn delete_all_removes_everything() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = Storage::open(&db_path).unwrap();
        apply_all(&storage.conn).unwrap();

        storage.conn.execute(
            "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, data_source, cost)
             VALUES ('e1', 'b1', '2025-01-01T00:00:00Z', 'openai', 'gpt-4o', 100, 50, 'High', 'web', '0.005')",
            [],
        ).unwrap();

        let retention = RetentionService::new(&storage.conn);
        retention.delete_all().unwrap();

        let count: i64 = storage
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn diagnostics_reports_valid_data() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = Storage::open(&db_path).unwrap();
        apply_all(&storage.conn).unwrap();

        let report = database_diagnostics(&storage.conn).unwrap();
        assert!(report.integrity_ok);
        assert!(report.db_size_bytes > 0);
        assert_eq!(report.event_count, 0);
    }
}
