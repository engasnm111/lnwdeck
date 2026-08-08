use lnwdeck_domain::UsageBatch;
use rusqlite::Connection;

pub struct UsageRepository<'a> {
    conn: &'a Connection,
}

impl<'a> UsageRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn ingest_batch(&self, batch: &UsageBatch) -> Result<(), rusqlite::Error> {
        self.ingest_batch_with_counts(batch).map(|_| ())
    }

    /// Ingests a batch and returns `(inserted, duplicates_skipped)`.
    /// Event ids are the stable fingerprints: `INSERT OR IGNORE` skips
    /// rows whose fingerprint already exists.
    pub fn ingest_batch_with_counts(
        &self,
        batch: &UsageBatch,
    ) -> Result<(u64, u64), rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;

        let mut inserted: u64 = 0;
        let mut duplicates: u64 = 0;

        for event in &batch.events {
            let timestamp = event.timestamp.to_rfc3339();

            let changed = tx.execute(
                "INSERT OR IGNORE INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_cached, tokens_cache_write, tokens_output, tokens_reasoning, confidence, data_source, cost, session_hash, project_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                rusqlite::params![
                    event.id,
                    batch.batch_id,
                    timestamp,
                    event.provider_id,
                    event.model,
                    event.tokens_input as i64,
                    event.tokens_cached as i64,
                    event.tokens_cache_write as i64,
                    event.tokens_output as i64,
                    event.tokens_reasoning as i64,
                    format!("{:?}", event.confidence),
                    event.data_source,
                    event.cost,
                    event.session_hash.as_deref().unwrap_or(""),
                    event.project_hash.as_deref().unwrap_or(""),
                ],
            )?;
            if changed == 1 {
                inserted += 1;
            } else {
                duplicates += 1;
            }
        }

        tx.commit()?;
        Ok((inserted, duplicates))
    }

    /// Replaces the normalized rows for a full-scan local provider. Full scans
    /// are required for sources whose source rows are cumulative snapshots;
    /// keeping older snapshots would double-count every refresh. Only the
    /// named provider and its two known source revisions are touched.
    pub fn replace_provider_batch(
        &self,
        batch: &UsageBatch,
        provider_id: &str,
        current_source: &str,
        legacy_source: &str,
    ) -> Result<(u64, u64), rusqlite::Error> {
        if batch.events.is_empty() {
            return Ok((0, 0));
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM usage_events
             WHERE provider_id = ?1 AND data_source IN (?2, ?3)",
            rusqlite::params![provider_id, current_source, legacy_source],
        )?;

        let mut inserted = 0u64;
        for event in &batch.events {
            inserted += tx.execute(
                "INSERT OR IGNORE INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_cached, tokens_cache_write, tokens_output, tokens_reasoning, confidence, data_source, cost, session_hash, project_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                rusqlite::params![
                    event.id,
                    batch.batch_id,
                    event.timestamp.to_rfc3339(),
                    event.provider_id,
                    event.model,
                    event.tokens_input as i64,
                    event.tokens_cached as i64,
                    event.tokens_cache_write as i64,
                    event.tokens_output as i64,
                    event.tokens_reasoning as i64,
                    format!("{:?}", event.confidence),
                    event.data_source,
                    event.cost,
                    event.session_hash.as_deref().unwrap_or(""),
                    event.project_hash.as_deref().unwrap_or(""),
                ],
            )? as u64;
        }
        tx.commit()?;
        Ok((inserted, 0))
    }
}
