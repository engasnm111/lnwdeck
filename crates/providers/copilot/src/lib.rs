use lnwdeck_domain::{QuotaReport, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct CopilotAdapter;

impl ProviderAdapter for CopilotAdapter {
    fn id(&self) -> &str {
        "github_copilot"
    }
    fn name(&self) -> &str {
        "Copilot (GitHub)"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("copilot_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
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
        assert_eq!(CopilotAdapter.id(), "github_copilot");
    }
    #[test]
    fn requires_filesystem() {
        assert!(CopilotAdapter
            .required_permissions()
            .contains(&Permission::FileSystem));
    }
}
