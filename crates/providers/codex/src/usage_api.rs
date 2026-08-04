//! Codex subscription quota, as published by OpenAI.
//!
//! The Codex CLI stores an OAuth token in `~/.codex/auth.json`; the same token
//! authorises `GET /backend-api/wham/usage`, which returns the used percentage
//! and reset time of each rate-limit window. Windows are classified by their
//! declared duration rather than by position, because a free-tier account
//! receives only a weekly window and it arrives in the primary slot.

use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};
use lnwdeck_provider_http::{get_json, JsonRequest};
use std::path::Path;
use std::time::Duration;

const USAGE_ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";
/// 5 hours in seconds: the session window.
const SESSION_WINDOW_SECONDS: i64 = 18_000;
/// 7 days in seconds: the weekly window.
const WEEKLY_WINDOW_SECONDS: i64 = 604_800;

/// OAuth material the Codex CLI stores locally.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexAuth {
    pub access_token: String,
    /// Sent as `ChatGPT-Account-Id` when present; some plan tiers require it.
    pub account_id: Option<String>,
}

/// Reads the Codex OAuth token. Only the token and account id are read.
pub fn read_auth(auth_path: &Path) -> Option<CodexAuth> {
    let raw = std::fs::read_to_string(auth_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let tokens = value.get("tokens")?;
    let access_token = tokens
        .get("access_token")
        .and_then(|token| token.as_str())?
        .trim()
        .to_string();
    if access_token.is_empty() {
        return None;
    }
    let account_id = tokens
        .get("account_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Some(CodexAuth {
        access_token,
        account_id,
    })
}

/// Window duration classification. Returns the label and scope to use.
fn classify(seconds: Option<i64>) -> (&'static str, &'static str, QuotaWindowScope) {
    match seconds {
        Some(SESSION_WINDOW_SECONDS) => ("session", "Session", QuotaWindowScope::Rolling),
        Some(WEEKLY_WINDOW_SECONDS) => ("weekly", "Weekly", QuotaWindowScope::Weekly),
        _ => ("window", "Rate limit", QuotaWindowScope::Other),
    }
}

fn window_from(value: &serde_json::Value) -> Option<QuotaWindow> {
    let used_percent = value.get("used_percent").and_then(|value| value.as_f64())?;
    let seconds = value
        .get("limit_window_seconds")
        .and_then(|value| value.as_i64());
    let (key, label, scope) = classify(seconds);
    let reset_at = value
        .get("resets_in_seconds")
        .and_then(|value| value.as_i64())
        .and_then(|seconds| {
            chrono::Utc::now().checked_add_signed(chrono::Duration::seconds(seconds))
        })
        .or_else(|| {
            value
                .get("resets_at")
                .and_then(|value| value.as_str())
                .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
                .map(|parsed| parsed.with_timezone(&chrono::Utc))
        });
    Some(QuotaWindow::from_percent(
        key,
        label,
        scope,
        QuotaKind::Requests,
        used_percent,
        reset_at,
        Confidence::High,
    ))
}

/// Converts the `/wham/usage` payload into quota windows.
pub fn windows_from_payload(body: &serde_json::Value) -> Vec<QuotaWindow> {
    let Some(rate_limit) = body.get("rate_limit") else {
        return Vec::new();
    };
    let mut windows = Vec::new();
    for slot in ["primary_window", "secondary_window"] {
        if let Some(window) = rate_limit.get(slot).and_then(window_from) {
            // Two slots can report the same duration; keep the first.
            if !windows
                .iter()
                .any(|existing: &QuotaWindow| existing.window_key == window.window_key)
            {
                windows.push(window);
            }
        }
    }
    windows
}

