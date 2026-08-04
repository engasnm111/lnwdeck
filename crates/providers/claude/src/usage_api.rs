//! Claude subscription quota, as published by Anthropic.
//!
//! Claude Code stores an OAuth token locally; the same token authorises
//! `GET /api/oauth/usage`, which returns the utilization percentage and reset
//! time of each rate-limit window. That percentage is the only quota figure
//! Anthropic publishes, so it is carried through as a percentage and no token
//! count is invented.
//!
//! The request shape (endpoint, `anthropic-beta` header, window fields) matches
//! what the Claude Code CLI itself uses.

use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};
use lnwdeck_provider_http::{get_json, JsonRequest};
use std::path::Path;
use std::time::Duration;

const USAGE_ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA_HEADER: (&str, &str) = ("anthropic-beta", "oauth-2025-04-20");

/// Windows Credential Manager target used by newer Claude Code builds.
const CREDENTIAL_TARGET: &str = "Claude Code-credentials";

/// Extracts the OAuth access token from a Claude Code credentials payload.
pub fn token_from_payload(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let token = value
        .get("claudeAiOauth")
        .and_then(|oauth| oauth.get("accessToken"))
        .and_then(|token| token.as_str())?
        .trim()
        .to_string();
    (!token.is_empty()).then_some(token)
}

/// Reads the OAuth access token Claude Code stored for this user.
///
/// Older builds keep it in `~/.claude/.credentials.json`; newer Windows builds
/// keep it in the Credential Manager. Both are checked, the file first. Only the
/// token is read: it is passed straight to Anthropic and never persisted,
/// logged or exported.
pub fn read_access_token(credentials_path: &Path) -> Option<String> {
    if let Ok(raw) = std::fs::read_to_string(credentials_path) {
        if let Some(token) = token_from_payload(&raw) {
            return Some(token);
        }
    }
    lnwdeck_windows_integration::CredentialStore::get_by_target(CREDENTIAL_TARGET)
        .ok()
        .and_then(|raw| token_from_payload(&raw))
}

/// One published window, before it becomes a `QuotaWindow`.
#[derive(Debug, Clone, PartialEq)]
struct PublishedWindow {
    key: &'static str,
    label: String,
    scope: QuotaWindowScope,
    utilization: f64,
    resets_at: Option<String>,
}

fn parse_reset(value: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    let text = value?;
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|parsed| parsed.with_timezone(&chrono::Utc))
}

