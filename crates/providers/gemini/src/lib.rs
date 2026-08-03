use lnwdeck_domain::{QuotaSnapshot, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct GeminiAdapter;

impl ProviderAdapter for GeminiAdapter {
    fn id(&self) -> &str {
        "google_gemini"
    }
    fn name(&self) -> &str {
        "Gemini (Google)"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("gemini_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
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
        assert_eq!(GeminiAdapter.id(), "google_gemini");
    }
    #[test]
    fn requires_credential_and_network() {
        let p = GeminiAdapter.required_permissions();
        assert!(p.contains(&Permission::Credential));
        assert!(p.contains(&Permission::Network));
    }
}
