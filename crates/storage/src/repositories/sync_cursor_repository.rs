use rusqlite::Connection;

pub struct SyncCursorRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SyncCursorRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn upsert_cursor(
        &self,
        provider_id: &str,
        cursor_value: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_cursors (provider_id, cursor_value, updated_at)
             VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![provider_id, cursor_value],
        )?;
        Ok(())
    }

    pub fn get_cursor(&self, provider_id: &str) -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT cursor_value FROM sync_cursors WHERE provider_id = ?1")?;
        let mut rows = stmt.query_map([provider_id], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }
}
