use chrono::{Duration, Utc};
use lnwdeck_application::dashboard::{DashboardQuery, DashboardRange, QueryDashboard};
use lnwdeck_storage::{migrations::apply_all, Storage};
use tempfile::tempdir;

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
    assert_eq!(dashboard.sessions[0].providers.len(), 2);
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
