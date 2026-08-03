use inwdeck_domain::{QuotaSnapshot, UsageBatch};
use inwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct CursorAdapter;

impl ProviderAdapter for CursorAdapter {
    fn id(&self) -> &str {
        "cursor_ide"
    }
    fn name(&self) -> &str {
        "Cursor"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("cursor_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "IDE-based".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_is_correct() {
        assert_eq!(CursorAdapter.id(), "cursor_ide");
    }
    #[test]
    fn requires_filesystem() {
        assert!(CursorAdapter
            .required_permissions()
            .contains(&Permission::FileSystem));
    }
}
