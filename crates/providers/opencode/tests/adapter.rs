use lnwdeck_provider_opencode::{windows_from_dashboard_html, OpenCodeAdapter};
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
    assert_eq!(result.detection_method, "local_sqlite+credential");
    assert!(matches!(
        result.permission_state.as_str(),
        "credential_required" | "read_ok+credential_stored"
    ));
    if result.permission_state == "credential_required" {
        assert_eq!(result.detection_error_code, "NOT_CONFIGURED");
    } else {
        assert!(result.detection_error_code.is_empty());
    }
    assert!(result.last_detection_at.is_some());
}

#[test]
fn detection_negative_when_database_missing() {
    let dir = tempdir().expect("temp dir");
    let missing = dir.path().join("no-such-dir").join("opencode.db");
    let adapter = adapter_for(&missing);

    let result = adapter.detect().expect("detect");
    if result.detected {
        assert_eq!(result.permission_state, "credential_stored");
        assert_eq!(result.detection_method, "credential");
        assert_eq!(result.source_type, "remote_api");
        assert!(result.source_exists);
    } else {
        assert!(!result.source_exists);
        assert_eq!(result.permission_state, "credential_required");
        assert_eq!(result.detection_error_code, "NOT_CONFIGURED");
        assert_eq!(
            adapter.health_check().status,
            AdapterHealthStatus::NotConfigured
        );
    }
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
    assert_eq!(first.tokens_cached, 10);
    assert_eq!(first.tokens_cache_write, 3);
    assert_eq!(first.tokens_output, 50);
    assert_eq!(first.tokens_reasoning, 5);
    assert_eq!(first.cost, "0.001200");
    assert_eq!(first.provider_id, "opencode");
    assert_eq!(first.data_source, "opencode_db");
    assert_eq!(first.confidence, lnwdeck_domain::Confidence::High);
    assert_eq!(first.id.len(), 64, "keyed hash fingerprint");

    let session_hash = first
        .session_hash
        .as_deref()
        .expect("session hash must be populated");
    let project_hash = first
        .project_hash
        .as_deref()
        .expect("project hash must be populated");
    assert_eq!(session_hash.len(), 64, "session id is a keyed hash");
    assert_eq!(project_hash.len(), 64, "project id is a keyed hash");
    let serialized = serde_json::to_string(&batch).expect("serialize");
    assert!(
        !serialized.contains("sess_0001") && !serialized.contains("proj_0001"),
        "raw session and project ids must never appear in normalized data"
    );

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
        "tokens_cached",
        "tokens_cache_write",
        "tokens_output",
        "tokens_reasoning",
        "confidence",
        "data_source",
        "cost",
        "session_hash",
        "project_hash",
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

    assert!(matches!(
        adapter.health_check().status,
        AdapterHealthStatus::Healthy
            | AdapterHealthStatus::Degraded
            | AdapterHealthStatus::NotConfigured
    ));

    let adapter = adapter_for(&dir.path().join("missing.db"));
    assert!(matches!(
        adapter.health_check().status,
        AdapterHealthStatus::Healthy
            | AdapterHealthStatus::Degraded
            | AdapterHealthStatus::NotConfigured
    ));
}

#[test]
fn dashboard_quota_uses_authoritative_percentages_and_reset_times() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-08-09T10:00:00Z")
        .expect("fixed timestamp")
        .with_timezone(&chrono::Utc);
    let html = r#"
        <script>
          {"rollingUsage":{"resetInSec":3600,"usagePercent":12.5},
           "weeklyUsage":{"usagePercent":48,"resetInSec":7200},
           "monthlyUsage":{"usagePercent":0.25,"resetInSec":86400}}
        </script>
    "#;

    let windows = windows_from_dashboard_html(html, now).expect("dashboard payload");
    assert_eq!(windows.len(), 3);

    let rolling = windows
        .iter()
        .find(|window| window.window_key == "5h")
        .expect("rolling window");
    assert_eq!(rolling.used_percent, Some(12.5));
    assert_eq!(
        rolling.reset_at,
        Some(now + chrono::Duration::seconds(3600))
    );
    assert_eq!(
        rolling.limit, None,
        "the provider reports percent, not dollars"
    );

    let weekly = windows
        .iter()
        .find(|window| window.window_key == "7d")
        .expect("weekly window");
    assert_eq!(weekly.used_percent, Some(48.0));
    assert_eq!(weekly.reset_at, Some(now + chrono::Duration::seconds(7200)));

    let monthly = windows
        .iter()
        .find(|window| window.window_key == "30d")
        .expect("monthly window");
    assert_eq!(monthly.used_percent, Some(25.0));
    assert_eq!(
        monthly.reset_at,
        Some(now + chrono::Duration::seconds(86400))
    );
}

#[test]
fn dashboard_parser_handles_nested_objects_and_string_numbers() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-08-09T10:00:00Z")
        .expect("fixed timestamp")
        .with_timezone(&chrono::Utc);
    let html = r#"
        <script>
          {"rollingUsage":{"metadata":{"label":"Go"},"resetInSec":"3600","usagePercent":"12.5"},
           "weeklyUsage":{"usagePercent":48,"resetInSec":7200},
           "monthlyUsage":{"usagePercent":0.25,"resetInSec":86400}}
        </script>
    "#;

    let windows = windows_from_dashboard_html(html, now).expect("dashboard payload");
    assert_eq!(windows.len(), 3);
    assert_eq!(
        windows
            .iter()
            .find(|window| window.window_key == "5h")
            .expect("rolling window")
            .used_percent,
        Some(12.5)
    );
}

#[test]
fn malformed_dashboard_quota_cannot_turn_into_a_full_percentage() {
    let html = r#"<script>{"rollingUsage":{"usagePercent":"100"}}</script>"#;
    let error = windows_from_dashboard_html(
        html,
        chrono::DateTime::parse_from_rfc3339("2026-08-09T10:00:00Z")
            .expect("fixed timestamp")
            .with_timezone(&chrono::Utc),
    )
    .expect_err("invalid provider data must be rejected");
    assert_eq!(error, "SOURCE_SCHEMA_MISMATCH");
}

#[test]
fn requires_read_permission_on_local_store() {
    let dir = tempdir().expect("temp dir");
    let db_path = create_fixture_db(dir.path());
    let adapter = adapter_for(&db_path);

    let permissions = adapter.required_permissions();
    assert!(permissions.contains(&lnwdeck_provider_runtime::Permission::FileSystem));
    assert!(permissions.contains(&lnwdeck_provider_runtime::Permission::Network));
    assert!(permissions.contains(&lnwdeck_provider_runtime::Permission::Credential));

    let descriptor = adapter.descriptor();
    assert_eq!(
        descriptor.quota_support,
        lnwdeck_provider_runtime::ChannelSupport::Native
    );
    assert_eq!(
        descriptor.auth,
        lnwdeck_provider_runtime::AuthKind::BrowserCookie
    );
}
