use chrono::DateTime;
use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use lnwdeck_storage::repositories::UsageRepository;
use lnwdeck_storage::{migrations::apply_all, Storage};
use tempfile::tempdir;

fn sample_event(id: &str, provider: &str) -> UsageEvent {
    UsageEvent {
        id: id.to_string(),
        timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        provider_id: provider.to_string(),
        model: "gpt-4o".to_string(),
        tokens_input: 100,
        tokens_cached: 0,
        tokens_cache_write: 0,
        tokens_output: 50,
        tokens_reasoning: 0,
        confidence: Confidence::High,
        data_source: "web".to_string(),
        cost: "0.005".to_string(),
        session_hash: None,
        project_hash: None,
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

#[test]
fn replace_provider_batch_removes_legacy_snapshot_rows() {
    let storage = setup_db();
    let repo = UsageRepository::new(&storage.conn);

    let mut legacy = sample_event("legacy", "openai_codex");
    legacy.data_source = "local_jsonl".to_string();
    repo.ingest_batch(&UsageBatch {
        batch_id: "legacy_batch".to_string(),
        events: vec![legacy],
    })
    .expect("ingest legacy snapshot");

    let mut current = sample_event("current", "openai_codex");
    current.data_source = "local_jsonl_v2".to_string();
    current.tokens_input = 1000;
    current.tokens_cached = 9000;
    current.tokens_cache_write = 20;
    current.tokens_output = 400;
    current.tokens_reasoning = 120;
    repo.replace_provider_batch(
        &UsageBatch {
            batch_id: "current_batch".to_string(),
            events: vec![current],
        },
        "openai_codex",
        "local_jsonl_v2",
        "local_jsonl",
    )
    .expect("replace provider snapshot");

    let (count, legacy_count, total): (i64, i64, i64) = storage
        .conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN data_source = 'local_jsonl' THEN 1 ELSE 0 END),
                    SUM(tokens_input + tokens_cached + tokens_cache_write + tokens_output)
             FROM usage_events WHERE provider_id = 'openai_codex'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query replacement");

    assert_eq!(count, 1);
    assert_eq!(legacy_count, 0);
    assert_eq!(total, 10420);
}
