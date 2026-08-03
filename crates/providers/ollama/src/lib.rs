use inwdeck_domain::{QuotaSnapshot, UsageBatch};
use inwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct OllamaAdapter;

impl ProviderAdapter for OllamaAdapter {
    fn id(&self) -> &str {
        "ollama_local"
    }
    fn name(&self) -> &str {
        "Ollama"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("ollama_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "Local server".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::Network]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_is_correct() {
        assert_eq!(OllamaAdapter.id(), "ollama_local");
    }
    #[test]
    fn requires_network() {
        assert!(OllamaAdapter
            .required_permissions()
            .contains(&Permission::Network));
    }
}
