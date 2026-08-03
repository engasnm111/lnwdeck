use chrono::DateTime;
use lnwdeck_application::refresh::RefreshAll;
use lnwdeck_domain::{Confidence, QuotaReport, UsageBatch, UsageEvent};
use lnwdeck_provider_runtime::{
    AdapterHealth, AdapterHealthStatus, DetectionResult, Permission, ProviderAdapter,
};
use lnwdeck_storage::repositories::DiagnosticsRepository;
use lnwdeck_storage::{migrations::apply_all, Storage};
use tempfile::tempdir;

fn event(id: &str, model: &str, input: u64, output: u64) -> UsageEvent {
    UsageEvent {
        id: id.to_string(),
        timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        provider_id: "fake_provider".to_string(),
        model: model.to_string(),
        tokens_input: input,
        tokens_output: output,
        confidence: Confidence::High,
        data_source: "fixture".to_string(),
        cost: "0.001".to_string(),
    }
}

struct SuccessAdapter;

impl ProviderAdapter for SuccessAdapter {
    fn id(&self) -> &str {
        "fake_provider"
    }
    fn name(&self) -> &str {
        "Fake Provider"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "fixture_batch".to_string(),
            events: vec![
                event("evt_1", "model-a", 100, 50),
                event("evt_2", "model-b", 200, 60),
            ],
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
    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(DetectionResult {
            provider_id: "fake_provider".to_string(),
            display_name: "Fake Provider".to_string(),
            enabled: true,
            detected: true,
            detection_method: "fixture".to_string(),
            source_type: "fixture".to_string(),
            source_exists: true,
            permission_state: "read_ok".to_string(),
            adapter_version: "0.2.0".to_string(),
            last_detection_at: Some("2026-08-03T00:00:00Z".to_string()),
            detection_error_code: String::new(),
        })
    }
    fn collect_usage_with_cursor(
        &self,
        _cursor: Option<&str>,
    ) -> lnwdeck_provider_runtime::CollectionResult {
        let started_at = chrono::Utc::now();
        let mut base = lnwdeck_provider_runtime::CollectionResult::from_basic(
            self.id(),
            "passive_scan",
            started_at,
            self.collect_usage(),
            _cursor,
        );
        base.outcome.source_records_seen = 2;
        base.outcome.records_parsed = 2;
        base.next_cursor = Some("cursor_100".to_string());
        base
    }
}

struct FailingAdapter;

impl ProviderAdapter for FailingAdapter {
    fn id(&self) -> &str {
        "failing_provider"
    }
    fn name(&self) -> &str {
        "Failing Provider"
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
            message: "down".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(DetectionResult {
            provider_id: "failing_provider".to_string(),
            display_name: "Failing Provider".to_string(),
            enabled: true,
            detected: false,
            detection_method: "fixture".to_string(),
            source_type: "fixture".to_string(),
            source_exists: false,
            permission_state: "n/a".to_string(),
            adapter_version: "0.2.0".to_string(),
            last_detection_at: Some("2026-08-03T00:00:00Z".to_string()),
            detection_error_code: String::new(),
        })
    }
}

struct ViolatingAdapter;

