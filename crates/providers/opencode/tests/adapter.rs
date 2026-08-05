use chrono::Datelike;
use lnwdeck_provider_opencode::OpenCodeAdapter;
use lnwdeck_provider_runtime::{AdapterHealthStatus, ProviderAdapter};
use lnwdeck_security::PrivacyGuard;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const HASH_KEY: &[u8] = b"test-fixture-key-0000000000000000";

fn create_fixture_db(dir: &Path) -> PathBuf {
    let db_path = dir.join("opencode.db");
    let conn = Connection::open(&db_path).expect("open fixture db");
    conn.execute_batch(
        "CREATE TABLE session (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            model TEXT,
            cost REAL NOT NULL DEFAULT 0,
            tokens_input INTEGER NOT NULL DEFAULT 0,
            tokens_output INTEGER NOT NULL DEFAULT 0,
            tokens_reasoning INTEGER NOT NULL DEFAULT 0,
            tokens_cache_read INTEGER NOT NULL DEFAULT 0,
            tokens_cache_write INTEGER NOT NULL DEFAULT 0,
            time_updated INTEGER NOT NULL,
            time_archived INTEGER
        );",
    )
    .expect("create session table");
    conn.execute(
        "INSERT INTO session (id, project_id, model, cost, tokens_input, tokens_output,
                              tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated)
         VALUES ('sess_0001', 'proj_0001', 'opencode-go/test-model', 0.0012, 100, 50, 5, 10, 3, 1700000000000)",
        [],
    )
    .expect("insert session 1");
    conn.execute(
        "INSERT INTO session (id, project_id, model, cost, tokens_input, tokens_output,
                              tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated)
         VALUES ('sess_0002', 'proj_0002', 'anthropic/claude-sonnet-4', 0.0088, 400, 200, 20, 0, 0, 1700000100000)",
        [],
    )
    .expect("insert session 2");
    conn.execute(
        "INSERT INTO session (id, project_id, model, cost, tokens_input, tokens_output,
                              tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated)
         VALUES ('sess_0003', 'proj_0003', 'local-model', 0, 0, 0, 0, 0, 0, 1700000200000)",
        [],
    )
    .expect("insert zero-usage session");
    conn.close().expect("close fixture db");
    db_path
}

fn adapter_for(db_path: &Path) -> OpenCodeAdapter {
    OpenCodeAdapter::with_db_path(HASH_KEY, db_path.to_path_buf())
}

#[test]
fn detection_positive_on_fixture_database() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let result = adapter.detect().expect("detect");
    assert!(result.detected);
    assert!(result.source_exists);
    assert_eq!(result.source_type, "sqlite");
    assert_eq!(result.detection_method, "local_sqlite");
    assert_eq!(result.permission_state, "read_ok");
    assert!(result.last_detection_at.is_some());
}

#[test]
fn detection_negative_when_database_missing() {
    let dir = tempdir().expect("temp dir");
    let missing = dir.path().join("no-such-dir").join("opencode.db");
    let adapter = adapter_for(&missing);

    let result = adapter.detect().expect("detect");
    assert!(!result.detected);
    assert!(!result.source_exists);
    assert!(
        result.detection_error_code.is_empty(),
        "missing source is not an error"
    );
}

#[test]
fn detection_reports_unreadable_database() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("opencode.db");
    std::fs::write(&db_path, b"this is not a sqlite database").expect("write corrupt file");
    let adapter = adapter_for(&db_path);

    let result = adapter.detect().expect("detect");
    assert!(!result.detected);
    assert!(result.source_exists);
    assert!(
        !result.detection_error_code.is_empty(),
        "corrupt source must carry a code"
    );
}

#[test]
fn collection_normalizes_sessions_to_usage_events() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let result = adapter.collect_usage_with_cursor(None);
    let batch = result.batch.expect("batch");
    let outcome = &result.outcome;

    assert_eq!(outcome.error_code, "");
    assert_eq!(
        outcome.source_records_seen, 2,
        "zero-usage session excluded"
    );
    assert_eq!(outcome.records_parsed, 2);
    assert_eq!(outcome.events_normalized, 2);
    assert_eq!(batch.events.len(), 2);

    let first = batch
        .events
        .iter()
        .find(|e| e.model == "opencode-go/test-model")
        .expect("first event");
    assert_eq!(first.tokens_input, 100);
    assert_eq!(first.tokens_output, 55, "reasoning tokens added to output");
    assert_eq!(first.cost, "0.001200");
    assert_eq!(first.provider_id, "opencode");
    assert_eq!(first.data_source, "opencode_db");
    assert_eq!(first.confidence, lnwdeck_domain::Confidence::High);
    assert_eq!(first.id.len(), 64, "keyed hash fingerprint");

    assert_eq!(
        result.next_cursor.as_deref(),
        Some("1700000100000"),
        "cursor advances to newest session"
    );
}

