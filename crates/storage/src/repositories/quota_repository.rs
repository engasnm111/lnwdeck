use inwdeck_domain::QuotaSnapshot;
use rusqlite::Connection;

pub struct QuotaRepository<'a> {
    conn: &'a Connection,
}

impl<'a> QuotaRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert_snapshot(&self, snapshot: &QuotaSnapshot) -> Result<(), rusqlite::Error> {
        let recorded_at = snapshot.recorded_at.to_rfc3339();
        self.conn.execute(
            "INSERT INTO quota_snapshots (provider_id, quota_limit, quota_used, recorded_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                snapshot.provider_id,
                snapshot.quota_limit as i64,
                snapshot.quota_used as i64,
                recorded_at,
            ],
        )?;
        Ok(())
    }
}
