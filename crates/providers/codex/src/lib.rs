use lnwdeck_domain::{QuotaSnapshot, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct CodexAdapter;

impl ProviderAdapter for CodexAdapter {
    fn id(&self) -> &str {
        "openai_codex"
    }
    fn name(&self) -> &str {
        "Codex (OpenAI)"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("codex_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "Not configured".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::Credential]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_is_correct() {
        assert_eq!(CodexAdapter.id(), "openai_codex");
    }
    #[test]
    fn requires_credential() {
        assert!(CodexAdapter
            .required_permissions()
            .contains(&Permission::Credential));
    }
}
