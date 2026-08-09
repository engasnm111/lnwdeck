//! Kimi Code subscription-quota API integration.
//!
//! Kimi Code stores its OAuth material in the CLI-owned
//! `credentials/kimi-code.json` file. This module reads the access token only
//! for the duration of the HTTPS request; it never returns or persists the
//! credential. The response is normalized from Kimi's `usage.limits` and
//! `usage`, `limits[].detail` and `totalQuota` objects without creating a local
//! estimate.

use chrono::{DateTime, Utc};
use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};
use lnwdeck_provider_http::{get_json, post_form, JsonRequest};
use serde_json::Value;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

pub const USAGE_URL: &str = "https://api.kimi.com/coding/v1/usages";
const AUTH_URL: &str = "https://auth.kimi.com/api/oauth/token";
const OAUTH_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
const CREDENTIAL_FILE: &str = "credentials/kimi-code.json";

struct Credentials {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<f64>,
}

/// Fetches the provider-published Kimi Code quota windows.
pub fn fetch_windows(
    roots: &[PathBuf],
    timeout: Duration,
) -> Result<Option<Vec<QuotaWindow>>, String> {
    let Some(credentials) = read_credentials(roots)? else {
        return Err("NOT_CONFIGURED".to_string());
    };
    let mut token = credentials.access_token.clone();
    if credentials_expired(credentials.expires_at) {
        if let Some(refresh_token) = credentials.refresh_token.as_deref() {
            token = Some(refresh_access_token(refresh_token, timeout)?);
        }
    }

    let Some(token) = token else {
        return Err("AUTH_EXPIRED".to_string());
    };
    match fetch_usage(&token, timeout) {
        Ok(response) => windows_from_payload(&response.body),
        Err(code) if code == "AUTH_EXPIRED" => {
            let Some(refresh_token) = credentials.refresh_token.as_deref() else {
                return Err(code);
            };
            let refreshed = refresh_access_token(refresh_token, timeout)?;
            let response = fetch_usage(&refreshed, timeout)?;
            windows_from_payload(&response.body)
        }
        Err(code) => Err(code),
    }
}

fn fetch_usage(
    token: &str,
    timeout: Duration,
) -> Result<lnwdeck_provider_http::HttpResponse, String> {
    get_json(JsonRequest {
        timeout,
        ..JsonRequest::new(USAGE_URL).bearer(token)
    })
}

fn refresh_access_token(refresh_token: &str, timeout: Duration) -> Result<String, String> {
    if refresh_token.trim().is_empty() {
        return Err("AUTH_EXPIRED".to_string());
    }
    let headers = [("X-Msh-Platform", "kimi_cli")];
    let form = [
        ("client_id", OAUTH_CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    let response = post_form(
        JsonRequest {
            timeout,
            ..JsonRequest::new(AUTH_URL)
        }
        .with_headers(&headers),
        &form,
    )?;
    response
        .body
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())
}

/// Parses the stable portion of the Kimi usage response.
///
/// Kimi publishes a primary `usage` window, an optional first
/// `limits[].detail` window, and an optional `totalQuota` window. We accept
/// the fields used by the provider API, but never derive a limit from local
/// wire-log token totals.
pub fn windows_from_payload(payload: &Value) -> Result<Option<Vec<QuotaWindow>>, String> {
    let usage = payload
        .get("usage")
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let mut windows = Vec::new();
    if let Some(window) = window_from_detail(usage, 0, "Kimi usage quota") {
        windows.push(window);
    }

    let limits = payload
        .get("limits")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if let Some(limit) = limits.first() {
        let detail = limit.get("detail").unwrap_or(limit);
        if let Some(window) = window_from_detail(detail, 1, "Kimi coding quota") {
            windows.push(window);
        }
    }

    if let Some(total) = payload.get("totalQuota") {
        if let Some(window) = window_from_detail(total, 2, "Kimi total quota") {
            if !windows
                .iter()
                .any(|existing| existing.window_key == window.window_key)
            {
                windows.push(window);
            }
        }
    }

    if windows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(windows))
    }
}

fn read_credentials(roots: &[PathBuf]) -> Result<Option<Credentials>, String> {
    for root in roots {
        let path = root.join(CREDENTIAL_FILE);
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                "PERMISSION_DENIED".to_string()
            } else {
                "SOURCE_UNAVAILABLE".to_string()
            }
        })?;
        let credentials: Value =
            serde_json::from_str(&raw).map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        let access_token = ["access_token", "accessToken", "token"]
            .iter()
            .find_map(|key| credentials.get(*key).and_then(Value::as_str))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string);
        let refresh_token = credentials
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string);
        if access_token.is_some() || refresh_token.is_some() {
            return Ok(Some(Credentials {
                access_token,
                refresh_token,
                expires_at: number(&credentials, &["expires_at", "expiresAt"]),
            }));
        }
    }
    Ok(None)
}

fn credentials_expired(expires_at: Option<f64>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    expires_at > 0.0
        && expires_at * 1_000.0 <= chrono::Utc::now().timestamp_millis() as f64 + 30_000.0
}

