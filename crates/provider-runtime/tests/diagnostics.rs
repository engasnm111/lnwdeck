use lnwdeck_domain::{QuotaReport, UsageBatch};
use lnwdeck_provider_runtime::{AdapterHealth, AdapterHealthStatus, Permission, ProviderAdapter};

struct TestAdapter;

impl ProviderAdapter for TestAdapter {
    fn id(&self) -> &str {
        "test_adapter"
    }
    fn name(&self) -> &str {
        "Test Adapter"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "test".to_string(),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
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

struct FailingAdapter;

impl ProviderAdapter for FailingAdapter {
    fn id(&self) -> &str {
        "failing_adapter"
    }
    fn name(&self) -> &str {
        "Failing Adapter"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Err("SOURCE_UNAVAILABLE".to_string())
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        Ok(None)
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Unhealthy,
            message: "failing".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

#[test]
fn default_detection_reports_unsupported_without_side_effects() {
    let result = TestAdapter.detect().expect("detection must not fail");
    assert_eq!(result.provider_id, "test_adapter");
    assert_eq!(result.display_name, "Test Adapter");
    assert!(!result.detected, "stub adapter must not claim detection");
    assert_eq!(result.detection_method, "unsupported");
    assert!(!result.source_exists);
}

#[test]
fn default_collection_wraps_successful_batch() {
    let result = TestAdapter.collect_usage_with_cursor(None);
    assert!(result.batch.is_some(), "successful adapter yields a batch");
    assert_eq!(result.outcome.provider_id, "test_adapter");
    assert_eq!(result.outcome.error_code, "", "no error on success");
    assert!(!result.outcome.started_at.is_empty());
    assert!(!result.outcome.finished_at.is_empty());
}

#[test]
fn default_collection_records_failure_without_panicking() {
    let result = FailingAdapter.collect_usage_with_cursor(None);
    assert!(
        result.batch.is_none(),
        "failed adapter must not yield a batch"
    );
    assert_eq!(result.outcome.error_code, "SOURCE_UNAVAILABLE");
    assert_eq!(result.outcome.events_normalized, 0);
}

#[test]
fn default_collection_preserves_cursor() {
    let result = TestAdapter.collect_usage_with_cursor(Some("cursor-42"));
    assert_eq!(result.next_cursor.as_deref(), Some("cursor-42"));
}

#[test]
fn detection_result_serializes_without_forbidden_fields() {
    let result = TestAdapter.detect().expect("detection");
    let json = serde_json::to_value(&result).expect("serialize");
    let object = json.as_object().expect("object");
    for key in [
        "prompt",
        "response",
        "path",
        "file_name",
        "cookie",
        "token",
        "api_key",
    ] {
        assert!(
            !object.contains_key(key),
            "forbidden key {key} in detection JSON"
        );
    }
}

#[test]
fn collection_outcome_serializes_without_forbidden_fields() {
    let result = TestAdapter.collect_usage_with_cursor(None);
    let json = serde_json::to_value(&result.outcome).expect("serialize");
    let object = json.as_object().expect("object");
    for key in [
        "prompt",
        "response",
        "path",
        "file_name",
        "cookie",
        "token",
        "api_key",
    ] {
        assert!(
            !object.contains_key(key),
            "forbidden key {key} in outcome JSON"
        );
    }
}

#[test]
fn collection_result_exposes_diagnostic_fields() {
    let result = TestAdapter.collect_usage_with_cursor(None);
    let outcome = &result.outcome;
    assert_eq!(outcome.collector_mode, "basic");
    assert_eq!(outcome.duration_ms, 0, "basic mode reports zero duration");
    assert_eq!(outcome.source_records_seen, 0);
    assert_eq!(outcome.records_parsed, 0);
    assert_eq!(outcome.events_normalized, 0);
    assert_eq!(outcome.events_rejected, 0);
    assert_eq!(outcome.duplicates_skipped, 0);
    assert_eq!(outcome.events_inserted, 0);
    assert_eq!(outcome.quota_snapshots_inserted, 0);
    assert!(outcome.warning_codes.is_empty());
    assert!(outcome.next_retry_at.is_none());
}