#[test]
fn collection_is_incremental_from_cursor() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let at_latest = adapter.collect_usage_with_cursor(Some("1700000100000"));
    assert_eq!(at_latest.batch.expect("batch").events.len(), 0);

    let between = adapter.collect_usage_with_cursor(Some("1700000000000"));
    let between_batch = between.batch.expect("batch");
    assert_eq!(between_batch.events.len(), 1);
    assert_eq!(between_batch.events[0].model, "anthropic/claude-sonnet-4");
}

#[test]
fn collection_survives_corrupt_database() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("opencode.db");
    std::fs::write(&db_path, b"garbage-not-sqlite").expect("write corrupt file");
    let adapter = adapter_for(&db_path);

    let result = adapter.collect_usage_with_cursor(None);
    assert!(result.batch.is_none());
    assert!(!result.outcome.error_code.is_empty());
}

#[test]
fn collected_batch_passes_privacy_guard() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let result = adapter.collect_usage_with_cursor(None);
    let batch = result.batch.expect("batch");
    assert!(
        PrivacyGuard::validate_usage_batch(&batch).is_ok(),
        "normalized events must pass the privacy guard"
    );

    let json: serde_json::Value = serde_json::to_value(&batch).expect("serialize");
    let forbidden_keys = [
        "prompt",
        "response",
        "path",
        "file_name",
        "cookie",
        "token",
        "api_key",
    ];
    fn walk(value: &serde_json::Value, forbidden_keys: &[&str]) -> Vec<String> {
        match value {
            serde_json::Value::Object(map) => {
                let mut hits = Vec::new();
                for (key, val) in map {
                    if forbidden_keys.contains(&key.as_str()) {
                        hits.push(key.clone());
                    }
                    hits.extend(walk(val, forbidden_keys));
                }
                hits
            }
            serde_json::Value::Array(items) => items
                .iter()
                .flat_map(|item| walk(item, forbidden_keys))
                .collect(),
            _ => Vec::new(),
        }
    }
    let hits = walk(&json, &forbidden_keys);
    assert!(
        hits.is_empty(),
        "forbidden keys in collected batch: {hits:?}"
    );

    let text = json.to_string();
    for marker in ["C:\\", "/home/", "sk-", "Bearer "] {
        assert!(!text.contains(marker), "forbidden content marker {marker}");
    }
}

#[test]
fn events_serialize_with_only_allowed_keys() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let batch = adapter
        .collect_usage_with_cursor(None)
        .batch
        .expect("batch");
    let json = serde_json::to_value(&batch.events[0]).expect("serialize");
    let keys: Vec<String> = json.as_object().expect("object").keys().cloned().collect();
    let allowed = [
        "id",
        "timestamp",
        "provider_id",
        "model",
        "tokens_input",
        "tokens_output",
        "confidence",
        "data_source",
        "cost",
    ];
    for key in &keys {
        assert!(allowed.contains(&key.as_str()), "unexpected key {key}");
    }
}

#[test]
fn model_json_descriptors_are_reduced_to_model_id() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("opencode.db");
    let conn = Connection::open(&db_path).expect("open");
    conn.execute_batch(
        "CREATE TABLE session (
            id TEXT PRIMARY KEY, project_id TEXT NOT NULL, model TEXT,
            cost REAL NOT NULL DEFAULT 0, tokens_input INTEGER NOT NULL DEFAULT 0,
            tokens_output INTEGER NOT NULL DEFAULT 0, tokens_reasoning INTEGER NOT NULL DEFAULT 0,
            tokens_cache_read INTEGER NOT NULL DEFAULT 0, tokens_cache_write INTEGER NOT NULL DEFAULT 0,
            time_updated INTEGER NOT NULL, time_archived INTEGER
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO session (id, project_id, model, cost, tokens_input, tokens_output,
                              tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated)
         VALUES ('sess_j', 'proj_j', '{\"id\":\"glm-5.2\",\"providerID\":\"opencode-go\",\"variant\":\"default\"}', 0.001, 10, 5, 0, 0, 0, 1700000300000)",
        [],
    )
    .expect("insert json model");
    conn.close().expect("close");

    let adapter = adapter_for(&db_path);
    let batch = adapter
        .collect_usage_with_cursor(None)
        .batch
        .expect("batch");
    assert_eq!(batch.events[0].model, "glm-5.2");
}

#[test]
fn health_reflects_detection() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    assert_eq!(adapter.health_check().status, AdapterHealthStatus::Healthy);

    let adapter = adapter_for(&dir.path().join("missing.db"));
    assert_ne!(adapter.health_check().status, AdapterHealthStatus::Healthy);
}

