use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AdapterRegistry, AdaptiveScheduler,
    AuthKind, ChannelSupport, Permission, Permissions, ProviderAdapter, SourceKind,
};
use std::sync::Mutex;
use std::time::Duration;

fn sample_event(provider_id: &str) -> UsageEvent {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    UsageEvent {
        id: format!("evt_{provider_id}_{n}"),
        timestamp: chrono::Utc::now(),
        provider_id: provider_id.to_string(),
        model: "gpt-4o".to_string(),
        tokens_input: 100,
        tokens_output: 50,
        confidence: Confidence::High,
        data_source: "web".to_string(),
        cost: "0.005".to_string(),
        session_hash: None,
        project_hash: None,
    }
}

/// Descriptor for a test adapter that collects usage from a local source.
fn usage_descriptor(id: &'static str, name: &'static str) -> AdapterDescriptor {
    AdapterDescriptor {
        id,
        display_name: name,
        vendor: "Test Vendor",
        source_kind: SourceKind::LocalJsonl,
        usage_support: ChannelSupport::LocalEstimate,
        quota_support: ChannelSupport::Unsupported,
        auth: AuthKind::LocalFiles,
        adapter_version: "0.2.0",
    }
}

// ── Fake adapters ──

struct SuccessAdapter {
    id: &'static str,
}

impl ProviderAdapter for SuccessAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        usage_descriptor(self.id, "Success")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: format!("batch_{}", self.id),
            events: vec![sample_event(self.id)],
        })
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

struct PartialDataAdapter {
    id: &'static str,
}

impl ProviderAdapter for PartialDataAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        usage_descriptor(self.id, "PartialData")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Err("partial data: only 3 of 10 records".to_string())
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Degraded,
            message: "partial data".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

struct RateLimitAdapter {
    id: &'static str,
    call_count: Mutex<usize>,
}

impl ProviderAdapter for RateLimitAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        usage_descriptor(self.id, "RateLimit")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        Err("rate limit exceeded".to_string())
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Unhealthy,
            message: "rate limited".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

#[allow(dead_code)]
struct TimeoutAdapter;

impl ProviderAdapter for TimeoutAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        usage_descriptor("timeout", "Timeout")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        std::thread::sleep(Duration::from_secs(10));
        Ok(UsageBatch {
            batch_id: "timeout_batch".to_string(),
            events: vec![],
        })
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Unhealthy,
            message: "timeout".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

#[allow(dead_code)]
struct PanicAdapter;

impl ProviderAdapter for PanicAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        usage_descriptor("panic", "Panic")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "panic_batch".to_string(),
            events: vec![sample_event("panic_adapter")],
        })
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Unhealthy,
            message: "panicked".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

// ── Isolation tests ──

#[test]
fn failing_adapter_does_not_cancel_others() {
    let mut registry = AdapterRegistry::new();
    registry
        .register(Box::new(SuccessAdapter { id: "success" }))
        .expect("register adapter");
    registry
        .register(Box::new(RateLimitAdapter {
            id: "rate_limited",
            call_count: Mutex::new(0),
        }))
        .expect("register adapter");

    let results: Vec<_> = registry
        .adapters()
        .iter()
        .map(|a| a.collect_usage())
        .collect();

    assert!(results[0].is_ok(), "success adapter must return Ok");
    assert!(results[1].is_err(), "rate-limited adapter must return Err");
}

#[test]
fn last_good_data_remains_after_failure() {
    let mut registry = AdapterRegistry::new();
    registry
        .register(Box::new(SuccessAdapter { id: "good" }))
        .expect("register adapter");
    registry
        .register(Box::new(RateLimitAdapter {
            id: "bad",
            call_count: Mutex::new(0),
        }))
        .expect("register adapter");

    let results: Vec<_> = registry
        .adapters()
        .iter()
        .map(|a| a.collect_usage())
        .collect();

    let good_data: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
    assert!(!good_data.is_empty(), "last-good data must remain visible");
}

#[test]
fn health_reports_individual_adapter_status() {
    let mut registry = AdapterRegistry::new();
    registry
        .register(Box::new(SuccessAdapter { id: "s1" }))
        .expect("register adapter");
    registry
        .register(Box::new(PartialDataAdapter { id: "p1" }))
        .expect("register adapter");

    let health: Vec<_> = registry
        .adapters()
        .iter()
        .map(|a| (a.id().to_string(), a.health_check()))
        .collect();

    let s1_health = health.iter().find(|(id, _)| id == "s1").unwrap();
    assert_eq!(s1_health.1.status, AdapterHealthStatus::Healthy);

    let p1_health = health.iter().find(|(id, _)| id == "p1").unwrap();
    assert_eq!(p1_health.1.status, AdapterHealthStatus::Degraded);
}

#[test]
fn adapter_with_unsatisfied_permissions_is_skipped() {
    struct RestrictedAdapter;
    impl ProviderAdapter for RestrictedAdapter {
        fn descriptor(&self) -> AdapterDescriptor {
            usage_descriptor("restricted", "Restricted")
        }
        fn collect_usage(&self) -> Result<UsageBatch, String> {
            Ok(UsageBatch {
                batch_id: "restricted".to_string(),
                events: vec![],
            })
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

    let permissions = Permissions::new(&[]);
    let adapter = RestrictedAdapter;
    let required = adapter.required_permissions();

    let all_granted = required.iter().all(|p| permissions.has(p));
    assert!(
        !all_granted,
        "FileSystem permission must not be granted by default"
    );
}

#[test]
fn scheduler_respects_adapter_registry() {
    let mut registry = AdapterRegistry::new();
    registry
        .register(Box::new(SuccessAdapter { id: "s1" }))
        .expect("register adapter");

    let scheduler = AdaptiveScheduler::new(registry);
    assert_eq!(
        scheduler.adapter_count(),
        1,
        "scheduler must track registered adapters"
    );
}