impl ProviderAdapter for ViolatingAdapter {
    fn id(&self) -> &str {
        "violating_provider"
    }
    fn name(&self) -> &str {
        "Violating Provider"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let mut evt = event("evt_bad", "model-x", 1, 1);
        evt.cost = "C:\\Users\\someone\\passwords.txt".to_string();
        Ok(UsageBatch {
            batch_id: "bad_batch".to_string(),
            events: vec![evt],
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

fn setup_db() -> Storage {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("apply migrations");
    storage
}

fn event_count(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
        .expect("count")
}

#[test]
fn refresh_persists_detection_collection_and_events() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&SuccessAdapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(cycle.usage.len(), 1);
    let outcome = &cycle.usage[0];
    assert_eq!(outcome.provider_id, "fake_provider");
    assert_eq!(outcome.events_inserted, 2);
    assert_eq!(outcome.error_code, "");

    assert_eq!(event_count(&storage.conn), 2);

    let diag = DiagnosticsRepository::new(&storage.conn);
    let states = diag.provider_states().expect("states");
    assert_eq!(states.len(), 1);
    assert!(states[0].detected, "detection must be persisted");

    let runs = diag.latest_runs().expect("runs");
    assert_eq!(runs.len(), 2, "one usage run and one quota run");
    let usage_run = runs
        .iter()
        .find(|r| r.collector_mode != "quota_collect")
        .expect("usage run");
    assert_eq!(usage_run.events_inserted, 2);

    let quota_outcome = &cycle.quota[0];
    assert_eq!(
        quota_outcome.error_code, "UNSUPPORTED",
        "adapter without quota support reports unsupported"
    );
    assert_eq!(quota_outcome.windows_collected, 0);

    let cursor: String = storage
        .conn
        .query_row(
            "SELECT cursor_value FROM sync_cursors WHERE provider_id = 'fake_provider'",
            [],
            |row| row.get(0),
        )
        .expect("cursor");
    assert_eq!(cursor, "cursor_100");
}

#[test]
fn refresh_is_idempotent_and_counts_duplicates() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&SuccessAdapter];

    RefreshAll::execute(&storage.conn, &adapters);
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(cycle.usage[0].events_inserted, 0, "no new rows on repeat");
    assert_eq!(cycle.usage[0].duplicates_skipped, 2);
    assert_eq!(event_count(&storage.conn), 2, "rows must not duplicate");

    let diag = DiagnosticsRepository::new(&storage.conn);
    let totals = diag.pipeline_totals().expect("totals");
    assert_eq!(totals.events_seen, 4, "totals aggregate both runs");
    assert_eq!(totals.events_inserted, 2);
    assert_eq!(totals.duplicates_skipped, 2);
}

#[test]
fn failing_adapter_is_isolated_and_recorded() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&FailingAdapter, &SuccessAdapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(cycle.usage.len(), 2, "both adapters produce outcomes");
    let failed = cycle
        .usage
        .iter()
        .find(|o| o.provider_id == "failing_provider")
        .expect("failed outcome");
    assert_eq!(failed.error_code, "SOURCE_UNAVAILABLE");
    assert_eq!(failed.events_inserted, 0);

    let ok = cycle
        .usage
        .iter()
        .find(|o| o.provider_id == "fake_provider")
        .expect("ok outcome");
    assert_eq!(ok.error_code, "");

    assert_eq!(
        event_count(&storage.conn),
        2,
        "successful adapter still ingested"
    );

    let diag = DiagnosticsRepository::new(&storage.conn);
    let runs = diag.latest_runs().expect("runs");
    assert_eq!(runs.len(), 4, "usage + quota run per adapter recorded");
}

#[test]
fn privacy_violation_rejects_batch_and_is_recorded() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&ViolatingAdapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    let outcome = &cycle.usage[0];
    assert_eq!(outcome.error_code, "PRIVACY_VIOLATION");
    assert_eq!(outcome.events_rejected, 1);
    assert_eq!(outcome.events_inserted, 0);
    assert_eq!(
        event_count(&storage.conn),
        0,
        "unsafe payload must not persist"
    );

    let diag = DiagnosticsRepository::new(&storage.conn);
    let totals = diag.pipeline_totals().expect("totals");
    assert_eq!(totals.privacy_rejections, 1);
}

struct QuotaAdapter;

impl ProviderAdapter for QuotaAdapter {
    fn id(&self) -> &str {
        "quota_provider"
    }
    fn name(&self) -> &str {
        "Quota Provider"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "empty".to_string(),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<lnwdeck_domain::QuotaReport>, String> {
        let window = lnwdeck_domain::QuotaWindow::new(
            "5h",
            "5-hour",
            lnwdeck_domain::QuotaWindowScope::Rolling,
            lnwdeck_domain::QuotaKind::Requests,
            40,
            100,
            None,
            Confidence::High,
        );
        let window2 = lnwdeck_domain::QuotaWindow::new(
            "7d",
            "7-day",
            lnwdeck_domain::QuotaWindowScope::Weekly,
            lnwdeck_domain::QuotaKind::Requests,
            300,
            1000,
            None,
            Confidence::High,
        );
        Ok(Some(lnwdeck_domain::QuotaReport::new(
            "quota_provider",
            "fixture_api",
            vec![window, window2],
            chrono::Duration::hours(1),
        )))
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

struct UsageOkQuotaFailingAdapter;

impl ProviderAdapter for UsageOkQuotaFailingAdapter {
    fn id(&self) -> &str {
        "mixed_provider"
    }
    fn name(&self) -> &str {
        "Mixed Provider"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "mixed_batch".to_string(),
            events: vec![event("evt_mixed", "model-m", 10, 5)],
        })
    }
    fn collect_quota(&self) -> Result<Option<lnwdeck_domain::QuotaReport>, String> {
        Err("AUTH_EXPIRED".to_string())
    }
    fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: AdapterHealthStatus::Degraded,
            message: "auth".to_string(),
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }
}

