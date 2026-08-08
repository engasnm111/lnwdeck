use lnwdeck_domain::UsageEvent;
use serde_json::Value;

const FORBIDDEN_FIELDS: &[&str] = &["prompt", "response", "path", "file_name", "cookie", "token"];

const ALLOWED_FIELDS: &[&str] = &[
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

fn json_value(event: &UsageEvent) -> Value {
    let json_str = serde_json::to_string(event).expect("serialization must succeed");
    serde_json::from_str(&json_str).expect("deserialization must succeed")
}

fn collect_keys(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            keys
        }
        _ => vec![],
    }
}

#[test]
fn serialized_usage_event_has_only_allowed_fields() {
    let event = UsageEvent::new_sample();
    let value = json_value(&event);
    let keys = collect_keys(&value);

    for key in &keys {
        assert!(
            ALLOWED_FIELDS.contains(&key.as_str()),
            "unexpected field found: {key}"
        );
    }

    for expected in ALLOWED_FIELDS {
        assert!(
            keys.contains(&expected.to_string()),
            "required field missing: {expected}"
        );
    }
}

#[test]
fn serialized_usage_event_has_no_forbidden_fields() {
    let event = UsageEvent::new_sample();
    let json_str = serde_json::to_string(&event).expect("serialization must succeed");
    let value: Value = serde_json::from_str(&json_str).expect("deserialization must succeed");

    let keys = collect_keys(&value);

    for forbidden in FORBIDDEN_FIELDS {
        assert!(
            !keys.contains(&forbidden.to_string()),
            "forbidden field found in serialized output: {forbidden}"
        );
    }
}

#[test]
fn json_schema_includes_allowed_keys_only() {
    let schema = schemars::schema_for!(UsageEvent);
    let schema_json = serde_json::to_value(&schema).expect("schema serialization must succeed");

    let properties = schema_json["properties"]
        .as_object()
        .expect("schema must have properties");

    let schema_keys: Vec<String> = properties.keys().cloned().collect();

    for key in &schema_keys {
        assert!(
            ALLOWED_FIELDS.contains(&key.as_str()),
            "unexpected field in JSON Schema: {key}"
        );
    }

    for forbidden in FORBIDDEN_FIELDS {
        assert!(
            !schema_keys.contains(&forbidden.to_string()),
            "forbidden field in JSON Schema: {forbidden}"
        );
    }
}
