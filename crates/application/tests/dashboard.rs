use chrono::{Duration, Local, TimeZone, Utc};
use lnwdeck_application::dashboard::{DashboardQuery, DashboardRange, QueryDashboard};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterRegistry, AuthKind, ChannelSupport, ProviderAdapter, SourceKind,
};
use lnwdeck_storage::{migrations::apply_all, Storage};
use tempfile::tempdir;

struct FixtureAdapter(AdapterDescriptor);

impl ProviderAdapter for FixtureAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        self.0
    }
}

fn registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry
        .register(Box::new(FixtureAdapter(AdapterDescriptor {
            id: "claude",
            display_name: "Claude",
            vendor: "Anthropic",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Unsupported,
            auth: AuthKind::LocalFiles,
            adapter_version: "test",
        })))
        .expect("register fixture provider");
    registry
}

fn open_db() -> Storage {
    let dir = tempdir().expect("temp dir");
    let dir = std::mem::ManuallyDrop::new(dir);
    let storage = Storage::open(&dir.path().join("dashboard.db")).expect("open");
    apply_all(&storage.conn).expect("migrate");
    storage
}

fn insert_event(
    storage: &Storage,
    id: &str,
    provider: &str,
    session: &str,
    timestamp: String,
    input: i64,
    output: i64,
) {
    storage
        .conn
        .execute(
            "INSERT INTO usage_events
                (id, batch_id, timestamp, provider_id, model, tokens_input,
                 tokens_output, confidence, data_source, cost, session_hash,
                 project_hash)
             VALUES (?1, ?1, ?2, ?3, 'model', ?4, ?5, 'High', 'fixture', '0', ?6, '')",
            rusqlite::params![id, timestamp, provider, input, output, session],
        )
        .expect("insert event");
}

#[test]
fn dashboard_returns_provider_totals_trend_and_one_row_per_session() {
    let storage = open_db();
    let now = Utc::now();
    insert_event(
        &storage,
        "evt_a",
        "claude",
        "session-a",
        (now - Duration::hours(2)).to_rfc3339(),
        100,
        50,
    );
    insert_event(
        &storage,
        "evt_b",
        "codex",
        "session-a",
        (now - Duration::hours(1)).to_rfc3339(),
        200,
        25,
    );
    insert_event(
        &storage,
        "evt_c",
        "claude",
        "session-b",
        now.to_rfc3339(),
        10,
        5,
    );

    let dashboard = QueryDashboard::execute(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Total,
            start: None,
            end: None,
            provider_id: None,
        },
    )
    .expect("dashboard");

    assert_eq!(dashboard.total_tokens, 390);
    assert_eq!(dashboard.providers.len(), 2);
    assert_eq!(dashboard.sessions.len(), 2);
    assert_eq!(
        dashboard.sessions[0].session_hash, "session-b",
        "the session table must place the most recently active session first"
    );
    assert_eq!(dashboard.sessions[1].providers.len(), 2);
    assert!(!dashboard.trend.is_empty());
    assert!(!dashboard.heatmap.is_empty());
}

#[test]
fn dashboard_provider_filter_is_applied_to_every_section() {
    let storage = open_db();
    let now = Utc::now().to_rfc3339();
    insert_event(
        &storage,
        "evt_a",
        "claude",
        "session-a",
        now.clone(),
        100,
        0,
    );
    insert_event(&storage, "evt_b", "codex", "session-b", now, 200, 0);

    let dashboard = QueryDashboard::execute(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Total,
            start: None,
            end: None,
            provider_id: Some("claude".to_string()),
        },
    )
    .expect("dashboard");

    assert_eq!(dashboard.total_tokens, 100);
    assert_eq!(dashboard.providers.len(), 1);
    assert_eq!(dashboard.providers[0].provider_id, "claude");
    assert_eq!(dashboard.sessions.len(), 1);
    assert!(dashboard.sessions[0]
        .providers
        .iter()
        .all(|provider| provider.provider_id == "claude"));
}

