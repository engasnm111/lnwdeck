use lnwdeck_domain::UsageBatch;
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    ProviderAdapter, SourceKind,
};

fn descriptor(id: &'static str, name: &'static str) -> AdapterDescriptor {
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

struct TestAdapter;

impl ProviderAdapter for TestAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        descriptor("test_adapter", "Test Adapter")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "test".to_string(),
            events: vec![],
        })
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Healthy,
            message: "ok".to_string(),
        }
    }
}

struct FailingAdapter;

impl ProviderAdapter for FailingAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        descriptor("failing_adapter", "Failing Adapter")
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Err("SOURCE_UNAVAILABLE".to_string())
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Unhealthy,
            message: "failing".to_string(),
        }
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
    assert_eq!(
        outcome.collector_mode, "local_scan",
        "the collector mode comes from the declared channel support"
    );
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

/// An adapter whose descriptor declares no usage support must never produce a
/// successful collection: the channel is not called and the attempt is
/// recorded as NOT_SUPPORTED.
#[test]
fn unsupported_channels_are_recorded_as_not_supported() {
    struct InertAdapter;
    impl ProviderAdapter for InertAdapter {
        fn descriptor(&self) -> AdapterDescriptor {
            AdapterDescriptor {
                id: "inert",
                display_name: "Inert",
                vendor: "Test Vendor",
                source_kind: SourceKind::None,
                usage_support: ChannelSupport::Unsupported,
                quota_support: ChannelSupport::Unsupported,
                auth: AuthKind::None,
                adapter_version: "0.2.0",
            }
        }
        fn collect_usage(&self) -> Result<UsageBatch, String> {
            panic!("an unsupported usage channel must never be invoked");
        }
    }

    let usage = InertAdapter.collect_usage_with_cursor(Some("cursor-1"));
    assert!(usage.batch.is_none(), "no batch is produced");
    assert_eq!(usage.outcome.error_code, "NOT_SUPPORTED");
    assert!(usage.outcome.is_not_supported());
    assert_eq!(usage.outcome.events_normalized, 0);
    assert_eq!(usage.outcome.collector_mode, "unsupported");
    assert_eq!(
        usage.next_cursor.as_deref(),
        Some("cursor-1"),
        "an unsupported attempt must not disturb the stored cursor"
    );

    let quota = InertAdapter.collect_quota_report();
    assert!(quota.report.is_none());
    assert_eq!(quota.outcome.error_code, "NOT_SUPPORTED");

    let health = InertAdapter.health_check();
    assert_eq!(
        health.status,
        AdapterHealthStatus::Unsupported,
        "an adapter that collects nothing must never report itself healthy"
    );

    let detection = InertAdapter.detect().expect("detection must not fail");
    assert!(!detection.detected);
    assert_eq!(detection.detection_error_code, "NOT_SUPPORTED");
}

/// A supported channel that returns `Ok(None)` means the source was missing,
/// which must stay distinguishable from an unimplemented integration.
#[test]
fn supported_channel_without_source_reports_source_unavailable() {
    struct SourcelessAdapter;
    impl ProviderAdapter for SourcelessAdapter {
        fn descriptor(&self) -> AdapterDescriptor {
            AdapterDescriptor {
                quota_support: ChannelSupport::LocalEstimate,
                ..descriptor("sourceless", "Sourceless")
            }
        }
    }

    let quota = SourcelessAdapter.collect_quota_report();
    assert!(quota.report.is_none());
    assert_eq!(quota.outcome.error_code, "SOURCE_UNAVAILABLE");
}
