use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use lnwdeck_security::{IdentifierHasher, PrivacyGuard, Redactor};
use serde_json::Value;

fn sample_batch() -> UsageBatch {
    UsageBatch {
        batch_id: "batch_001".to_string(),
        events: vec![UsageEvent {
            id: "evt_001".to_string(),
            timestamp: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            provider_id: "prov_openai".to_string(),
            model: "gpt-4o".to_string(),
            tokens_input: 100,
            tokens_output: 50,
            confidence: Confidence::High,
            data_source: "web".to_string(),
            cost: "0.0015".to_string(),
            session_hash: None,
            project_hash: None,
        }],
    }
}

fn batch_with_extra_field(extra_key: &str, extra_value: &str) -> Value {
    let mut json = serde_json::to_value(sample_batch()).unwrap();
    if let Value::Object(ref mut map) = json {
        map.insert(
            extra_key.to_string(),
            Value::String(extra_value.to_string()),
        );
    }
    json
}

#[test]
fn rejects_windows_path_in_extra_field() {
    let json = batch_with_extra_field("file_path", "C:\\Users\\test\\secret.txt");
    assert!(PrivacyGuard::validate_raw_json(&json).is_err());
}

#[test]
fn rejects_unix_path_in_extra_field() {
    let json = batch_with_extra_field("file_path", "/home/user/secret.txt");
    assert!(PrivacyGuard::validate_raw_json(&json).is_err());
}

#[test]
fn rejects_bearer_token() {
    let json = batch_with_extra_field("auth", "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...");
    assert!(PrivacyGuard::validate_raw_json(&json).is_err());
}

#[test]
fn rejects_api_key_pattern() {
    let json = batch_with_extra_field("api_key", "sk-proj-abc123def456ghijklmnopqrstuvwxyz");
    assert!(PrivacyGuard::validate_raw_json(&json).is_err());
}

#[test]
fn accepts_valid_batch() {
    let batch = sample_batch();
    assert!(PrivacyGuard::validate_usage_batch(&batch).is_ok());
}

#[test]
fn redactor_strips_bearer_token() {
    let input = "Authorization: Bearer sk-abc123";
    let sanitized = Redactor::sanitize(input);
    assert!(!sanitized.contains("Bearer"));
    assert!(!sanitized.contains("sk-abc123"));
}

#[test]
fn redactor_strips_paths() {
    let input = "Loaded from C:\\Users\\test\\config.json";
    let sanitized = Redactor::sanitize(input);
    assert!(!sanitized.contains("C:\\Users"));
}

#[test]
fn redactor_preserves_safe_content() {
    let input = "Provider: openai, model: gpt-4o, tokens: 150";
    let sanitized = Redactor::sanitize(input);
    assert!(sanitized.contains("openai"));
    assert!(sanitized.contains("gpt-4o"));
    assert!(sanitized.contains("150"));
}

#[test]
fn identifier_hasher_produces_consistent_output() {
    let h1 = IdentifierHasher::new(b"test-key");
    let h2 = IdentifierHasher::new(b"test-key");
    let hash1 = h1.hash(b"session-123");
    let hash2 = h2.hash(b"session-123");
    assert_eq!(hash1, hash2);
}

#[test]
fn identifier_hasher_produces_different_output_for_different_keys() {
    let h1 = IdentifierHasher::new(b"key-a");
    let h2 = IdentifierHasher::new(b"key-b");
    let hash1 = h1.hash(b"session-123");
    let hash2 = h2.hash(b"session-123");
    assert_ne!(hash1, hash2);
}

#[test]
fn identifier_hasher_produces_different_output_for_different_inputs() {
    let h = IdentifierHasher::new(b"test-key");
    let hash1 = h.hash(b"session-123");
    let hash2 = h.hash(b"session-456");
    assert_ne!(hash1, hash2);
}

#[test]
fn privacy_violation_error_does_not_expose_sensitive_value() {
    let json = batch_with_extra_field("secret", "C:\\Users\\victim\\passwords.txt");
    let err = PrivacyGuard::validate_raw_json(&json).unwrap_err();
    let msg = format!("{err:?}");
    assert!(!msg.contains("C:\\Users"));
}
