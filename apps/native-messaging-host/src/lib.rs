pub mod protocol;

use protocol::QuotaMessage;

/// Where the desktop app keeps its database.
fn db_path() -> std::path::PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::var("APPDATA").map(std::path::PathBuf::from))
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("lnwdeck")
        .join("lnwdeck.db")
}

/// Handles one message from the browser extension. Quota detections are
/// recorded into the lnwdeck event log so they are visible on the System page;
/// everything else is acknowledged. The desktop app is never required to be
/// running for the host to answer.
pub fn process_message(msg: &QuotaMessage) -> QuotaMessage {
    if msg.msg_type == "quota_update" {
        let detail = msg
            .payload
            .as_ref()
            .map(|payload| payload.to_string())
            .unwrap_or_default();
        record_browser_quota(&detail);
    }
    QuotaMessage {
        msg_type: "ack".to_string(),
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        nonce: msg.nonce.clone(),
        payload: Some(serde_json::json!({"status": "received"})),
    }
}

/// Writes a sanitized browser-quota event (provider + remaining only). The
/// event log lives in the same SQLite database the desktop app uses.
fn record_browser_quota(detail: &str) {
    use lnwdeck_storage::repositories::{AppEventLevel, AppEventRepository};
    let Ok(conn) = rusqlite::Connection::open(db_path()) else {
        return;
    };
    let _ = AppEventRepository::new(&conn).record(
        "browser_helper",
        AppEventLevel::Info,
        "BROWSER_QUOTA_DETECTED",
        detail,
    );
}
