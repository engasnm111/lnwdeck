use crate::collection::CollectionResult;
use crate::detection::DetectionResult;
use crate::health::AdapterHealth;
use crate::permissions::Permission;
use crate::quota::QuotaCollectionResult;
use lnwdeck_domain::{QuotaReport, UsageBatch};

pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn collect_usage(&self) -> Result<UsageBatch, String>;
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String>;
    fn health_check(&self) -> AdapterHealth;
    fn required_permissions(&self) -> Vec<Permission>;

    /// Runs sanitized detection. Adapters without detection support report
    /// `detected = false` with method `unsupported`.
    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(DetectionResult::unsupported(self.id(), self.name()))
    }

    /// Collects usage with an optional persisted cursor. The default
    /// implementation delegates to `collect_usage` and preserves the cursor.
    fn collect_usage_with_cursor(&self, cursor: Option<&str>) -> CollectionResult {
        let started_at = chrono::Utc::now();
        CollectionResult::from_basic(self.id(), "basic", started_at, self.collect_usage(), cursor)
    }

    /// Collects the provider quota report and wraps the result with evidence.
    /// `Ok(None)` is reported as `UNSUPPORTED`; errors are encoded in the
    /// outcome, never thrown.
    fn collect_quota_report(&self) -> QuotaCollectionResult {
        let started_at = chrono::Utc::now();
        match self.collect_quota() {
            Ok(Some(report)) => QuotaCollectionResult::from_report(report, started_at),
            Ok(None) => QuotaCollectionResult::unsupported(self.id(), started_at),
            Err(code) => QuotaCollectionResult::failed(self.id(), started_at, code),
        }
    }
}
