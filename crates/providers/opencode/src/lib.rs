use lnwdeck_domain::{QuotaSnapshot, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct OpenCodeAdapter;

impl ProviderAdapter for OpenCodeAdapter {
    fn id(&self) -> &str {
        "opencode_cli"
    }
    fn name(&self) -> &str {
        "OpenCode"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("opencode_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "Local CLI".to_string(),
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
        assert_eq!(OpenCodeAdapter.id(), "opencode_cli");
    }
    #[test]
    fn requires_filesystem() {
        assert!(OpenCodeAdapter
            .required_permissions()
            .contains(&Permission::FileSystem));
    }
}