/// Fetches the published windows.
///
/// `Ok(None)` means no token is stored, so nothing was requested.
pub fn fetch_windows(
    auth_path: &Path,
    endpoint: &str,
    timeout: Duration,
) -> Result<Option<Vec<QuotaWindow>>, String> {
    let Some(auth) = read_auth(auth_path) else {
        return Ok(None);
    };
    let account_header = auth.account_id.clone().unwrap_or_default();
    let headers: Vec<(&str, &str)> = if account_header.is_empty() {
        Vec::new()
    } else {
        vec![("chatgpt-account-id", account_header.as_str())]
    };
    let response = get_json(
        JsonRequest {
            timeout,
            ..JsonRequest::new(endpoint)
        }
        .bearer(&auth.access_token)
        .with_headers(&headers),
    )?;
    let windows = windows_from_payload(&response.body);
    if windows.is_empty() {
        return Err("QUOTA_NOT_PUBLISHED".to_string());
    }
    Ok(Some(windows))
}

/// Default endpoint, overridable for verification against a local server.
pub fn default_endpoint() -> String {
    std::env::var("LNWDECK_CODEX_USAGE_ENDPOINT").unwrap_or_else(|_| USAGE_ENDPOINT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reads_the_token_and_optional_account_id() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("auth.json");
        std::fs::write(
            &path,
            r#"{"tokens":{"access_token":"eyJ-example","account_id":"acct_1"}}"#,
        )
        .expect("write auth");
        let auth = read_auth(&path).expect("auth");
        assert_eq!(auth.access_token, "eyJ-example");
        assert_eq!(auth.account_id.as_deref(), Some("acct_1"));

        let without = dir.path().join("no-account.json");
        std::fs::write(&without, r#"{"tokens":{"access_token":"t"}}"#).expect("write");
        assert_eq!(read_auth(&without).expect("auth").account_id, None);
    }

    #[test]
    fn an_unusable_auth_file_yields_no_token() {
        let dir = tempdir().expect("temp dir");
        assert_eq!(read_auth(&dir.path().join("absent.json")), None);
        let blank = dir.path().join("blank.json");
        std::fs::write(&blank, r#"{"tokens":{"access_token":""}}"#).expect("write");
        assert_eq!(read_auth(&blank), None);
    }

    #[test]
    fn windows_are_classified_by_declared_duration_not_by_slot() {
        let body = serde_json::json!({
            "rate_limit": {
                // A free-tier account reports its weekly window in the primary slot.
                "primary_window": {
                    "used_percent": 59,
                    "limit_window_seconds": 604800,
                    "resets_in_seconds": 3600
                },
                "secondary_window": {
                    "used_percent": 12,
                    "limit_window_seconds": 18000,
                    "resets_in_seconds": 600
                }
            }
        });
        let windows = windows_from_payload(&body);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "Weekly");
        assert_eq!(windows[0].used_percent, Some(59.0));
        assert_eq!(windows[0].remaining_percent, Some(41.0));
        assert!(windows[0].reset_at.is_some());
        assert_eq!(windows[1].label, "Session");
        assert_eq!(windows[1].remaining_percent, Some(88.0));
        for window in &windows {
            window.check_invariants().expect("consistent");
        }
    }

    #[test]
    fn an_unknown_duration_is_kept_rather_than_dropped() {
        let body = serde_json::json!({
            "rate_limit": {
                "primary_window": { "used_percent": 5, "limit_window_seconds": 999 }
            }
        });
        let windows = windows_from_payload(&body);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "Rate limit");
        assert_eq!(windows[0].reset_at, None);
    }

    #[test]
    fn a_payload_without_rate_limits_produces_nothing() {
        assert!(windows_from_payload(&serde_json::json!({})).is_empty());
        assert!(windows_from_payload(&serde_json::json!({ "rate_limit": {} })).is_empty());
        assert!(windows_from_payload(
            &serde_json::json!({ "rate_limit": { "primary_window": { "used_percent": null } } })
        )
        .is_empty());
    }

    #[test]
    fn no_stored_token_means_no_request_and_no_error() {
        let dir = tempdir().expect("temp dir");
        assert_eq!(
            fetch_windows(
                &dir.path().join("absent.json"),
                "https://127.0.0.1:1/usage",
                Duration::from_millis(200)
            ),
            Ok(None)
        );
    }
}
