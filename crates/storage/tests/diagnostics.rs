use lnwdeck_storage::repositories::diagnostics_repository::{
    CollectorRunRow, DiagnosticsRepository, PipelineTotals, ProviderStateRow,
};
use lnwdeck_storage::repositories::AppSettingsRepository;
use lnwdeck_storage::{migrations::apply_all, Storage};
use tempfile::tempdir;

fn setup_db() -> Storage {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("apply migrations");
    storage
}

fn provider_state(provider_id: &str, detected: bool) -> ProviderStateRow {
    ProviderStateRow {
        provider_id: provider_id.to_string(),
        display_name: format!("Provider {provider_id}"),
        enabled: true,
        detected,
        detection_method: "local_sqlite".to_string(),
        source_type: "sqlite".to_string(),
        source_exists: detected,
        permission_state: "read_ok".to_string(),
        adapter_version: "0.2.0".to_string(),
        last_detection_at: Some("2026-08-03T00:00:00Z".to_string()),
        detection_error_code: String::new(),
    }
}

fn run(
    provider_id: &str,
    id_hint: u64,
    events_inserted: u64,
    error_code: &str,
    next_retry_at: Option<String>,
) -> CollectorRunRow {
    CollectorRunRow {
        id: id_hint,
        provider_id: provider_id.to_string(),
        collector_mode: "passive_scan".to_string(),
        started_at: format!("2026-08-03T00:00:{id_hint:02}Z"),
        finished_at: format!("2026-08-03T00:01:{id_hint:02}Z"),
        duration_ms: 12,
        source_records_seen: 10,
        records_parsed: 9,
        events_normalized: 8,
        events_rejected: 1,
        duplicates_skipped: 2,
        events_inserted,
        quota_snapshots_inserted: 0,
        warning_codes: vec![],
        error_code: error_code.to_string(),
        next_retry_at,
    }
}

#[test]
fn migration_002_creates_diagnostics_tables() {
    let storage = setup_db();
    let tables = storage.list_tables().expect("list tables");
    for expected in ["provider_states", "collector_runs", "app_settings"] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {expected}: {tables:?}"
        );
    }
}

#[test]
fn provider_state_upsert_is_idempotent() {
    let storage = setup_db();
    let repo = DiagnosticsRepository::new(&storage.conn);

    repo.upsert_provider_state(&provider_state("opencode_cli", true))
        .expect("upsert 1");
    let mut updated = provider_state("opencode_cli", true);
    updated.detection_error_code = "LOCKED".to_string();
    repo.upsert_provider_state(&updated).expect("upsert 2");

    let states = repo.provider_states().expect("query states");
    assert_eq!(states.len(), 1, "upsert must not duplicate rows");
    assert_eq!(states[0].detection_error_code, "LOCKED");
    assert_eq!(states[0].provider_id, "opencode_cli");
}

#[test]
fn latest_runs_returns_newest_run_per_provider() {
    let storage = setup_db();
    let repo = DiagnosticsRepository::new(&storage.conn);

    repo.insert_collector_run(&run("a", 1, 5, "", None))
        .expect("run a1");
    repo.insert_collector_run(&run("a", 2, 6, "", None))
        .expect("run a2");
    repo.insert_collector_run(&run("b", 3, 7, "", None))
        .expect("run b1");

    let latest = repo.latest_runs().expect("latest runs");
    assert_eq!(latest.len(), 2, "one run per provider");
    let a = latest.iter().find(|r| r.provider_id == "a").expect("a");
    assert_eq!(a.events_inserted, 6, "newest run for provider a");
    let b = latest.iter().find(|r| r.provider_id == "b").expect("b");
    assert_eq!(b.events_inserted, 7);
}

#[test]
fn pipeline_totals_aggregate_all_runs() {
    let storage = setup_db();
    let repo = DiagnosticsRepository::new(&storage.conn);

    repo.insert_collector_run(&run("a", 1, 5, "", None))
        .expect("a1");
    repo.insert_collector_run(&run("a", 2, 6, "", None))
        .expect("a2");
    // The retry time must lie in the future: the totals query only reports
    // retries that have not passed yet, so a fixed date would make this test
    // pass or fail depending on the day it runs.
    let retry_at = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    repo.insert_collector_run(&run(
        "b",
        3,
        0,
        "SOURCE_UNAVAILABLE",
        Some(retry_at.clone()),
    ))
    .expect("b1");

    let totals: PipelineTotals = repo.pipeline_totals().expect("totals");
    assert_eq!(totals.events_seen, 30);
    assert_eq!(totals.events_parsed, 27);
    assert_eq!(totals.events_normalized, 24);
    assert_eq!(totals.events_rejected, 3);
    assert_eq!(totals.duplicates_skipped, 6);
    assert_eq!(totals.events_inserted, 11);
    assert_eq!(totals.privacy_rejections, 3);
    assert_eq!(
        totals.last_successful_sync.as_deref(),
        Some("2026-08-03T00:01:02Z"),
        "latest successful run timestamp"
    );
    assert_eq!(
        totals.next_retry_at.as_deref(),
        Some(retry_at.as_str()),
        "next retry comes from failed runs"
    );
}

#[test]
fn app_settings_roundtrip() {
    let storage = setup_db();
    let repo = AppSettingsRepository::new(&storage.conn);

    assert_eq!(repo.get("hash_key").expect("missing"), None);
    repo.set("hash_key", "deadbeef").expect("set");
    assert_eq!(
        repo.get("hash_key").expect("get").as_deref(),
        Some("deadbeef")
    );
}

#[test]
fn diagnostics_rows_serialize_without_forbidden_fields() {
    let storage = setup_db();
    let repo = DiagnosticsRepository::new(&storage.conn);
    repo.upsert_provider_state(&provider_state("opencode_cli", true))
        .expect("state");
    repo.insert_collector_run(&run("opencode_cli", 1, 5, "", None))
        .expect("run");

    let states_json = serde_json::to_value(repo.provider_states().expect("states")).expect("json");
    let runs_json = serde_json::to_value(repo.latest_runs().expect("runs")).expect("json");
    for json in [&states_json, &runs_json] {
        let s = serde_json::to_string(json).expect("string");
        for forbidden in [
            "prompt",
            "response",
            "path",
            "file_name",
            "cookie",
            "token",
            "api_key",
        ] {
            assert!(
                !s.to_lowercase().contains(forbidden),
                "forbidden marker {forbidden} in diagnostics JSON"
            );
        }
    }
}