fn published(
    body: &serde_json::Value,
    field: &str,
    key: &'static str,
    label: &str,
    scope: QuotaWindowScope,
) -> Option<PublishedWindow> {
    let window = body.get(field)?;
    let utilization = window
        .get("utilization")
        .and_then(|value| value.as_f64())
        .or_else(|| window.get("percent").and_then(|value| value.as_f64()))?;
    Some(PublishedWindow {
        key,
        label: label.to_string(),
        scope,
        utilization,
        resets_at: window
            .get("resets_at")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

/// Converts the `/api/oauth/usage` payload into quota windows.
///
/// Returns an empty vector when the account has no window at all, so the caller
/// reports "no quota published" instead of an empty success.
pub fn windows_from_payload(body: &serde_json::Value) -> Vec<QuotaWindow> {
    let mut published_windows = Vec::new();
    if let Some(window) = published(
        body,
        "five_hour",
        "five_hour",
        "Session",
        QuotaWindowScope::Rolling,
    ) {
        published_windows.push(window);
    }
    if let Some(window) = published(
        body,
        "seven_day",
        "seven_day",
        "Weekly",
        QuotaWindowScope::Weekly,
    ) {
        published_windows.push(window);
    }
    if let Some(window) = published(
        body,
        "seven_day_opus",
        "seven_day_opus",
        "Weekly Opus",
        QuotaWindowScope::Weekly,
    ) {
        published_windows.push(window);
    }

    let mut windows: Vec<QuotaWindow> = published_windows
        .into_iter()
        .map(|window| {
            QuotaWindow::from_percent(
                window.key,
                window.label,
                window.scope,
                QuotaKind::Requests,
                window.utilization,
                parse_reset(window.resets_at.as_deref()),
                Confidence::High,
            )
        })
        .collect();

    // Model-scoped weekly windows arrive only in the generic `limits` array.
    if let Some(limits) = body.get("limits").and_then(|value| value.as_array()) {
        for entry in limits {
            if entry.get("kind").and_then(|kind| kind.as_str()) != Some("weekly_scoped") {
                continue;
            }
            let Some(percent) = entry.get("percent").and_then(|value| value.as_f64()) else {
                continue;
            };
            let model = entry.get("scope").and_then(|scope| scope.get("model"));
            let label = model
                .and_then(|model| model.get("display_name"))
                .and_then(|value| value.as_str())
                .or_else(|| {
                    model
                        .and_then(|model| model.get("id"))
                        .and_then(|value| value.as_str())
                })
                .map(str::trim)
                .filter(|label| !label.is_empty());
            let Some(label) = label else { continue };
            // A scoped window that duplicates the dedicated Opus field is skipped
            // so the same limit is not rendered twice.
            if label.eq_ignore_ascii_case("opus")
                && windows.iter().any(|w| w.window_key == "seven_day_opus")
            {
                continue;
            }
            windows.push(QuotaWindow::from_percent(
                "weekly_scoped",
                label,
                QuotaWindowScope::Weekly,
                QuotaKind::Requests,
                percent,
                parse_reset(entry.get("resets_at").and_then(|value| value.as_str())),
                Confidence::High,
            ));
        }
    }

    windows
}

/// Fetches the published windows.
///
/// `Ok(None)` means no token is stored, so nothing was requested. Errors are
/// sanitized codes: `AUTH_EXPIRED`, `RATE_LIMITED`, `QUOTA_NOT_PUBLISHED`.
pub fn fetch_windows(
    credentials_path: &Path,
    endpoint: &str,
    timeout: Duration,
) -> Result<Option<Vec<QuotaWindow>>, String> {
    let Some(token) = read_access_token(credentials_path) else {
        return Ok(None);
    };
    let headers = [OAUTH_BETA_HEADER];
    let response = get_json(
        JsonRequest {
            timeout,
            ..JsonRequest::new(endpoint)
        }
        .bearer(&token)
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
    std::env::var("LNWDECK_CLAUDE_USAGE_ENDPOINT").unwrap_or_else(|_| USAGE_ENDPOINT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extracts_the_token_from_a_credentials_payload() {
        assert_eq!(
            token_from_payload(r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat-x"}}"#).as_deref(),
            Some("sk-ant-oat-x")
        );
        assert_eq!(token_from_payload("not json"), None);
        assert_eq!(token_from_payload(r#"{"claudeAiOauth":{}}"#), None);
        assert_eq!(
            token_from_payload(r#"{"claudeAiOauth":{"accessToken":"   "}}"#),
            None
        );
    }

    #[test]
    fn reads_the_oauth_token_from_the_credentials_file() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(".credentials.json");
        std::fs::write(
            &path,
            r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat-example","refreshToken":"r"}}"#,
        )
        .expect("write credentials");
        assert_eq!(
            read_access_token(&path).as_deref(),
            Some("sk-ant-oat-example")
        );
    }

    #[test]
    fn an_unusable_credentials_file_falls_through_to_the_credential_manager() {
        let dir = tempdir().expect("temp dir");
        // No file and (on a machine without a stored Claude credential) no
        // Credential Manager entry either: the result is no token, never a
        // fabricated one.
        let from_missing = read_access_token(&dir.path().join("absent.json"));
        let from_manager =
            lnwdeck_windows_integration::CredentialStore::get_by_target("Claude Code-credentials")
                .ok()
                .and_then(|raw| token_from_payload(&raw));
        assert_eq!(
            from_missing, from_manager,
            "the file path and the credential manager path must agree"
        );

        let wrong = dir.path().join("wrong.json");
        std::fs::write(&wrong, "not json").expect("write");
        assert_eq!(read_access_token(&wrong), from_manager);
    }

    #[test]
    fn maps_every_published_window_to_a_percentage() {
        let body = serde_json::json!({
            "five_hour": { "utilization": 37, "resets_at": "2026-08-04T13:01:00Z" },
            "seven_day": { "utilization": 46, "resets_at": "2026-08-06T11:00:00Z" },
            "seven_day_opus": { "utilization": 97, "resets_at": "2026-08-06T12:00:00Z" }
        });
        let windows = windows_from_payload(&body);
        assert_eq!(windows.len(), 3);

        let session = &windows[0];
        assert_eq!(session.label, "Session");
        assert_eq!(session.used_percent, Some(37.0));
        assert_eq!(session.remaining_percent, Some(63.0));
        assert_eq!(session.limit, None, "a percentage is not an absolute limit");
        assert!(session.reset_at.is_some());
        session.check_invariants().expect("consistent");

        assert_eq!(windows[1].label, "Weekly");
        assert_eq!(windows[1].remaining_percent, Some(54.0));
        assert_eq!(windows[2].label, "Weekly Opus");
        assert_eq!(windows[2].remaining_percent, Some(3.0));
    }

    #[test]
    fn model_scoped_weekly_windows_are_included_once() {
        let body = serde_json::json!({
            "five_hour": { "utilization": 10, "resets_at": null },
            "limits": [
                {
                    "kind": "weekly_scoped",
                    "percent": 12,
                    "resets_at": "2026-08-06T12:00:00Z",
                    "scope": { "model": { "display_name": "Sonnet" } }
                },
                {
                    "kind": "weekly_scoped",
                    "percent": 80,
                    "scope": { "model": { "display_name": "Opus" } }
                },
                { "kind": "something_else", "percent": 50 }
            ],
            "seven_day_opus": { "utilization": 80 }
        });
        let windows = windows_from_payload(&body);
        let labels: Vec<&str> = windows.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["Session", "Weekly Opus", "Sonnet"],
            "the scoped Opus entry duplicates the dedicated field and is dropped"
        );
    }

    #[test]
    fn a_payload_without_any_window_produces_nothing() {
        assert!(windows_from_payload(&serde_json::json!({})).is_empty());
        assert!(windows_from_payload(&serde_json::json!({ "five_hour": {} })).is_empty());
        assert!(windows_from_payload(
            &serde_json::json!({ "five_hour": { "utilization": "lots" } })
        )
        .is_empty());
    }

    #[test]
    fn no_stored_token_means_no_request_and_no_error() {
        let dir = tempdir().expect("temp dir");
        let missing = dir.path().join("absent.json");
        // The endpoint is unroutable: reaching it would mean a request was made.
        assert_eq!(
            fetch_windows(
                &missing,
                "https://127.0.0.1:1/usage",
                Duration::from_millis(200)
            ),
            Ok(None)
        );
    }
}
