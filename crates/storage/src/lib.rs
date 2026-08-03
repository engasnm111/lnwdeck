pub mod migrations;
pub mod repositories;
pub mod retention;

use rusqlite::Connection;
use std::path::Path;

pub struct Storage {
    pub conn: Connection,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn list_tables(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut tables = Vec::new();
        for row in rows {
            tables.push(row?);
        }
        Ok(tables)
    }

    pub fn integrity_check(&self) -> Result<(), rusqlite::Error> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if result != "ok" {
            return Err(rusqlite::Error::IntegralValueOutOfRange(0, 0));
        }
        Ok(())
    }

    pub fn backup_to(&self, dest: &Path) -> Result<(), rusqlite::Error> {
        let mut dest_conn = Connection::open(dest)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest_conn)?;
        backup.step(-1)?;
        Ok(())
    }
}
