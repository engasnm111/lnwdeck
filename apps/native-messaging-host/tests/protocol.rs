use inwdeck_native_messaging_host::protocol::{read_message, write_message, QuotaMessage};
use std::io::Cursor;

fn valid_message() -> QuotaMessage {
    QuotaMessage {
        msg_type: "quota_update".to_string(),
        version: 1,
        timestamp: "2025-01-01T00:00:00Z".to_string(),
        nonce: "abcdef123456".to_string(),
        payload: Some(serde_json::json!({"provider": "openai", "remaining": 500})),
    }
}

fn encode_message(msg: &QuotaMessage) -> Vec<u8> {
    let json = serde_json::to_vec(msg).unwrap();
    let len = json.len() as u32;
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&json);
    buf
}

#[test]
fn read_valid_length_prefixed_message() {
    let msg = valid_message();
    let encoded = encode_message(&msg);

    let mut cursor = Cursor::new(encoded);
    let decoded = read_message(&mut cursor).unwrap();

    assert_eq!(decoded.msg_type, "quota_update");
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.nonce, "abcdef123456");
}

#[test]
fn write_message_produces_correct_prefix() {
    let msg = valid_message();
    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();

    let json = serde_json::to_vec(&msg).unwrap();
    let len = json.len() as u32;

    assert_eq!(&buf[0..4], &len.to_le_bytes());
    assert_eq!(&buf[4..], &json);
}

#[test]
fn unknown_message_type_is_still_accepted() {
    let raw = r#"{"type":"unknown_action","version":1,"timestamp":"2025-01-01T00:00:00Z","nonce":"abc12345"}"#;
    let msg: QuotaMessage = serde_json::from_str(raw).unwrap();
    assert_eq!(msg.msg_type, "unknown_action");
}

#[test]
fn missing_required_fields_fails() {
    let raw = r#"{"type":"quota_update","version":1}"#;
    let result: Result<QuotaMessage, _> = serde_json::from_str(raw);
    assert!(result.is_err(), "missing required fields must fail");
}

#[test]
fn oversized_message_rejected() {
    // A message exceeding 1MB should be rejected
    let large_string = "a".repeat(1_048_577); // 1MB + 1 byte
    let msg = serde_json::json!({
        "type": "quota_update",
        "version": 1,
        "timestamp": "2025-01-01T00:00:00Z",
        "nonce": "abc12345",
        "payload": {"data": large_string}
    });
    let json = serde_json::to_vec(&msg).unwrap();
    assert!(
        json.len() > 1_048_576,
        "constructed oversized message must exceed 1MB"
    );

    let mut buf = Vec::new();
    let len = json.len() as u32;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&json);

    let mut cursor = Cursor::new(&buf);
    let result = read_message(&mut cursor);
    assert!(result.is_err(), "oversized message must be rejected");
}

#[test]
fn empty_nonce_fails_validation() {
    let raw =
        r#"{"type":"quota_update","version":1,"timestamp":"2025-01-01T00:00:00Z","nonce":""}"#;
    let msg: QuotaMessage = serde_json::from_str(raw).unwrap();
    assert!(msg.nonce.is_empty());
}

#[test]
fn payload_is_optional() {
    let raw = r#"{"type":"health_check","version":1,"timestamp":"2025-01-01T00:00:00Z","nonce":"abc12345"}"#;
    let msg: QuotaMessage = serde_json::from_str(raw).unwrap();
    assert_eq!(msg.payload, None);
}
