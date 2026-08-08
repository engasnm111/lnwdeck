use chrono::DateTime;
use lnwdeck_application::overview::QueryOverview;
use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use lnwdeck_storage::{migrations::apply_all, repositories::UsageRepository, Storage};
use tempfile::tempdir;

fn sample_event(id: &str, provider: &str, model: &str, input: u64, output: u64) -> UsageEvent {
    UsageEvent {
        id: id.to_string(),
        timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        provider_id: provider.to_string(),
        model: model.to_string(),
        tokens_input: input,
        tokens_cached: 0,
        tokens_cache_write: 0,
        tokens_output: output,
        tokens_reasoning: 0,
        confidence: Confidence::High,
        data_source: "web".to_string(),
        cost: "0.005".to_string(),
        session_hash: None,
        project_hash: None,
    }
}

#[test]
fn overview_returns_totals_across_providers() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();
    let repo = UsageRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![
            sample_event("evt_01", "openai", "gpt-4o", 100, 50),
            sample_event("evt_02", "anthropic", "claude-3", 200, 100),
            sample_event("evt_03", "openai", "gpt-4o", 50, 30),
        ],
    };

    repo.ingest_batch(&batch).unwrap();

    let overview = QueryOverview::execute(&storage.conn).unwrap();

    assert_eq!(overview.total_events, 3);
    assert_eq!(overview.total_tokens_input, 350);
    assert_eq!(overview.total_tokens_output, 180);
    assert_eq!(overview.provider_count, 2);
}

#[test]
fn overview_tracks_top_providers() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();
    let repo = UsageRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![
            sample_event("e1", "openai", "gpt-4o", 100, 50),
            sample_event("e2", "openai", "gpt-3.5", 50, 25),
            sample_event("e3", "anthropic", "claude-3", 200, 100),
        ],
    };

    repo.ingest_batch(&batch).unwrap();

    let overview = QueryOverview::execute(&storage.conn).unwrap();

    let top = overview.top_providers;
    assert!(!top.is_empty(), "top providers must not be empty");
    assert_eq!(
        top[0].provider_id, "openai",
        "openai should be top with 2 events"
    );
    assert_eq!(top[0].event_count, 2);
}

#[test]
fn overview_has_freshness() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();
    let repo = UsageRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![sample_event("e1", "openai", "gpt-4o", 100, 50)],
    };

    repo.ingest_batch(&batch).unwrap();

    let overview = QueryOverview::execute(&storage.conn).unwrap();

    assert!(
        overview.latest_event_at.is_some(),
        "must have latest event timestamp"
    );
    assert!(
        overview.oldest_event_at.is_some(),
        "must have oldest event timestamp"
    );
}

#[test]
fn overview_confidence_coverage_is_tracked() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();
    let repo = UsageRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![sample_event("e1", "openai", "gpt-4o", 100, 50)],
    };

    repo.ingest_batch(&batch).unwrap();

    let overview = QueryOverview::execute(&storage.conn).unwrap();

    assert!(overview.high_confidence_count > 0);
    assert!(overview.confidence_coverage > 0.0);
}
