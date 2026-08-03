pub mod protocol;

use protocol::QuotaMessage;

pub fn process_message(msg: &QuotaMessage) -> QuotaMessage {
    QuotaMessage {
        msg_type: "ack".to_string(),
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        nonce: msg.nonce.clone(),
        payload: Some(serde_json::json!({"status": "received"})),
    }
}
