use crate::collection::CollectionResult;
use crate::detection::DetectionResult;
use crate::health::AdapterHealth;
use crate::permissions::Permission;
use lnwdeck_domain::{QuotaSnapshot, UsageBatch};

pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn collect_usage(&self) -> Result<UsageBatch, String>;
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String>;
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
}
