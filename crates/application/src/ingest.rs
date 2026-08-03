use lnwdeck_domain::{QuotaSnapshot, UsageBatch};
use lnwdeck_security::PrivacyGuard;
use lnwdeck_storage::repositories::{QuotaRepository, UsageRepository};
use rusqlite::Connection;

pub struct IngestUsageBatch;

impl IngestUsageBatch {
    pub fn execute(conn: &Connection, batch: &UsageBatch) -> Result<(), IngestError> {
        PrivacyGuard::validate_usage_batch(batch).map_err(|_| IngestError::PrivacyViolation)?;
        let repo = UsageRepository::new(conn);
        repo.ingest_batch(batch).map_err(IngestError::Storage)
    }
}

pub struct SaveQuotaSnapshot;

impl SaveQuotaSnapshot {
    pub fn execute(conn: &Connection, snapshot: &QuotaSnapshot) -> Result<(), IngestError> {
        let repo = QuotaRepository::new(conn);
        repo.insert_snapshot(snapshot).map_err(IngestError::Storage)
    }
}

#[derive(Debug)]
pub enum IngestError {
    PrivacyViolation,
    Storage(rusqlite::Error),
}