#[test]
fn quota_estimate_returns_usage_windows_without_fake_limits() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let conn = Connection::open(&db_path).expect("open");
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE session SET time_updated = ?1 WHERE id = 'sess_0001'",
        [now - 3600 * 1000],
    )
    .expect("fresh session 1");
    conn.execute(
        "UPDATE session SET time_updated = ?1 WHERE id = 'sess_0002'",
        [now - 60 * 1000],
    )
    .expect("fresh session 2");
    conn.close().expect("close");
    let adapter = adapter_for(&db_path);

    let report = adapter
        .collect_quota()
        .expect("quota call")
        .expect("report");
    assert_eq!(report.provider_id, "opencode");
    assert_eq!(report.source, "local_estimate");
    assert!(report.is_usable());
    assert_eq!(report.windows.len(), 3);

    let window = report
        .windows
        .iter()
        .find(|w| w.window_key == "5h")
        .expect("5h window");
    assert_eq!(window.used, 775, "sum of input+output+reasoning tokens");
    assert_eq!(window.limit, None, "limit is unknown, never fabricated");
    assert_eq!(window.remaining, None);
    assert_eq!(window.used_percent, None);
    assert_eq!(
        window.remaining_percent, None,
        "an unknown limit must not produce a percentage"
    );
    assert_eq!(window.confidence, lnwdeck_domain::Confidence::Medium);
}

/// Fixture with a `message` table shaped like the real opencode.db
/// (`id, session_id, time_created, time_updated, data` JSON). The JSON rows
/// mirror the real opencode-go message payloads, including absolute paths in
/// `path.cwd` so the quota estimate can be verified not to leak them.
fn create_go_fixture_db(dir: &Path) -> PathBuf {
    let db_path = dir.join("opencode.db");
    let conn = Connection::open(&db_path).expect("open fixture db");
    conn.execute_batch(
        "CREATE TABLE session (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            model TEXT,
            cost REAL NOT NULL DEFAULT 0,
            tokens_input INTEGER NOT NULL DEFAULT 0,
            tokens_output INTEGER NOT NULL DEFAULT 0,
            tokens_reasoning INTEGER NOT NULL DEFAULT 0,
            tokens_cache_read INTEGER NOT NULL DEFAULT 0,
            tokens_cache_write INTEGER NOT NULL DEFAULT 0,
            time_updated INTEGER NOT NULL
        );
        CREATE TABLE message (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            time_created INTEGER NOT NULL,
            time_updated INTEGER NOT NULL,
            data TEXT NOT NULL
        );",
    )
    .expect("create tables");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let go_data = |cost: f64| {
        format!(
            r#"{{"role":"assistant","cost":{cost},"modelID":"glm-5.2","providerID":"opencode-go","time":{{"created":{now_ms},"completed":{now_ms}}},"path":{{"cwd":"C:\\Users\\ABCz\\Desktop\\secret-project","root":"C:\\Users\\ABCz\\Desktop\\secret-project"}}}}"#
        )
    };
    let insert = |id: &str, data: &str| {
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES (?1, 'ses_go', ?2, ?2, ?3)",
            rusqlite::params![id, now_ms, data],
        )
        .expect("insert message");
    };

    insert("msg_1", &go_data(4.0));
    insert("msg_2", &go_data(3.0));
    insert("msg_3", &go_data(2.0));
    insert("msg_4", &go_data(1.0));
    // A cost stored as a string is not a billable number.
    insert(
        "msg_5",
        &format!(
            r#"{{"role":"assistant","cost":"50.00","providerID":"opencode-go","time":{{"created":{now_ms}}}}}"#
        ),
    );
    // User turns are not billed assistant output.
    insert(
        "msg_6",
        &format!(
            r#"{{"role":"user","cost":999.0,"providerID":"opencode-go","time":{{"created":{now_ms}}}}}"#
        ),
    );
    // Other providers are not OpenCode Go turns.
    insert(
        "msg_7",
        &format!(
            r#"{{"role":"assistant","cost":88.0,"providerID":"deepseek","time":{{"created":{now_ms}}}}}"#
        ),
    );
    conn.close().expect("close fixture db");
    db_path
}

fn next_week_end_utc(now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    let days = now.weekday().num_days_from_monday() as i64;
    let monday = (now - chrono::Duration::days(days)).date_naive();
    monday.and_hms_opt(0, 0, 0).expect("midnight").and_utc() + chrono::Duration::days(7)
}

fn next_month_end_utc(now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    let first = now.date_naive().with_day(1).expect("first of month");
    let start = first.and_hms_opt(0, 0, 0).expect("midnight").and_utc();
    if start.month() == 12 {
        start
            .with_year(start.year() + 1)
            .expect("next year")
            .with_month(1)
            .expect("january")
    } else {
        start.with_month(start.month() + 1).expect("next month")
    }
}