#[test]
fn dashboard_merges_legacy_opencode_rows_into_the_canonical_provider() {
    let storage = open_db();
    let now = Utc::now().to_rfc3339();
    insert_event(
        &storage,
        "evt_opencode_current",
        "opencode",
        "session-opencode",
        now.clone(),
        100,
        0,
    );
    insert_event(
        &storage,
        "evt_opencode_legacy",
        "opencode_cli",
        "session-opencode",
        now,
        200,
        0,
    );

    let dashboard = QueryDashboard::execute(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Total,
            start: None,
            end: None,
            provider_id: Some("opencode".to_string()),
        },
    )
    .expect("dashboard");

    assert_eq!(dashboard.total_tokens, 300);
    assert_eq!(dashboard.providers.len(), 1);
    assert_eq!(dashboard.providers[0].provider_id, "opencode");
    assert_eq!(dashboard.providers[0].total_tokens, 300);
    assert_eq!(dashboard.sessions.len(), 1);
    assert_eq!(dashboard.sessions[0].providers.len(), 1);
    assert_eq!(dashboard.sessions[0].providers[0].provider_id, "opencode");
}

#[test]
fn dashboard_month_range_is_trailing_30_days_and_uses_provider_identity() {
    let storage = open_db();
    let now = Utc::now();
    insert_event(
        &storage,
        "evt_month",
        "claude",
        "session-month",
        now.to_rfc3339(),
        1_234,
        567,
    );

    let dashboard = QueryDashboard::execute_with_registry(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Month,
            start: None,
            end: None,
            provider_id: None,
        },
        &registry(),
    )
    .expect("dashboard");

    let local_today = Local::now().date_naive();
    let first = local_today - Duration::days(29);
    let next_day = local_today.succ_opt().expect("next day");
    let first_key = first.format("%Y-%m-%d").to_string();
    let today_key = local_today.format("%Y-%m-%d").to_string();

    assert_eq!(
        dashboard.heatmap.len(),
        (next_day - first).num_days() as usize
    );
    assert_eq!(
        dashboard.heatmap.first().map(|cell| cell.day.as_str()),
        Some(first_key.as_str())
    );
    assert_eq!(
        dashboard.heatmap.last().map(|cell| cell.day.as_str()),
        Some(today_key.as_str())
    );
    assert!(dashboard.heatmap.iter().any(|cell| cell.total_tokens == 0));
    assert_eq!(dashboard.providers[0].display_name, "Claude");
    assert_eq!(dashboard.providers[0].vendor, "Anthropic");
    assert_eq!(dashboard.sessions[0].providers[0].display_name, "Claude");
}

#[test]
fn dashboard_week_range_is_trailing_seven_days() {
    let storage = open_db();
    let local_today = Local::now().date_naive();
    let local_noon = Local
        .from_local_datetime(&local_today.and_hms_opt(12, 0, 0).expect("local noon"))
        .single()
        .expect("unambiguous local noon");
    insert_event(
        &storage,
        "evt_week",
        "claude",
        "session-week",
        local_noon.with_timezone(&Utc).to_rfc3339(),
        1,
        0,
    );

    let dashboard = QueryDashboard::execute(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Week,
            start: None,
            end: None,
            provider_id: None,
        },
    )
    .expect("dashboard");

    let first = local_today - Duration::days(6);
    let first_key = first.format("%Y-%m-%d").to_string();
    let today_key = local_today.format("%Y-%m-%d").to_string();
    assert_eq!(dashboard.heatmap.len(), 7);
    assert_eq!(
        dashboard.heatmap.first().map(|cell| cell.day.as_str()),
        Some(first_key.as_str())
    );
    assert_eq!(
        dashboard.heatmap.last().map(|cell| cell.day.as_str()),
        Some(today_key.as_str())
    );
}

