use chrono::DateTime;
use lnwdeck_application::refresh::RefreshAll;
use lnwdeck_domain::{Confidence, QuotaSnapshot, UsageBatch, UsageEvent};
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
            adapter_version: "0.1.0".to_string(),
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
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
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
            adapter_version: "0.1.0".to_string(),
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
    let outcomes = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(outcomes.len(), 1);
    let outcome = &outcomes[0];
    assert_eq!(outcome.provider_id, "fake_provider");
    assert_eq!(outcome.events_inserted, 2);
    assert_eq!(outcome.error_code, "");

    assert_eq!(event_count(&storage.conn), 2);

    let diag = DiagnosticsRepository::new(&storage.conn);
    let states = diag.provider_states().expect("states");
    assert_eq!(states.len(), 1);
    assert!(states[0].detected, "detection must be persisted");

    let runs = diag.latest_runs().expect("runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].events_inserted, 2);

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
    let outcomes = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(outcomes[0].events_inserted, 0, "no new rows on repeat");
    assert_eq!(outcomes[0].duplicates_skipped, 2);
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
    let outcomes = RefreshAll::execute(&storage.conn, &adapters);

    assert_eq!(outcomes.len(), 2, "both adapters produce outcomes");
    let failed = outcomes
        .iter()
        .find(|o| o.provider_id == "failing_provider")
        .expect("failed outcome");
    assert_eq!(failed.error_code, "SOURCE_UNAVAILABLE");
    assert_eq!(failed.events_inserted, 0);

    let ok = outcomes
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
    assert_eq!(runs.len(), 2, "every attempt recorded");
}

#[test]
fn privacy_violation_rejects_batch_and_is_recorded() {
    let storage = setup_db();
    let adapters: Vec<&dyn ProviderAdapter> = vec![&ViolatingAdapter];
    let outcomes = RefreshAll::execute(&storage.conn, &adapters);

    let outcome = &outcomes[0];
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
