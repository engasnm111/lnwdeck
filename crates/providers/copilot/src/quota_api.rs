//! GitHub Copilot subscription quota from the same endpoint used by Copilot clients.

use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, JsonRequest};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

const COPILOT_USER_URL: &str = "https://api.github.com/copilot_internal/user";

fn likely_token(value: &str) -> bool {
    let value = value.trim();
    (value.starts_with("gho_")
        || value.starts_with("ghu_")
        || value.starts_with("ghp_")
        || value.starts_with("ghs_")
        || value.starts_with("ghr_")
        || value.starts_with("github_pat_"))
        && value.len() >= 24
}

fn token_from_json(path: &Path) -> Option<String> {
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let object = value.as_object()?;
    let mut fallback = None;
    for (key, entry) in object {
        let Some(token) = entry
            .get("oauth_token")
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            continue;
        };
        if !likely_token(token) {
            continue;
        }
        if key.split(':').next() == Some("github.com") {
            return Some(token.to_string());
        }
        fallback.get_or_insert_with(|| token.to_string());
    }
    fallback
}

fn token_from_auth_db(path: &Path) -> Option<String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT token_schema_version, hex(token_ciphertext) \
             FROM oauth_tokens ORDER BY last_used_at DESC, updated_at DESC",
        )
        .ok()?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .ok()?;
    for row in rows.flatten() {
        // TokenTracker can decode schema 0 on every platform. Newer encrypted
        // auth.db schemas need an OS-secret-store key; TokenTracker currently
        // does not read that key on Windows either, so we do not guess here.
        if row.0 != 0 || row.1.len() % 2 != 0 {
            continue;
        }
        let Some(bytes) = (0..row.1.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&row.1[index..index + 2], 16).ok())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Ok(token) = String::from_utf8(bytes) else {
            continue;
        };
        if likely_token(&token) {
            return Some(token.trim().to_string());
        }
    }
    None
}

pub fn read_oauth_token(home: &Path) -> Option<String> {
    let root = home.join(".config").join("github-copilot");
    for name in ["apps.json", "hosts.json"] {
        if let Some(token) = token_from_json(&root.join(name)) {
            return Some(token);
        }
    }
    token_from_auth_db(&root.join("auth.db"))
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

fn reset_at(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let text = value?.as_str()?.trim();
    let normalized = if text.len() == 10 && text.as_bytes().get(4) == Some(&b'-') {
        format!("{text}T00:00:00Z")
    } else {
        text.to_string()
    };
    DateTime::parse_from_rfc3339(&normalized)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn snapshot_window(
    snapshot: &Value,
    reset: Option<DateTime<Utc>>,
    key: &str,
    label: &str,
) -> Option<QuotaWindow> {
    let entitlement = number(snapshot.get("entitlement"));
    let remaining = number(snapshot.get("remaining"));
    let percent_remaining = number(snapshot.get("percent_remaining"));
    if entitlement.unwrap_or(0.0) <= 0.0
        && remaining.unwrap_or(0.0) <= 0.0
        && percent_remaining.unwrap_or(0.0) <= 0.0
    {
        return None;
    }
    let used_percent = if let Some(percent) = percent_remaining {
        100.0 - percent
    } else {
        let entitlement = entitlement.filter(|value| *value > 0.0)?;
        let remaining = remaining?;
        (entitlement - remaining) / entitlement * 100.0
    }
    .clamp(0.0, 100.0);
    Some(QuotaWindow::from_percent(
        key,
        label,
        QuotaWindowScope::Monthly,
        QuotaKind::Requests,
        used_percent,
        reset,
        Confidence::High,
    ))
}

pub fn windows_from_payload(payload: &Value) -> Vec<QuotaWindow> {
    let reset = reset_at(payload.get("quota_reset_date"));
    let snapshots = payload.get("quota_snapshots").unwrap_or(&Value::Null);
    [
        ("premium_interactions", "Premium interactions"),
        ("chat", "Chat"),
    ]
    .into_iter()
    .filter_map(|(key, label)| snapshot_window(snapshots.get(key)?, reset, key, label))
    .collect()
}

pub fn fetch_report(home: &Path, timeout: Duration) -> Result<Option<QuotaReport>, String> {
    let Some(token) = read_oauth_token(home) else {
        return Ok(None);
    };
    let headers = [
        ("editor-version", "vscode/1.96.2"),
        ("editor-plugin-version", "copilot-chat/0.26.7"),
        ("user-agent", "GitHubCopilotChat/0.26.7"),
        ("x-github-api-version", "2025-04-01"),
    ];
    let auth = format!("token {token}");
    let response = get_json(
        JsonRequest {
            timeout,
            ..JsonRequest::new(COPILOT_USER_URL)
        }
        .raw_auth(&auth)
        .with_headers(&headers),
    )?;
    let windows = windows_from_payload(&response.body);
    if windows.is_empty() {
        return Ok(None);
    }
    let mut report = QuotaReport::new("github_copilot", "provider_api", windows, DEFAULT_FRESHNESS);
    report.plan = response
        .body
        .get("copilot_plan")
        .and_then(Value::as_str)
        .map(|plan| plan.to_string());
    Ok(Some(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_premium_and_chat_quota() {
        let payload = serde_json::json!({
            "copilot_plan": "pro",
            "quota_reset_date": "2026-10-01",
            "quota_snapshots": {
                "premium_interactions": {"entitlement": 300, "remaining": 225, "percent_remaining": 75},
                "chat": {"entitlement": 1000, "remaining": 400}
            }
        });
        let windows = windows_from_payload(&payload);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].used_percent, Some(25.0));
        assert_eq!(windows[1].used_percent, Some(60.0));
        assert!(windows.iter().all(|window| window.reset_at.is_some()));
    }

    #[test]
    fn reads_legacy_plaintext_auth() {
        let dir = tempfile::tempdir().expect("temp dir");
        let auth = dir.path().join(".config").join("github-copilot");
        std::fs::create_dir_all(&auth).expect("auth dir");
        std::fs::write(
            auth.join("hosts.json"),
            r#"{"github.com":{"oauth_token":"gho_abcdefghijklmnopqrstuvwxyz0123456789"}}"#,
        )
        .expect("write auth");
        assert!(read_oauth_token(dir.path()).is_some());
    }
}
