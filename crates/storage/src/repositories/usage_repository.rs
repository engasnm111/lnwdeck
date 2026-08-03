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
        let tx = self.conn.unchecked_transaction()?;

        for event in &batch.events {
            let timestamp = event.timestamp.to_rfc3339();

            match tx.execute(
                "INSERT OR IGNORE INTO usage_events (id, batch_id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    event.id,
                    batch.batch_id,
                    timestamp,
                    event.provider_id,
                    event.model,
                    event.tokens_input as i64,
                    event.tokens_output as i64,
                    format!("{:?}", event.confidence),
                    event.data_source,
                    event.cost,
                ],
            ) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            }
        }

        tx.commit()?;
        Ok(())
    }
}
