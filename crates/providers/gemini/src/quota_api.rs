//! Gemini subscription quota, as published by Google's Code Assist API.
//!
//! The Gemini CLI and Antigravity store an OAuth payload in
//! `~/.gemini/oauth_creds.json`; the same token authorises
//! `cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota`, which returns
//! the remaining fraction and reset time of every model's request window.
//! Windows are grouped by model family (pro / flash / flash-lite), taking the
//! lowest remaining fraction per family โ€” a free-tier account sees real
//! windows and a real plan label. When no token is stored, or the API is
//! unreachable, quota is unavailable; the local session scan remains a
//! separate usage channel.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{post_json, JsonRequest};
use std::path::Path;
use std::time::Duration;

const LOAD_CODE_ASSIST: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const RETRIEVE_QUOTA: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// OAuth material the Gemini CLI stores locally. Only the token is read.
#[derive(Debug, Clone, PartialEq)]
pub struct GeminiOAuth {
    pub access_token: String,
}

/// Reads the Gemini OAuth token from `oauth_creds.json`.
pub fn read_oauth(auth_path: &Path) -> Option<GeminiOAuth> {
    let raw = std::fs::read_to_string(auth_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let access_token = value
        .get("access_token")
        .and_then(|token| token.as_str())?
        .trim()
        .to_string();
    if access_token.is_empty() {
        return None;
    }
    Some(GeminiOAuth { access_token })
}

fn model_is_pro(id: &str) -> bool {
    id.to_lowercase().contains("pro")
}

fn model_is_flash_lite(id: &str) -> bool {
    id.to_lowercase().contains("flash-lite")
}

fn model_is_flash(id: &str) -> bool {
    let lower = id.to_lowercase();
    lower.contains("flash") && !model_is_flash_lite(id)
}

/// Plan label from the tier id Google publishes.
fn plan_label(tier: &str) -> Option<&'static str> {
    match tier {
        "standard-tier" => Some("Paid"),
        "legacy-tier" => Some("Legacy"),
        "free-tier" => Some("Free"),
        _ => None,
    }
}