fn window_from_detail(detail: &Value, index: usize, default_label: &str) -> Option<QuotaWindow> {
    let used = number(detail, &["used", "usedQuota", "consumed"]);
    let remaining = number(detail, &["remaining", "remainingQuota"]);
    let limit = number(detail, &["limit", "total", "quota"]);
    let reset_at = reset_at(detail);
    let scope = scope(detail);
    let window_key = format!("kimi_{index}");
    let label = if index == 0 {
        default_label.to_string()
    } else {
        format!("{default_label} {index}")
    };

    let percent = ["usedPercent", "usagePercent", "percentage"]
        .iter()
        .find_map(|key| number(detail, &[*key]))
        .filter(|percent| (0.0..=100.0).contains(percent));

    if let Some(limit) = whole_number(limit) {
        if used.is_none() && remaining.is_none() && percent.is_none() {
            return None;
        }
        let limit = NonZeroU64::new(limit)?;
        let used = used
            .or_else(|| remaining.map(|value| (limit.get() as f64 - value).max(0.0)))
            .and_then(|value| whole_number(Some(value)))
            .unwrap_or(0);
        return Some(QuotaWindow::with_limit(
            window_key,
            label,
            scope,
            QuotaKind::Credits,
            used,
            limit,
            reset_at,
            Confidence::High,
        ));
    }

    if let (Some(used), Some(remaining)) = (used, remaining) {
        if let Some(limit) = whole_number(Some(used + remaining)).and_then(NonZeroU64::new) {
            return Some(QuotaWindow::with_limit(
                window_key,
                label,
                scope,
                QuotaKind::Credits,
                whole_number(Some(used)).unwrap_or(0),
                limit,
                reset_at,
                Confidence::High,
            ));
        }
    }

    percent.map(|percent| {
        QuotaWindow::from_percent(
            window_key,
            label,
            scope,
            QuotaKind::Credits,
            percent,
            reset_at,
            Confidence::High,
        )
    })
}

fn number(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        })
        .filter(|value: &f64| value.is_finite() && *value >= 0.0)
}

fn whole_number(value: Option<f64>) -> Option<u64> {
    value.map(|value| value.round().min(u64::MAX as f64) as u64)
}

fn scope(value: &Value) -> QuotaWindowScope {
    let text = ["window", "period", "name"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if text.contains('h') || text.contains("hour") {
        QuotaWindowScope::Rolling
    } else if text.contains("day") {
        QuotaWindowScope::Daily
    } else if text.contains("week") {
        QuotaWindowScope::Weekly
    } else if text.contains("month") {
        QuotaWindowScope::Monthly
    } else {
        QuotaWindowScope::Other
    }
}

fn reset_at(value: &Value) -> Option<DateTime<Utc>> {
    let reset = ["resetTime", "reset_at", "resetAt"]
        .iter()
        .find_map(|key| value.get(*key))?;
    match reset {
        Value::String(text) => DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|date| date.with_timezone(&Utc)),
        Value::Number(number) => timestamp(number.as_f64()?),
        _ => None,
    }
}

fn timestamp(value: f64) -> Option<DateTime<Utc>> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    if value >= 10_000_000_000.0 {
        DateTime::from_timestamp_millis(value.round() as i64)
    } else {
        DateTime::from_timestamp(value.round() as i64, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kimi_detail_and_total_quota() {
        let payload = serde_json::json!({
            "usage": {
                "limit": 2000,
                "used": 500,
                "remaining": 1500,
                "resetTime": "2026-08-10T00:00:00Z"
            },
            "limits": [{
                "detail": {
                    "limit": 1000,
                    "used": 250,
                    "remaining": 750,
                    "resetTime": "2026-08-11T00:00:00Z"
                }
            }],
            "totalQuota": {
                "limit": "5000",
                "used": "1000",
                "resetTime": "2026-08-15T00:00:00Z"
            }
        });
        let windows = windows_from_payload(&payload)
            .expect("payload")
            .expect("windows");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].limit, Some(2000));
        assert_eq!(windows[0].used, 500);
        assert_eq!(windows[0].remaining, Some(1500));
        assert!(windows[0].reset_at.is_some());
        assert_eq!(windows[1].limit, Some(1000));
        assert_eq!(windows[1].used, 250);
        assert_eq!(windows[2].limit, Some(5000));
        assert_eq!(windows[2].used, 1000);
        for window in windows {
            window.check_invariants().expect("valid window");
        }
    }

    #[test]
    fn empty_or_missing_kimi_limits_do_not_create_a_bar() {
        for payload in [
            serde_json::json!({"usage": {"limit": 1000}}),
            serde_json::json!({"usage": {}, "limits": [{"detail": {}}]}),
        ] {
            assert_eq!(windows_from_payload(&payload).expect("schema"), None);
        }
    }

    #[test]
    fn malformed_kimi_payload_is_rejected() {
        assert_eq!(
            windows_from_payload(&serde_json::json!({})),
            Err("SOURCE_SCHEMA_MISMATCH".to_string())
        );
    }
}