struct QuotaLeakingAdapter;

impl ProviderAdapter for QuotaLeakingAdapter {
    fn id(&self) -> &str {
        "leaking_provider"
    }
    fn name(&self) -> &str {
        "Leaking Provider"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Ok(UsageBatch {
            batch_id: "leak_batch".to_string(),
            events: vec![],
        })
    }
    fn collect_quota(&self) -> Result<Option<lnwdeck_domain::QuotaReport>, String> {
        let mut report = lnwdeck_domain::QuotaReport::new(
            "leaking_provider",
            "fixture_api",
            vec![],
            chrono::Duration::hours(1),
        );
        report.plan = Some("C:\\Users\\someone\\passwords.txt".to_string());
        Ok(Some(report))
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
fn quota_report_is_persisted_by_refresh_cycle() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&QuotaAdapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    let quota = &cycle.quota[0];
    assert_eq!(quota.windows_collected, 2);
    assert_eq!(quota.error_code, "");

    let report = lnwdeck_storage::repositories::QuotaRepository::new(&storage.conn)
        .latest_report("quota_provider")
        .expect("latest")
        .expect("report exists");
    assert_eq!(report.windows.len(), 2);
    assert_eq!(report.windows[0].remaining, 60);
    assert!(report.is_usable());

    let diag = DiagnosticsRepository::new(&storage.conn);
    let runs = diag.latest_runs().expect("runs");
    assert_eq!(runs.len(), 2, "usage + quota run recorded");
    let quota_run = runs
        .iter()
        .find(|r| r.collector_mode == "quota_collect")
        .expect("quota run");
    assert_eq!(quota_run.quota_snapshots_inserted, 2);
    assert_eq!(quota_run.error_code, "");
}

#[test]
fn quota_failure_does_not_erase_usage_data() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&UsageOkQuotaFailingAdapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(
        cycle.usage[0].events_inserted, 1,
        "usage channel unaffected"
    );
    assert_eq!(cycle.quota[0].error_code, "AUTH_EXPIRED");
    assert!(cycle.quota[0].status.is_error());

    assert_eq!(event_count(&storage.conn), 1);
    let report = lnwdeck_storage::repositories::QuotaRepository::new(&storage.conn)
        .latest_report("mixed_provider")
        .expect("latest");
    assert!(report.is_none(), "failed quota must not persist a report");
}

#[test]
fn quota_privacy_violation_is_rejected_and_recorded() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&QuotaLeakingAdapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(cycle.quota[0].error_code, "PRIVACY_VIOLATION");
    let report = lnwdeck_storage::repositories::QuotaRepository::new(&storage.conn)
        .latest_report("leaking_provider")
        .expect("latest");
    assert!(report.is_none(), "unsafe quota payload must not persist");
}

#[test]
fn refresh_provider_isolates_a_single_adapter() {
    let storage = setup_db();
    let cycle = RefreshAll::refresh_provider(&storage.conn, &SuccessAdapter);

    assert_eq!(cycle.usage.len(), 1, "exactly one usage outcome");
    assert_eq!(cycle.quota.len(), 1, "exactly one quota outcome");
    assert_eq!(cycle.usage[0].provider_id, "fake_provider");
    assert_eq!(cycle.usage[0].events_inserted, 2);
    assert_eq!(cycle.quota[0].error_code, "UNSUPPORTED");

    let failing_runs: i64 = storage
        .conn
        .query_row(
            "SELECT COUNT(*) FROM collector_runs WHERE provider_id = 'failing_provider'",
            [],
            |row| row.get(0),
        )
        .expect("count failing runs");
    assert_eq!(
        failing_runs, 0,
        "other providers must not be touched by a single-provider refresh"
    );
}
