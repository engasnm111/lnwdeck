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

fn timestamp_from_number(value: f64) -> Option<chrono::DateTime<chrono::Utc>> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    if value >= 10_000_000_000.0 {
        chrono::DateTime::from_timestamp_millis(value.round() as i64)
    } else {
        chrono::DateTime::from_timestamp(value.round() as i64, 0)
    }
}

fn timestamp_from_value(value: &serde_json::Value) -> Option<chrono::DateTime<chrono::Utc>> {
    match value {
        serde_json::Value::Number(number) => timestamp_from_number(number.as_f64()?),
        serde_json::Value::String(text) => chrono::DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|parsed| parsed.with_timezone(&chrono::Utc))
            .or_else(|| {
                text.trim()
                    .parse::<f64>()
                    .ok()
                    .and_then(timestamp_from_number)
            }),
        _ => None,
    }
}

fn absolute_reset_at(value: &serde_json::Value) -> Option<chrono::DateTime<chrono::Utc>> {
    ["reset_at", "resets_at", "resetAt", "resetsAt"]
        .iter()
        .find_map(|key| value.get(*key).and_then(timestamp_from_value))
}

fn value_number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite())
}

fn window_from(value: &serde_json::Value) -> Option<QuotaWindow> {
    let used_percent = value_number(value.get("used_percent"))?;
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
        .or_else(|| absolute_reset_at(value));
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

fn credit_window(value: &serde_json::Value) -> Option<QuotaWindow> {
    let limit = value_number(value.get("limit"));
    let used = value_number(value.get("used"));
    let mut percent = value_number(value.get("used_percent"));
    if let (Some(limit), Some(used)) = (limit, used) {
        if limit > 0.0 && (percent.is_none() || percent == Some(0.0) && used > 0.0) {
            percent = Some(used / limit * 100.0);
        }
    }
    let percent = percent?.clamp(0.0, 100.0);
    let reset = value.get("reset_at").and_then(timestamp_from_value);
    Some(QuotaWindow::from_percent(
        "credits",
        "Credits",
        QuotaWindowScope::Other,
        QuotaKind::Credits,
        percent,
        reset,
        Confidence::High,
    ))
}

fn is_spark_limit(value: &serde_json::Value) -> bool {
    ["limit_name", "metered_feature"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .any(|name| name.to_ascii_lowercase().contains("spark"))
}

fn spark_window(value: &serde_json::Value) -> Option<QuotaWindow> {
    let mut window = window_from(value)?;
    match window.window_key.as_str() {
        "session" => {
            window.window_key = "spark_session".to_string();
            window.label = "Spark Session".to_string();
        }
        "weekly" => {
            window.window_key = "spark_weekly".to_string();
            window.label = "Spark Weekly".to_string();
        }
        _ => {
            window.window_key = "spark".to_string();
            window.label = "Spark".to_string();
        }
    }
    Some(window)
}

/// Converts the `/wham/usage` payload into quota windows, including the
/// current spend-control credit bucket and Spark-specific rate limits.
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
    if let Some(window) = body
        .pointer("/spend_control/individual_limit")
        .and_then(credit_window)
    {
        windows.push(window);
    }
    if let Some(additional) = body
        .get("additional_rate_limits")
        .and_then(serde_json::Value::as_array)
    {
        for entry in additional.iter().filter(|entry| is_spark_limit(entry)) {
            let Some(rate_limit) = entry.get("rate_limit") else {
                continue;
            };
            for slot in ["primary_window", "secondary_window"] {
                let Some(window) = rate_limit.get(slot).and_then(spark_window) else {
                    continue;
                };
                if !windows
                    .iter()
                    .any(|existing| existing.window_key == window.window_key)
                {
                    windows.push(window);
                }
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
    fn current_payload_includes_credit_and_spark_windows() {
        let body = serde_json::json!({
            "rate_limit": {
                "primary_window": {"used_percent": 10, "limit_window_seconds": 18000}
            },
            "spend_control": {
                "individual_limit": {"limit": 200, "used": 50, "used_percent": 0}
            },
            "additional_rate_limits": [{
                "limit_name": "codex_spark",
                "rate_limit": {
                    "primary_window": {"used_percent": 30, "limit_window_seconds": 18000},
                    "secondary_window": {"used_percent": 40, "limit_window_seconds": 604800}
                }
            }]
        });
        let windows = windows_from_payload(&body);
        assert!(windows
            .iter()
            .any(|window| window.window_key == "credits" && window.used_percent == Some(25.0)));
        assert!(windows.iter().any(
            |window| window.window_key == "spark_session" && window.used_percent == Some(30.0)
        ));
        assert!(
            windows
                .iter()
                .any(|window| window.window_key == "spark_weekly"
                    && window.used_percent == Some(40.0))
        );
    }

    #[test]
    fn numeric_reset_at_is_preserved() {
        let body = serde_json::json!({
            "rate_limit": {
                "primary_window": {
                    "used_percent": 46,
                    "limit_window_seconds": 604800,
                    "reset_at": 1_800_000_000
                }
            }
        });

        let windows = windows_from_payload(&body);

        assert_eq!(
            windows[0].reset_at.map(|value| value.timestamp()),
            Some(1_800_000_000)
        );
    }

    #[test]
    fn numeric_reset_at_accepts_seconds_or_milliseconds() {
        let body = serde_json::json!({
            "rate_limit": {
                "primary_window": {
                    "used_percent": 46,
                    "limit_window_seconds": 18000,
                    "reset_at": 1_800_000_000
                },
                "secondary_window": {
                    "used_percent": 17,
                    "limit_window_seconds": 604800,
                    "reset_at": 1_800_000_000_000i64
                }
            }
        });

        let windows = windows_from_payload(&body);

        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows[0].reset_at.map(|value| value.timestamp()),
            Some(1_800_000_000)
        );
        assert_eq!(
            windows[1].reset_at.map(|value| value.timestamp()),
            Some(1_800_000_000)
        );
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
