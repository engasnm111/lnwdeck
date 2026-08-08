//! Session and project display-name metadata.
//!
//! Raw session ids and folder paths never reach the database; adapters
//! persist keyed hashes. This repository manages only the user-entered
//! display names attached to those hashes. The hashes themselves are opaque
//! and are never renamed.

use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MetaRow {
    pub hash: String,
    pub display_name: String,
}

pub struct SessionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SessionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// All user-entered session display names.
    pub fn list_session_meta(&self) -> Result<Vec<MetaRow>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT session_hash, display_name FROM session_meta")?;
        let rows = stmt.query_map([], |row| {
            Ok(MetaRow {
                hash: row.get(0)?,
                display_name: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    /// All user-entered project display names.
    pub fn list_project_meta(&self) -> Result<Vec<MetaRow>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT project_hash, display_name FROM project_meta")?;
        let rows = stmt.query_map([], |row| {
            Ok(MetaRow {
                hash: row.get(0)?,
                display_name: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    /// Stores (or replaces) the display name for a session. An empty name
    /// clears the user-entered name, falling back to the generated label.
    pub fn rename_session(
        &self,
        session_hash: &str,
        display_name: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO session_meta (session_hash, display_name, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(session_hash) DO UPDATE SET
               display_name = excluded.display_name,
               updated_at = excluded.updated_at",
            params![session_hash, display_name],
        )?;
        Ok(())
    }

    /// Stores (or replaces) the display name for a project. An empty name
    /// clears the user-entered name, falling back to the generated label.
    pub fn rename_project(
        &self,
        project_hash: &str,
        display_name: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO project_meta (project_hash, display_name, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(project_hash) DO UPDATE SET
               display_name = excluded.display_name,
               updated_at = excluded.updated_at",
            params![project_hash, display_name],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    #[test]
    fn migration_adds_attribution_columns_and_meta_tables() {
        let storage = open_db();
        let column_count: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('usage_events')
                 WHERE name IN ('session_hash', 'project_hash')",
                [],
                |row| row.get(0),
            )
            .expect("pragma query");
        assert_eq!(column_count, 2, "both attribution columns must exist");

        let meta_tables: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('session_meta', 'project_meta')",
                [],
                |row| row.get(0),
            )
            .expect("table query");
        assert_eq!(meta_tables, 2, "both meta tables must exist");
    }

    #[test]
    fn rename_session_is_an_upsert_and_clears_with_empty_name() {
        let storage = open_db();
        let repo = SessionRepository::new(&storage.conn);

        repo.rename_session("hash-a", "fix notifications")
            .expect("first rename");
        let meta = repo.list_session_meta().expect("list");
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].hash, "hash-a");
        assert_eq!(meta[0].display_name, "fix notifications");

        repo.rename_session("hash-a", "refactor pipeline")
            .expect("second rename");
        let meta = repo.list_session_meta().expect("list");
        assert_eq!(meta.len(), 1, "rename must not duplicate the row");
        assert_eq!(meta[0].display_name, "refactor pipeline");

        repo.rename_session("hash-a", "").expect("clear");
        let meta = repo.list_session_meta().expect("list");
        assert_eq!(meta[0].display_name, "", "empty name clears the override");
    }

    #[test]
    fn rename_project_is_an_upsert() {
        let storage = open_db();
        let repo = SessionRepository::new(&storage.conn);

        repo.rename_project("proj-1", "lnwdeck").expect("rename");
        let meta = repo.list_project_meta().expect("list");
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].hash, "proj-1");
        assert_eq!(meta[0].display_name, "lnwdeck");

        repo.rename_project("proj-1", "lnwdeck v2")
            .expect("rename again");
        let meta = repo.list_project_meta().expect("list");
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].display_name, "lnwdeck v2");
    }

    #[test]
    fn unknown_hashes_yield_empty_meta_lists() {
        let storage = open_db();
        let repo = SessionRepository::new(&storage.conn);
        assert!(repo.list_session_meta().expect("sessions").is_empty());
        assert!(repo.list_project_meta().expect("projects").is_empty());
    }
}
