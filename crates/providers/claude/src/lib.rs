use lnwdeck_domain::{QuotaSnapshot, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

pub struct ClaudeAdapter;

impl ProviderAdapter for ClaudeAdapter {
    fn id(&self) -> &str {
        "anthropic_claude"
    }
    fn name(&self) -> &str {
        "Claude (Anthropic)"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("claude_{}", chrono::Utc::now().timestamp()),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "Not detected".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_correct_id() {
        assert_eq!(ClaudeAdapter.id(), "anthropic_claude");
    }

    #[test]
    fn produces_valid_batch() {
        let result = ClaudeAdapter.collect_usage().unwrap();
        assert!(result.events.is_empty(), "no events when not configured");
    }

    #[test]
    fn returns_healthy_status() {
        let health = ClaudeAdapter.health_check();
        assert_eq!(health.status, AdapterHealthStatus::Healthy);
    }
}
