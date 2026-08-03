use crate::health::AdapterHealth;
use crate::permissions::Permission;
use inwdeck_domain::{QuotaSnapshot, UsageBatch};

pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn collect_usage(&self) -> Result<UsageBatch, String>;
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String>;
    fn health_check(&self) -> AdapterHealth;
    fn required_permissions(&self) -> Vec<Permission>;
}
