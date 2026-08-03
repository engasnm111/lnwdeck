use lnwdeck_domain::{QuotaSnapshot, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct OpenRouterAdapter;

impl ProviderAdapter for OpenRouterAdapter {
    fn id(&self) -> &str {
        "openrouter_api"
    }
    fn name(&self) -> &str {
        "OpenRouter"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("openrouter_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "API aggregator".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::Credential, Permission::Network]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_is_correct() {
        assert_eq!(OpenRouterAdapter.id(), "openrouter_api");
    }
}
