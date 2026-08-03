use crate::adapter::ProviderAdapter;
use crate::permissions::Permissions;
use crate::registry::AdapterRegistry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub default_interval: Duration,
    pub idle_slowdown_factor: f64,
    pub max_backoff: Duration,
    pub base_backoff: Duration,
    pub hard_timeout: Duration,
    pub jitter_factor: f64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            default_interval: Duration::from_secs(300),
            idle_slowdown_factor: 2.0,
            max_backoff: Duration::from_secs(3600),
            base_backoff: Duration::from_secs(5),
            hard_timeout: Duration::from_secs(30),
            jitter_factor: 0.1,
        }
    }
}

struct AdapterState {
    backoff: Duration,
    last_run: Option<Instant>,
    last_success: Option<Instant>,
}

pub struct AdaptiveScheduler {
    registry: AdapterRegistry,
    cancel_flag: Arc<AtomicBool>,
    states: Mutex<HashMap<String, AdapterState>>,
    config: SchedulerConfig,
    permissions: Permissions,
}

impl AdaptiveScheduler {
    pub fn new(registry: AdapterRegistry) -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            registry,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            config: SchedulerConfig::default(),
            permissions: Permissions::new(&[]),
        }
    }

    pub fn with_config(mut self, config: SchedulerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn adapter_count(&self) -> usize {
        self.registry.adapters().len()
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    fn is_adapter_permitted(&self, adapter: &dyn ProviderAdapter) -> bool {
        let required = adapter.required_permissions();
        required.iter().all(|p| self.permissions.has(p))
    }

    fn compute_backoff(&self, state: &AdapterState) -> Duration {
        let jitter_range = (state.backoff.as_secs_f64() * self.config.jitter_factor) as u64;
        let jitter = if jitter_range > 0 {
            Duration::from_millis((jitter_range as f64 * rand_f64()) as u64)
        } else {
            Duration::ZERO
        };
        (state.backoff + jitter).min(self.config.max_backoff)
    }

    pub fn schedule_all(&self) -> Vec<(String, Duration)> {
        let states = self.states.lock().unwrap();
        let mut schedule = Vec::new();

        let default_state = AdapterState {
            backoff: self.config.base_backoff,
            last_run: None,
            last_success: None,
        };

        for adapter in self.registry.adapters() {
            let state = states.get(adapter.id()).unwrap_or(&default_state);
            let delay = self.compute_backoff(state);
            schedule.push((adapter.id().to_string(), delay));
        }
        schedule
    }

    pub fn trigger_manual(&self, adapter_id: &str) -> Option<bool> {
        for adapter in self.registry.adapters() {
            if adapter.id() == adapter_id {
                if !self.is_adapter_permitted(adapter.as_ref()) {
                    return Some(false);
                }
                return Some(true);
            }
        }
        None
    }

    pub fn mark_success(&self, adapter_id: &str) {
        let mut states = self.states.lock().unwrap();
        let now = Instant::now();
        let state = states
            .entry(adapter_id.to_string())
            .or_insert(AdapterState {
                backoff: self.config.base_backoff,
                last_run: Some(now),
                last_success: Some(now),
            });
        state.backoff = self.config.base_backoff;
        state.last_success = Some(now);
        state.last_run = Some(now);
    }

    pub fn mark_failure(&self, adapter_id: &str) {
        let mut states = self.states.lock().unwrap();
        let now = Instant::now();
        let state = states
            .entry(adapter_id.to_string())
            .or_insert(AdapterState {
                backoff: self.config.base_backoff,
                last_run: Some(now),
                last_success: None,
            });
        state.backoff = (state.backoff * 2).min(self.config.max_backoff);
        state.last_run = Some(now);
    }
}

fn rand_f64() -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    Instant::now().hash(&mut hasher);
    (hasher.finish() as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::ProviderAdapter;
    use crate::health::{AdapterHealth, AdapterHealthStatus};
    use crate::permissions::Permission;
    use lnwdeck_domain::{QuotaSnapshot, UsageBatch};

    struct TestAdapter {
        id: String,
    }

    impl ProviderAdapter for TestAdapter {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            "Test"
        }
        fn collect_usage(&self) -> Result<UsageBatch, String> {
            Ok(UsageBatch {
                batch_id: "test".to_string(),
                events: vec![],
            })
        }
        fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
            Ok(None)
        }
        fn health_check(&self) -> AdapterHealth {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "ok".to_string(),
            }
        }
        fn required_permissions(&self) -> Vec<Permission> {
            vec![]
        }
    }

    #[test]
    fn backoff_increases_on_failure() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(TestAdapter {
            id: "a1".to_string(),
        }));
        let scheduler = AdaptiveScheduler::new(registry);

        let before = scheduler
            .schedule_all()
            .into_iter()
            .find(|(id, _)| id == "a1")
            .unwrap()
            .1;

        scheduler.mark_failure("a1");
        scheduler.mark_failure("a1");

        let after = scheduler
            .schedule_all()
            .into_iter()
            .find(|(id, _)| id == "a1")
            .unwrap()
            .1;

        assert!(after > before, "backoff must increase after failures");
    }

    #[test]
    fn backoff_resets_on_success() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(TestAdapter {
            id: "a1".to_string(),
        }));
        let scheduler = AdaptiveScheduler::new(registry);

        scheduler.mark_failure("a1");
        scheduler.mark_failure("a1");
        scheduler.mark_success("a1");

        let after = scheduler
            .schedule_all()
            .into_iter()
            .find(|(id, _)| id == "a1")
            .unwrap()
            .1;

        assert_eq!(
            after,
            SchedulerConfig::default().base_backoff,
            "backoff resets on success"
        );
    }

    #[test]
    fn manual_trigger_returns_none_for_unknown_adapter() {
        let registry = AdapterRegistry::new();
        let scheduler = AdaptiveScheduler::new(registry);
        assert_eq!(scheduler.trigger_manual("unknown"), None);
    }

    #[test]
    fn manual_trigger_checks_permissions() {
        struct RestrictedAdapter;
        impl ProviderAdapter for RestrictedAdapter {
            fn id(&self) -> &str {
                "restricted"
            }
            fn name(&self) -> &str {
                "Restricted"
            }
            fn collect_usage(&self) -> Result<UsageBatch, String> {
                Ok(UsageBatch {
                    batch_id: "r".to_string(),
                    events: vec![],
                })
            }
            fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
                Ok(None)
            }
            fn health_check(&self) -> AdapterHealth {
                AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "ok".to_string(),
                }
            }
            fn required_permissions(&self) -> Vec<Permission> {
                vec![Permission::FileSystem]
            }
        }

        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(RestrictedAdapter));
        let scheduler = AdaptiveScheduler::new(registry);

        assert_eq!(scheduler.trigger_manual("restricted"), Some(false));
    }

    #[test]
    fn cancel_stops_pending_work() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(TestAdapter {
            id: "a1".to_string(),
        }));
        let scheduler = AdaptiveScheduler::new(registry);

        assert!(!scheduler.is_cancelled());
        scheduler.cancel();
        assert!(scheduler.is_cancelled());
        scheduler.reset_cancel();
        assert!(!scheduler.is_cancelled());
    }
}