struct Bucket {
    model_id: String,
    remaining_fraction: f64,
    reset_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn parse_buckets(value: &serde_json::Value) -> Vec<Bucket> {
    let Some(buckets) = value.get("buckets").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for bucket in buckets {
        let Some(model_id) = bucket.get("modelId").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(remaining) = bucket.get("remainingFraction").and_then(|v| v.as_f64()) else {
            continue;
        };
        if !remaining.is_finite() || !(0.0..=1.0).contains(&remaining) {
            continue;
        }
        let reset_at = bucket
            .get("resetTime")
            .and_then(|v| v.as_str())
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .map(|parsed| parsed.with_timezone(&chrono::Utc));
        out.push(Bucket {
            model_id: model_id.to_string(),
            remaining_fraction: remaining,
            reset_at,
        });
    }
    out
}

fn pick_lowest(buckets: &[Bucket], predicate: impl Fn(&str) -> bool) -> Option<&Bucket> {
    buckets
        .iter()
        .filter(|bucket| predicate(&bucket.model_id))
        .min_by(|a, b| {
            a.remaining_fraction
                .partial_cmp(&b.remaining_fraction)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn to_window(bucket: &Bucket, key: &str, label: &str, scope: QuotaWindowScope) -> QuotaWindow {
    let used_percent = 100.0 - bucket.remaining_fraction * 100.0;
    QuotaWindow::from_percent(
        key,
        label,
        scope,
        QuotaKind::Requests,
        used_percent,
        bucket.reset_at,
        Confidence::High,
    )
}

/// Converts the `retrieveUserQuota` payload into quota windows: the lowest
/// remaining fraction per model family, falling back to the overall lowest
/// when no family is recognizable.
pub fn windows_from_payload(body: &serde_json::Value) -> (Vec<QuotaWindow>, Option<String>) {
    let buckets = parse_buckets(body);
    if buckets.is_empty() {
        return (Vec::new(), None);
    }
    let pro = pick_lowest(&buckets, model_is_pro);
    let flash = pick_lowest(&buckets, model_is_flash);
    let flash_lite = pick_lowest(&buckets, model_is_flash_lite);
    let fallback = if pro.is_none() && flash.is_none() && flash_lite.is_none() {
        buckets.iter().min_by(|a, b| {
            a.remaining_fraction
                .partial_cmp(&b.remaining_fraction)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    } else {
        None
    };

    let mut windows = Vec::new();
    if let Some(bucket) = pro.or(fallback) {
        windows.push(to_window(
            bucket,
            "pro",
            "Gemini Pro",
            QuotaWindowScope::Weekly,
        ));
    }
    if let Some(bucket) = flash {
        windows.push(to_window(
            bucket,
            "flash",
            "Gemini Flash",
            QuotaWindowScope::Weekly,
        ));
    }
    if let Some(bucket) = flash_lite {
        windows.push(to_window(
            bucket,
            "flash-lite",
            "Gemini Flash-Lite",
            QuotaWindowScope::Weekly,
        ));
    }
    (windows, None)
}

/// Fetches the published Gemini quota windows.
///
/// `Ok(None)` means no token is stored, so nothing was requested. The tier
/// label comes from the Code Assist status endpoint; a failure there still
/// yields the quota windows.
pub fn fetch_windows(auth_path: &Path, timeout: Duration) -> Result<Option<QuotaReport>, String> {
    let Some(oauth) = read_oauth(auth_path) else {
        return Ok(None);
    };

    let assist = post_json(
        JsonRequest {
            raw_auth_token: None,
            browser_cookie: None,
            url: LOAD_CODE_ASSIST,
            bearer_token: Some(&oauth.access_token),
            timeout,
            capture_headers: &[],
            extra_headers: &[],
        },
        &serde_json::json!({ "metadata": { "ideType": "GEMINI_CLI", "pluginType": "GEMINI" } }),
    )
    .map_err(|code| {
        if code == "AUTH_EXPIRED" || code == "RATE_LIMITED" {
            code
        } else {
            "QUOTA_NOT_PUBLISHED".to_string()
        }
    })?;

    let tier = assist
        .body
        .get("currentTier")
        .and_then(|tier| tier.get("id"))
        .and_then(|id| id.as_str())
        .map(str::to_string);
    let project_id = assist
        .body
        .get("projectId")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let quota_body = if let Some(project) = project_id {
        serde_json::json!({ "project": project })
    } else {
        serde_json::json!({})
    };
    let response = post_json(
        JsonRequest {
            raw_auth_token: None,
            browser_cookie: None,
            url: RETRIEVE_QUOTA,
            bearer_token: Some(&oauth.access_token),
            timeout,
            capture_headers: &[],
            extra_headers: &[],
        },
        &quota_body,
    )?;

    let (windows, _) = windows_from_payload(&response.body);
    if windows.is_empty() {
        return Err("QUOTA_NOT_PUBLISHED".to_string());
    }
    let mut report = QuotaReport::new("google_gemini", "provider_api", windows, DEFAULT_FRESHNESS);
    if let Some(tier) = tier {
        report.plan = plan_label(&tier).map(str::to_string);
    }
    Ok(Some(report))
}

/// Default endpoint override for verification against a local server.
pub fn default_timeout() -> Duration {
    REQUEST_TIMEOUT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_token_from_oauth_creds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("oauth_creds.json");
        std::fs::write(
            &path,
            r#"{"access_token":"eyJ-example","expiry_date":1786000000000,"refresh_token":"r"}"#,
        )
        .expect("write creds");
        let oauth = read_oauth(&path).expect("oauth");
        assert_eq!(oauth.access_token, "eyJ-example");
    }

    #[test]
    fn a_missing_or_blank_token_yields_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(read_oauth(&dir.path().join("absent.json")), None);
        let blank = dir.path().join("blank.json");
        std::fs::write(&blank, r#"{"access_token":""}"#).expect("write");
        assert_eq!(read_oauth(&blank), None);
    }

    #[test]
    fn buckets_are_grouped_by_model_family_with_the_lowest_fraction() {
        let body = serde_json::json!({
            "buckets": [
                { "modelId": "gemini-2.5-flash", "remainingFraction": 0.9, "resetTime": "2026-08-08T04:49:45Z" },
                { "modelId": "gemini-2.5-flash-lite", "remainingFraction": 1.0, "resetTime": "2026-08-08T04:49:45Z" },
                { "modelId": "gemini-2.5-pro", "remainingFraction": 0.4, "resetTime": "2026-08-08T04:49:45Z" },
                { "modelId": "gemini-3.1-flash-lite", "remainingFraction": 0.2, "resetTime": "2026-08-08T04:49:45Z" },
            ]
        });
        let (windows, _) = windows_from_payload(&body);
        assert_eq!(windows.len(), 3, "pro, flash and flash-lite families");
        let pro = windows.iter().find(|w| w.window_key == "pro").unwrap();
        assert_eq!(pro.used_percent, Some(60.0), "0.4 remaining = 60% used");
        let flash = windows.iter().find(|w| w.window_key == "flash").unwrap();
        assert_eq!(flash.used_percent, Some(10.0));
        let lite = windows
            .iter()
            .find(|w| w.window_key == "flash-lite")
            .unwrap();
        assert_eq!(lite.used_percent, Some(80.0), "lowest flash-lite wins");
        assert!(pro.reset_at.is_some());
        for window in &windows {
            window.check_invariants().expect("consistent");
        }
    }

    #[test]
    fn unknown_models_fall_back_to_the_lowest_bucket() {
        let body = serde_json::json!({
            "buckets": [
                { "modelId": "custom-model", "remainingFraction": 0.5, "resetTime": "2026-08-08T04:49:45Z" },
                { "modelId": "other-model", "remainingFraction": 0.1, "resetTime": "2026-08-08T04:49:45Z" },
            ]
        });
        let (windows, _) = windows_from_payload(&body);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, Some(90.0));
    }

    #[test]
    fn a_payload_without_buckets_produces_nothing() {
        assert!(windows_from_payload(&serde_json::json!({})).0.is_empty());
        assert!(windows_from_payload(&serde_json::json!({ "buckets": [] }))
            .0
            .is_empty());
        assert!(
            windows_from_payload(&serde_json::json!({ "buckets": [{}] }))
                .0
                .is_empty()
        );
    }

    #[test]
    fn out_of_range_remaining_fractions_are_ignored() {
        let body = serde_json::json!({
            "buckets": [
                { "modelId": "gemini-pro", "remainingFraction": 1.5 },
                { "modelId": "gemini-flash", "remainingFraction": -0.1 }
            ]
        });
        assert!(
            windows_from_payload(&body).0.is_empty(),
            "an invalid fraction must not become a negative or over-100 percentage"
        );
    }

    #[test]
    fn no_stored_token_means_no_request_and_no_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            fetch_windows(&dir.path().join("absent.json"), Duration::from_millis(200)),
            Ok(None)
        );
    }
}
