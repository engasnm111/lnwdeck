use lnwdeck_domain::{QuotaReport, UsageBatch};
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

pub struct SaveQuotaReport;

impl SaveQuotaReport {
    pub fn execute(conn: &Connection, report: &QuotaReport) -> Result<(), IngestError> {
        PrivacyGuard::validate_quota_report(report).map_err(|_| IngestError::PrivacyViolation)?;
        let repo = QuotaRepository::new(conn);
        repo.upsert_report(report)
            .map(|_| ())
            .map_err(IngestError::Storage)
    }
}

#[derive(Debug)]
pub enum IngestError {
    PrivacyViolation,
    Storage(rusqlite::Error),
}