#[test]
fn dashboard_day_includes_a_utc_timestamp_that_lands_on_today_locally() {
    // Stored timestamps are canonical UTC RFC3339 (`+00:00`), the form the
    // refresh pipeline writes via `DateTime<Utc>::to_rfc3339()`. The
    // index-friendly dashboard comparison relies on this storage contract, so
    // the local-day boundary is exercised with the canonical representation.
    let storage = open_db();
    let local_today = Local::now().date_naive();
    let local_noon = Local
        .from_local_datetime(&local_today.and_hms_opt(12, 0, 0).expect("local noon"))
        .single()
        .expect("unambiguous local noon");
    let timestamp = local_noon.with_timezone(&Utc).to_rfc3339();
    insert_event(
        &storage,
        "evt_today",
        "claude",
        "session-today",
        timestamp,
        8_000_000,
        1_000_000,
    );

    let dashboard = QueryDashboard::execute(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Day,
            start: None,
            end: None,
            provider_id: None,
        },
    )
    .expect("dashboard");

    assert_eq!(dashboard.total_tokens, 9_000_000);
    assert_eq!(dashboard.request_count, 1);
}

#[test]
fn dashboard_time_filter_uses_the_timestamp_index() {
    let storage = open_db();
    let now = Utc::now();
    insert_event(
        &storage,
        "evt_index",
        "claude",
        "session-index",
        now.to_rfc3339(),
        10,
        5,
    );

    let summary_sql = format!(
        "SELECT COUNT(*) FROM usage_events
         WHERE {} AND {}",
        lnwdeck_application::dashboard::time_filter_sql(),
        "(?3 = '' OR provider_id = ?3 OR (?3 = 'opencode' AND provider_id = 'opencode_cli'))"
    );
    let plan: Vec<String> = storage
        .conn
        .prepare(&format!("EXPLAIN QUERY PLAN {summary_sql}"))
        .expect("prepare explain")
        // EXPLAIN QUERY PLAN still requires the bound parameters to plan the
        // statement, so pass a representative month window.
        .query_map(
            rusqlite::params![
                "2026-08-01T00:00:00+00:00",
                "2026-09-01T00:00:00+00:00",
                "claude"
            ],
            |row| row.get::<_, String>(3),
        )
        .expect("query map")
        .collect::<Result<Vec<_>, _>>()
        .expect("plan rows");
    assert!(
        plan.iter()
            .any(|detail| detail.contains("USING INDEX idx_usage_timestamp")),
        "dashboard summary must search the timestamp index, got: {plan:?}"
    );
    assert!(
        plan.iter()
            .all(|detail| !detail.contains("SCAN usage_events")),
        "dashboard summary must not scan the table, got: {plan:?}"
    );
}

#[test]
fn dashboard_keeps_cache_and_reasoning_breakdown_in_totals() {
    let storage = open_db();
    storage
        .conn
        .execute(
            "INSERT INTO usage_events
                (id, batch_id, timestamp, provider_id, model, tokens_input,
                 tokens_cached, tokens_cache_write, tokens_output, tokens_reasoning,
                 confidence, data_source, cost, session_hash, project_hash)
             VALUES ('evt_breakdown', 'batch_breakdown', ?1, 'claude', 'model',
                     1000, 9000, 20, 400, 120, 'High', 'fixture', '0', 'session-breakdown', '')",
            [Utc::now().to_rfc3339()],
        )
        .expect("insert breakdown event");

    let dashboard = QueryDashboard::execute(
        &storage.conn,
        DashboardQuery {
            range: DashboardRange::Total,
            start: None,
            end: None,
            provider_id: None,
        },
    )
    .expect("dashboard");

    assert_eq!(dashboard.tokens_input, 1_000);
    assert_eq!(dashboard.tokens_cached, 9_000);
    assert_eq!(dashboard.tokens_cache_write, 20);
    assert_eq!(dashboard.tokens_output, 400);
    assert_eq!(dashboard.tokens_reasoning, 120);
    assert_eq!(dashboard.total_tokens, 10_420);
}
