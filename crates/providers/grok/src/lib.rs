use lnwdeck_domain::{QuotaReport, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct GrokAdapter;

impl ProviderAdapter for GrokAdapter {
    fn id(&self) -> &str {
        "xai_grok"
    }
    fn name(&self) -> &str {
        "Grok (xAI)"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("grok_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "API-based".to_string(),
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
        assert_eq!(GrokAdapter.id(), "xai_grok");
    }
}
