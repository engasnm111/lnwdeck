use chrono::DateTime;
use inwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use inwdeck_storage::repositories::UsageRepository;
use inwdeck_storage::{migrations::apply_all, Storage};
use tempfile::tempdir;

fn sample_event(id: &str, provider: &str) -> UsageEvent {
    UsageEvent {
        id: id.to_string(),
        timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        provider_id: provider.to_string(),
        model: "gpt-4o".to_string(),
        tokens_input: 100,
        tokens_output: 50,
        confidence: Confidence::High,
        data_source: "web".to_string(),
        cost: "0.005".to_string(),
    }
}

fn setup_db() -> Storage {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("apply migrations");
    storage
}

#[test]
fn ingest_single_batch() {
    let storage = setup_db();
    let repo = UsageRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![sample_event("evt_1", "openai")],
    };

    repo.ingest_batch(&batch).expect("ingest");
}

#[test]
fn ingest_same_batch_twice_is_idempotent() {
    let storage = setup_db();
    let repo = UsageRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![
            sample_event("evt_a", "openai"),
            sample_event("evt_b", "openai"),
        ],
    };

    repo.ingest_batch(&batch).expect("first ingest");
    repo.ingest_batch(&batch).expect("second ingest");

    let count: i64 = storage
        .conn
        .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
        .expect("count");

    assert_eq!(count, 2, "duplicate events must not be inserted");
}

#[test]
fn aggregate_queries_work() {
    let storage = setup_db();
    let repo = UsageRepository::new(&storage.conn);

    let batch1 = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![
            sample_event("evt_1", "openai"),
            sample_event("evt_2", "anthropic"),
        ],
    };
    let batch2 = UsageBatch {
        batch_id: "batch_2".to_string(),
        events: vec![sample_event("evt_3", "openai")],
    };

    repo.ingest_batch(&batch1).expect("ingest 1");
    repo.ingest_batch(&batch2).expect("ingest 2");

    let total_tokens: i64 = storage
        .conn
        .query_row(
            "SELECT SUM(tokens_input + tokens_output) FROM usage_events",
            [],
            |row| row.get(0),
        )
        .expect("sum");

    assert_eq!(total_tokens, 450);

    let openai_count: i64 = storage
        .conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE provider_id = 'openai'",
            [],
            |row| row.get(0),
        )
        .expect("count");

    assert_eq!(openai_count, 2);
}
