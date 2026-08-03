//! Background event log.
//!
//! Failures that happen outside a user action - the background refresh loop,
//! the update check, a migration - used to be dropped on the floor. They are
//! recorded here with a sanitized code and surfaced on the System page, so a
//! silent failure is impossible.

use rusqlite::{params, Connection};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppEventLevel {
    Info,
    Warning,
    Error,
}

impl AppEventLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "info" => Self::Info,
            "warning" => Self::Warning,
            _ => Self::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppEventRow {
    pub id: i64,
    pub occurred_at: String,
    /// Where it happened: refresh_loop, updater, migration, widget.
    pub source: String,
    pub level: AppEventLevel,
    /// Stable machine-readable code.
    pub code: String,
    /// Short sanitized detail. Never a path or a credential.
    pub detail: String,
}

pub struct AppEventRepository<'a> {
    conn: &'a Connection,
}

impl<'a> AppEventRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Records one event and returns its id.
    pub fn record(
        &self,
        source: &str,
        level: AppEventLevel,
        code: &str,
        detail: &str,
    ) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO app_events (occurred_at, source, level, code, detail)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                chrono::Utc::now().to_rfc3339(),
                source,
                level.as_str(),
                code,
                detail,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Newest events first, capped.
    pub fn recent(&self, limit: usize) -> Result<Vec<AppEventRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, occurred_at, source, level, code, detail
             FROM app_events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit.clamp(1, 500) as i64], |row| {
            let level: String = row.get(3)?;
            Ok(AppEventRow {
                id: row.get(0)?,
                occurred_at: row.get(1)?,
                source: row.get(2)?,
                level: AppEventLevel::parse(&level),
                code: row.get(4)?,
                detail: row.get(5)?,
            })
        })?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// Number of unresolved-looking problems: warnings and errors recorded at
    /// or after `since`.
    pub fn problem_count_since(&self, since: &str) -> Result<i64, rusqlite::Error> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM app_events
             WHERE occurred_at >= ?1 AND level IN ('warning', 'error')",
            [since],
            |row| row.get(0),
        )
    }

    /// Deletes events older than `before`, returning rows removed.
    pub fn prune(&self, before: &str) -> Result<usize, rusqlite::Error> {
        self.conn
            .execute("DELETE FROM app_events WHERE occurred_at < ?1", [before])
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
    fn events_are_recorded_and_returned_newest_first() {
        let storage = open_db();
        let repo = AppEventRepository::new(&storage.conn);
        repo.record(
            "refresh_loop",
            AppEventLevel::Error,
            "STORAGE_FAILURE",
            "the database could not be opened",
        )
        .expect("first");
        repo.record("updater", AppEventLevel::Warning, "UPDATE_CHECK_FAILED", "")
            .expect("second");

        let events = repo.recent(10).expect("recent");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].code, "UPDATE_CHECK_FAILED");
        assert_eq!(events[0].level, AppEventLevel::Warning);
        assert_eq!(events[1].source, "refresh_loop");
        assert_eq!(events[1].level, AppEventLevel::Error);
        assert!(!events[0].occurred_at.is_empty());
    }

    #[test]
    fn recent_is_capped_but_never_zero() {
        let storage = open_db();
        let repo = AppEventRepository::new(&storage.conn);
        for index in 0..5 {
            repo.record(
                "refresh_loop",
                AppEventLevel::Info,
                "TICK",
                &index.to_string(),
            )
            .expect("record");
        }
        assert_eq!(repo.recent(2).expect("recent").len(), 2);
        assert_eq!(
            repo.recent(0).expect("recent").len(),
            1,
            "a zero limit is clamped to one, never to an empty list"
        );
    }

    #[test]
    fn problem_count_ignores_info_events() {
        let storage = open_db();
        let repo = AppEventRepository::new(&storage.conn);
        repo.record("refresh_loop", AppEventLevel::Info, "TICK", "")
            .expect("info");
        repo.record("refresh_loop", AppEventLevel::Warning, "SLOW", "")
            .expect("warning");
        repo.record("refresh_loop", AppEventLevel::Error, "BROKEN", "")
            .expect("error");

        let since = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(repo.problem_count_since(&since).expect("count"), 2);

        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(repo.problem_count_since(&future).expect("count"), 0);
    }

    #[test]
    fn prune_removes_old_events_only() {
        let storage = open_db();
        let repo = AppEventRepository::new(&storage.conn);
        repo.record("refresh_loop", AppEventLevel::Info, "TICK", "")
            .expect("record");
        let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(repo.prune(&past).expect("prune"), 0);
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(repo.prune(&future).expect("prune"), 1);
        assert!(repo.recent(10).expect("recent").is_empty());
    }
}