#[test]
fn quota_estimate_uses_opencode_go_dollar_caps_from_message_table() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_go_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let report = adapter
        .collect_quota()
        .expect("quota call")
        .expect("report");
    assert_eq!(report.provider_id, "opencode");
    assert_eq!(report.source, "local_estimate");
    assert!(report.is_usable());
    assert_eq!(report.windows.len(), 3);

    let now = chrono::Utc::now();
    let five_h = report
        .windows
        .iter()
        .find(|w| w.window_key == "5h")
        .expect("5h window");
    assert_eq!(five_h.kind, lnwdeck_domain::QuotaKind::Credits);
    assert_eq!(five_h.limit, Some(12_000_000), "$12 cap in micro-dollars");
    assert_eq!(
        five_h.used, 10_000_000,
        "only opencode-go assistant rows with numeric cost are summed"
    );
    assert_eq!(five_h.remaining, Some(2_000_000));
    assert!(
        (five_h.used_percent.expect("percent") - 83.3333).abs() < 0.01,
        "10 of 12 dollars used"
    );
    let expected_reset = now.timestamp_millis() + 5 * 3600 * 1000;
    assert!(
        (five_h.reset_at.expect("reset").timestamp_millis() - expected_reset).abs() < 5_000,
        "5h window resets five hours after the oldest billed turn"
    );
    five_h.check_invariants().expect("consistent window");

    let seven_d = report
        .windows
        .iter()
        .find(|w| w.window_key == "7d")
        .expect("7d window");
    assert_eq!(seven_d.limit, Some(30_000_000), "$30 weekly cap");
    assert_eq!(seven_d.used, 10_000_000);
    assert_eq!(
        seven_d.reset_at.expect("reset"),
        next_week_end_utc(now),
        "weekly window resets at the next Monday boundary (UTC)"
    );
    seven_d.check_invariants().expect("consistent window");

    let thirty_d = report
        .windows
        .iter()
        .find(|w| w.window_key == "30d")
        .expect("30d window");
    assert_eq!(thirty_d.limit, Some(60_000_000), "$60 monthly cap");
    assert_eq!(thirty_d.used, 10_000_000);
    assert_eq!(
        thirty_d.reset_at.expect("reset"),
        next_month_end_utc(now),
        "monthly window resets at the next calendar month (UTC)"
    );
    thirty_d.check_invariants().expect("consistent window");
}

#[test]
fn quota_estimate_falls_back_to_usage_only_windows_without_go_rows() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_go_fixture_db(dir.path());
    let conn = Connection::open(&db_path).expect("open");
    conn.execute(
        "DELETE FROM message WHERE json_extract(data, '$.providerID') = 'opencode-go'",
        [],
    )
    .expect("remove go rows");
    conn.execute(
        "INSERT INTO session (id, project_id, model, cost, tokens_input, tokens_output,
                              tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated)
         VALUES ('sess_go', 'proj_go', 'glm-5.2', 0.1, 200, 100, 0, 0, 0, 1700000000000)",
        [],
    )
    .expect("insert session");
    conn.close().expect("close");

    let adapter = adapter_for(&db_path);
    let report = adapter
        .collect_quota()
        .expect("quota call")
        .expect("report");
    assert_eq!(report.windows.len(), 3);
    for window in &report.windows {
        assert_eq!(
            window.limit, None,
            "without opencode-go rows the limit stays unknown"
        );
        assert_eq!(window.remaining, None);
        assert_eq!(window.used_percent, None);
        window.check_invariants().expect("consistent window");
    }
}

#[test]
fn opencode_go_quota_report_leaks_no_paths_or_other_provider_costs() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_go_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let report = adapter
        .collect_quota()
        .expect("quota call")
        .expect("report");
    let json = serde_json::to_string(&report).expect("serialize");
    assert!(
        !json.contains("secret-project"),
        "absolute paths in message JSON must not reach the report"
    );
    assert!(!json.contains("C:\\\\"));
    assert!(
        !json.contains("deepseek"),
        "other providers' rows must not leak"
    );
    assert!(
        !json.contains("50.00"),
        "string-typed costs are excluded, not serialized"
    );
    // The 999.0 user turn and 88.0 other-provider turn must not move the
    // aggregated totals; the cap test asserts the exact 10_000_000 sum.
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    for window in value["windows"].as_array().expect("windows") {
        assert!(window["used"].as_u64().expect("used") <= 10_000_000);
    }
}

#[test]
fn quota_estimate_is_none_when_source_missing() {
    let dir = tempdir().expect("temp dir");
    let adapter = adapter_for(&dir.path().join("missing.db"));
    assert!(
        adapter.collect_quota().expect("quota call").is_none(),
        "no local source means no estimate"
    );
}

#[test]
fn requires_read_permission_on_local_store() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    assert!(adapter
        .required_permissions()
        .contains(&lnwdeck_provider_runtime::Permission::FileSystem));
}
