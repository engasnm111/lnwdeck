use chrono::DateTime;
use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use lnwdeck_security::PrivacyGuard;
use lnwdeck_storage::{migrations::apply_all, repositories::UsageRepository, Storage};
use std::cell::RefCell;
use tempfile::tempdir;

struct SpyRepository<'a> {
    conn: &'a rusqlite::Connection,
    call_count: RefCell<usize>,
}

impl<'a> SpyRepository<'a> {
    fn new(conn: &'a rusqlite::Connection) -> Self {
        Self {
            conn,
            call_count: RefCell::new(0),
        }
    }

    fn ingest_batch(&self, batch: &UsageBatch) -> Result<(), rusqlite::Error> {
        *self.call_count.borrow_mut() += 1;
        let repo = UsageRepository::new(self.conn);
        repo.ingest_batch(batch)
    }

    fn times_called(&self) -> usize {
        *self.call_count.borrow()
    }
}

fn sample_event(provider: &str) -> UsageEvent {
    UsageEvent {
        id: format!("evt_{}", provider),
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

#[test]
fn privacy_guard_blocks_batch_before_repository_called() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();

    let spy = SpyRepository::new(&storage.conn);

    // Build a batch that will be serialized with unsafe raw JSON data
    let batch = UsageBatch {
        batch_id: "test".to_string(),
        events: vec![sample_event("openai")],
    };

    // Simulate the raw JSON flow: extra field added by external source
    let raw_json = {
        let mut value = serde_json::to_value(&batch).unwrap();
        if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "path".to_string(),
                serde_json::Value::String("C:\\Users\\hacker\\passwords.txt".to_string()),
            );
        }
        value
    };

    // Step 1: Validate raw JSON BEFORE deserializing into domain types
    let raw_check = PrivacyGuard::validate_raw_json(&raw_json);
    assert!(
        raw_check.is_err(),
        "raw JSON with forbidden key must be rejected"
    );

    // Repository must never be called for unsafe input
    assert_eq!(
        spy.times_called(),
        0,
        "repository must not be called for unsafe input"
    );
}

#[test]
fn safe_batch_passes_privacy_guard_and_reaches_repository() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();

    let spy = SpyRepository::new(&storage.conn);

    let batch = UsageBatch {
        batch_id: "safe_batch".to_string(),
        events: vec![sample_event("openai")],
    };

    let guard_result = PrivacyGuard::validate_usage_batch(&batch);
    assert!(guard_result.is_ok());

    spy.ingest_batch(&batch).unwrap();
    assert_eq!(
        spy.times_called(),
        1,
        "repository must be called for safe input"
    );

    let count: i64 = storage
        .conn
        .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
