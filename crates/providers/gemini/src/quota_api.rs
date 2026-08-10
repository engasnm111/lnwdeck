//! Gemini subscription quota, as published by Google's Code Assist API.
//!
//! The Gemini CLI stores a legacy OAuth payload in `~/.gemini/oauth_creds.json`,
//! while the current Antigravity CLI keeps its token in the OS credential
//! manager under `gemini:antigravity` (a JSON document with a `token` object).
//! The same token authorises
//! `cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota`, which returns
//! the remaining fraction and reset time of every model's request window.
//! Windows are grouped by model family (pro / flash / flash-lite), taking the
//! lowest remaining fraction per family โ€” a free-tier account sees real
//! windows and a real plan label. When no token is stored, or the API is
//! unreachable, quota is unavailable; the local session scan remains a
//! separate usage channel.
//!
//! Access tokens expire hourly, so a stored refresh token is exchanged at
//! `oauth2.googleapis.com/token` using the same public OAuth client the
//! Antigravity CLI ships with; the refreshed token is used for the request and
//! never persisted or logged by lnwdeck.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{post_form, post_json, JsonRequest};
use std::path::Path;
use std::time::Duration;

const LOAD_CODE_ASSIST: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const RETRIEVE_QUOTA: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Windows Credential Manager target the Antigravity CLI writes its OAuth
/// payload to.
const ANTIGRAVITY_KEYRING_TARGET: &str = "gemini:antigravity";

/// OAuth material the Gemini/Antigravity CLIs store locally. Only the token
/// material is read; the raw values must never be logged or serialized.
#[derive(Debug, Clone, PartialEq)]
pub struct GeminiOAuth {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Expiry of the access token as epoch milliseconds, when published.
    pub expires_at_ms: Option<f64>,
}

/// Reads the Gemini OAuth token from `oauth_creds.json`.
pub fn read_oauth(auth_path: &Path) -> Option<GeminiOAuth> {
    read_oauth_with(auth_path, None)
}

/// Reads the OAuth token from the Windows Credential Manager blob the
/// Antigravity CLI writes, preferring it over the legacy creds file.
pub fn read_oauth_from_keyring() -> Option<GeminiOAuth> {
    #[cfg(windows)]
    {
        let raw = read_credential(ANTIGRAVITY_KEYRING_TARGET).ok()?;
        keyring_oauth(&raw)
    }
    #[cfg(not(windows))]
    {
        let _ = ANTIGRAVITY_KEYRING_TARGET;
        None
    }
}
/// Reads a generic credential from the Windows Credential Manager without
/// exposing the value through logs or read models.
#[cfg(windows)]
fn read_credential(target: &str) -> Result<String, String> {
    lnwdeck_windows_integration::CredentialStore::get_by_target(target)
        .map_err(|_| "CREDENTIAL_UNAVAILABLE".to_string())
}

/// Parses the JSON document the Antigravity CLI stores in the OS credential
/// manager: `{"token": {"access_token", "refresh_token", "expiry"}, ...}`.
fn keyring_oauth(raw: &str) -> Option<GeminiOAuth> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let token = value.get("token")?;
    let access_token = token
        .get("access_token")
        .and_then(|value| value.as_str())?
        .trim()
        .to_string();
    if access_token.is_empty() {
        return None;
    }
    let refresh_token = token
        .get("refresh_token")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string);
    let expires_at_ms = token
        .get("expiry")
        .and_then(|value| value.as_str())
        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(|parsed| parsed.timestamp_millis() as f64);
    Some(GeminiOAuth {
        access_token,
        refresh_token,
        expires_at_ms,
    })
}

/// Reads the OAuth token preferring the keyring blob when provided, falling
/// back to the legacy `oauth_creds.json` file.
fn read_oauth_with(auth_path: &Path, keyring: Option<&str>) -> Option<GeminiOAuth> {
    if let Some(raw) = keyring {
        if let Some(oauth) = keyring_oauth(raw) {
            return Some(oauth);
        }
    }
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
    let refresh_token = value
        .get("refresh_token")
        .and_then(|token| token.as_str())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string);
    let expires_at_ms = value
        .get("expiry_date")
        .or_else(|| value.get("expires_at"))
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0);
    Some(GeminiOAuth {
        access_token,
        refresh_token,
        expires_at_ms,
    })
}

/// True when the stored access token has expired (or no expiry is known but
/// the token is known to be short-lived from the API response).
fn is_expired(oauth: &GeminiOAuth) -> bool {
    let Some(expires_at_ms) = oauth.expires_at_ms else {
        return false;
    };
    expires_at_ms * 1_000.0 <= chrono::Utc::now().timestamp_millis() as f64 + 30_000.0
}

/// OAuth client pair (id, secret) the Antigravity IDE ships. Read from the
/// installed IDE bundle at runtime; never hardcoded in source.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthClient {
    client_id: String,
    client_secret: String,
}

/// Reads the OAuth client pairs from the installed Antigravity IDE bundle.
///
/// The IDE ships its Google OAuth clients in
/// `resources/app/out/main.js` inside the embedded
/// `out-build/vs/platform/cloudCode/common/oauthClient.js` module (a
/// minified chunk where the client id and its secret are consecutive string
/// assignments). lnwdeck locates that module on disk, extracts every
/// client-id / GOCSPX-secret pair it finds, and returns them in declaration
/// order — mirroring how TokenTracker reads gemini-cli's `oauth2.js` instead
/// of hardcoding client identifiers in source.
fn load_oauth_clients() -> Vec<OAuthClient> {
    let Some(path) = antigravity_main_js_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    extract_oauth_clients(&content)
}

/// Path to the Antigravity IDE `out/main.js`, when the IDE is installed on
/// this machine. Checks the common per-user and per-machine install roots.
fn antigravity_main_js_path() -> Option<std::path::PathBuf> {
    let mut roots = Vec::new();
    for var in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(value) = std::env::var_os(var) {
            let base = std::path::PathBuf::from(&value);
            roots.push(base.join("Programs"));
            roots.push(base);
        }
    }
    for root in roots {
        for dir in ["Antigravity IDE", "antigravity"] {
            let candidate = root
                .join(dir)
                .join("resources")
                .join("app")
                .join("out")
                .join("main.js");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Extracts `(client_id, client_secret)` pairs from the oauthClient.js module
/// embedded in the IDE's `out/main.js`.
///
/// The module is minified, so the parser works on the stable shape instead of
/// variable names: after the `oauthClient.js` module marker it collects
/// quoted string assignments (`="..."`) in order, keeps the values that look
/// like a Google OAuth client id (ends with `.apps.googleusercontent.com`)
/// and the values that look like a client secret (starts with `GOCSPX-`),
/// then pairs them by declaration order.
fn extract_oauth_clients(main_js: &str) -> Vec<OAuthClient> {
    let Some(marker) = main_js.find("oauthClient.js") else {
        return Vec::new();
    };
    // Bound the scan to the module region so unrelated constants in the
    // bundle do not get paired.
    let module = &main_js[marker..main_js.len().min(marker + 4096)];
    let mut ids = Vec::new();
    let mut secrets = Vec::new();
    let mut rest = module;
    while let Some(eq) = rest.find("=\"") {
        let after_quote = &rest[eq + 2..];
        let Some(end) = after_quote.find('"') else {
            break;
        };
        let value = &after_quote[..end];
        if is_client_id(value) {
            ids.push(value.to_string());
        } else if is_client_secret(value) {
            secrets.push(value.to_string());
        }
        rest = &after_quote[end + 1..];
    }
    ids.into_iter()
        .zip(secrets)
        .map(|(client_id, client_secret)| OAuthClient {
            client_id,
            client_secret,
        })
        .collect()
}

/// Google OAuth client id shape: `<number>-<slug>.apps.googleusercontent.com`.
fn is_client_id(value: &str) -> bool {
    let Some(rest) = value.strip_suffix(".apps.googleusercontent.com") else {
        return false;
    };
    let Some((numeric, slug)) = rest.split_once('-') else {
        return false;
    };
    !numeric.is_empty()
        && numeric.chars().all(|c| c.is_ascii_digit())
        && slug.len() >= 10
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// OAuth client secret shape: `GOCSPX-` followed by at least 8 characters.
fn is_client_secret(value: &str) -> bool {
    value.len() >= 16
        && value.strip_prefix("GOCSPX-").is_some_and(|rest| {
            rest.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

/// Refreshes the access token with the Antigravity OAuth client.
fn refresh_access_token(refresh_token: &str, timeout: Duration) -> Result<String, String> {
    if refresh_token.trim().is_empty() {
        return Err("AUTH_EXPIRED".to_string());
    }
    let clients = load_oauth_clients();
    if clients.is_empty() {
        return Err("SOURCE_UNAVAILABLE".to_string());
    }
    let mut last_error = None;
    for client in &clients {
        match refresh_with_client(client, refresh_token, timeout) {
            Ok(token) => return Ok(token),
            Err(code) => last_error = Some(code),
        }
    }
    Err(last_error.unwrap_or_else(|| "AUTH_EXPIRED".to_string()))
}

/// One refresh attempt with a specific OAuth client pair.
fn refresh_with_client(
    client: &OAuthClient,
    refresh_token: &str,
    timeout: Duration,
) -> Result<String, String> {
    let form = [
        ("client_id", client.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    let response = post_form(
        JsonRequest {
            timeout,
            ..JsonRequest::new(OAUTH_TOKEN_URL)
        },
        &form,
    )?;
    response
        .body
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())
}

/// Resolves the token to use for the quota request: refresh first when the
/// stored token has expired or the API rejected it.
fn resolve_token(oauth: &GeminiOAuth, timeout: Duration, rejected: bool) -> Result<String, String> {
    if !rejected && !is_expired(oauth) {
        return Ok(oauth.access_token.clone());
    }
    let Some(refresh_token) = oauth.refresh_token.as_deref() else {
        return Err("AUTH_EXPIRED".to_string());
    };
    refresh_access_token(refresh_token, timeout)
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

/// Tier id from the Code Assist status payload. The current CLI publishes
/// `allowedTiers` (with `ineligibleTiers` carrying the reason); older clients
/// returned `currentTier.id`.
fn tier_from_assist(body: &serde_json::Value) -> Option<String> {
    body.get("currentTier")
        .and_then(|tier| tier.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            body.get("allowedTiers")
                .and_then(serde_json::Value::as_array)
                .and_then(|tiers| tiers.first())
                .and_then(|tier| tier.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
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
/// `Ok(None)` means no token is stored, so nothing was requested. When
/// `use_keyring` is set, the Antigravity CLI's Windows Credential Manager
/// blob is consulted first and the legacy `oauth_creds.json` file is the
/// fallback; otherwise only the file is read. The tier label comes from the
/// Code Assist status endpoint; a failure there still yields the quota
/// windows.
pub fn fetch_windows(
    auth_path: &Path,
    timeout: Duration,
    use_keyring: bool,
) -> Result<Option<QuotaReport>, String> {
    let oauth = if use_keyring {
        read_oauth_from_keyring().or_else(|| read_oauth(auth_path))
    } else {
        read_oauth(auth_path)
    };
    let Some(oauth) = oauth else {
        return Ok(None);
    };

    // Preferred: the detailed weekly / 5-hour quota the Antigravity IDE
    // shows, read from its language server while it is running. Falls back
    // to the basic request-quota API below when the IDE is not open.
    if let Ok(Some(report)) = fetch_ls_report(timeout) {
        return Ok(Some(report));
    }

    let mut token = resolve_token(&oauth, timeout, false)?;
    let mut report = match fetch_report(&token, timeout) {
        Ok(report) => report,
        Err(code) if code == "AUTH_EXPIRED" => {
            token = resolve_token(&oauth, timeout, true)?;
            fetch_report(&token, timeout)?
        }
        Err(code) => return Err(code),
    };

    let assist = match post_json(
        JsonRequest {
            raw_auth_token: None,
            browser_cookie: None,
            url: LOAD_CODE_ASSIST,
            bearer_token: Some(&token),
            timeout,
            capture_headers: &[],
            extra_headers: &[],
        },
        &serde_json::json!({ "metadata": { "ideType": "GEMINI_CLI", "pluginType": "GEMINI" } }),
    ) {
        Ok(response) => response.body,
        Err(_) => return Ok(Some(report)),
    };
    if let Some(tier) = tier_from_assist(&assist) {
        report.plan = plan_label(&tier).map(str::to_string);
    }
    Ok(Some(report))
}

/// Detailed quota report from the Antigravity IDE language server.
///
/// `Ok(None)` means the IDE is not running (or its language server is not
/// reachable), so the caller should fall back to the basic quota API.
fn fetch_ls_report(timeout: Duration) -> Result<Option<QuotaReport>, String> {
    let ls = match lnwdeck_windows_integration::antigravity_ls::discover() {
        Ok(ls) => ls,
        Err(_) => return Ok(None),
    };
    // The LS is a localhost process, so a short per-port budget is plenty.
    // Each port probed with the full remote timeout would stack (three ports
    // at 15s each = 45s) and stall the whole refresh cycle.
    let ls_timeout = timeout.min(Duration::from_secs(3));
    match super::ls_quota::fetch_ls_windows(&ls.ports, &ls.csrf_token, ls_timeout) {
        Ok(windows) => Ok(Some(QuotaReport::new(
            "google_gemini",
            "provider_api",
            windows,
            DEFAULT_FRESHNESS,
        ))),
        Err(_) => Ok(None),
    }
}

fn fetch_report(token: &str, timeout: Duration) -> Result<QuotaReport, String> {
    let response = post_json(
        JsonRequest {
            raw_auth_token: None,
            browser_cookie: None,
            url: RETRIEVE_QUOTA,
            bearer_token: Some(token),
            timeout,
            capture_headers: &[],
            extra_headers: &[],
        },
        &serde_json::json!({}),
    )?;

    let (windows, _) = windows_from_payload(&response.body);
    if windows.is_empty() {
        return Err("QUOTA_NOT_PUBLISHED".to_string());
    }
    Ok(QuotaReport::new(
        "google_gemini",
        "provider_api",
        windows,
        DEFAULT_FRESHNESS,
    ))
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
            fetch_windows(
                &dir.path().join("absent.json"),
                Duration::from_millis(200),
                false
            ),
            Ok(None)
        );
    }

    #[test]
    fn parses_the_antigravity_keyring_blob() {
        let blob = r#"{"token":{"access_token":"ya29.keyring-token","token_type":"Bearer","refresh_token":"1//keyring-refresh","expiry":"2026-08-09T23:18:26.3901515+07:00"},"auth_method":"consumer"}"#;
        let oauth = keyring_oauth(blob).expect("keyring blob parses");
        assert_eq!(oauth.access_token, "ya29.keyring-token");
        assert_eq!(oauth.refresh_token.as_deref(), Some("1//keyring-refresh"));
        assert!(oauth.expires_at_ms.is_some());
    }

    #[test]
    fn keyring_blob_without_refresh_still_yields_the_token() {
        let blob = r#"{"token":{"access_token":"ya29.only-access"}}"#;
        let oauth = keyring_oauth(blob).expect("token present");
        assert_eq!(oauth.access_token, "ya29.only-access");
        assert_eq!(oauth.refresh_token, None);
    }

    #[test]
    fn unparseable_or_blank_keyring_blobs_yield_nothing() {
        assert_eq!(keyring_oauth("not json"), None);
        assert_eq!(keyring_oauth(r#"{"token":{}}"#), None);
        assert_eq!(keyring_oauth(r#"{"token":{"access_token":""}}"#), None);
    }

    #[test]
    fn keyring_credentials_take_precedence_over_the_creds_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("oauth_creds.json");
        std::fs::write(&path, r#"{"access_token":"stale-file-token"}"#).expect("write creds");
        let blob = r#"{"token":{"access_token":"fresh-keyring-token","refresh_token":"1//r"}}"#;

        let oauth = read_oauth_with(&path, Some(blob)).expect("oauth");
        assert_eq!(
            oauth.access_token, "fresh-keyring-token",
            "the keyring holds the token the current Antigravity CLI refreshed"
        );
        assert_eq!(oauth.refresh_token.as_deref(), Some("1//r"));
    }

    #[test]
    fn creds_file_is_the_fallback_when_no_keyring_blob_is_stored() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("oauth_creds.json");
        std::fs::write(
            &path,
            r#"{"access_token":"file-token","refresh_token":"1//f"}"#,
        )
        .expect("write creds");

        let oauth = read_oauth_with(&path, None).expect("oauth");
        assert_eq!(oauth.access_token, "file-token");
    }

    #[test]
    fn expired_tokens_are_refreshed_instead_of_used_directly() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("oauth_creds.json");
        std::fs::write(
            &path,
            r#"{"access_token":"expired-token","refresh_token":"1//r","expiry_date":1}"#,
        )
        .expect("write creds");

        let oauth = read_oauth(&path).expect("oauth");
        assert!(is_expired(&oauth), "an epoch-1 expiry must be expired");
        assert_eq!(
            oauth.refresh_token.as_deref(),
            Some("1//r"),
            "the refresh path must prefer the stored refresh token"
        );
    }

    #[test]
    fn oauth_clients_are_extracted_from_the_bundled_ide_module() {
        // Shape of the minified oauthClient.js module embedded in the IDE's
        // out/main.js: client id and secret are consecutive string
        // assignments. Synthetic values only.
        let main_js = r#"{"out-build/vs/platform/cloudCode/common/oauthClient.js"(){"use strict";A1e="111122223333-aaaaaaaabbbbccccdddd.apps.googleusercontent.com",B1e="GOCSPX-fake-secret-value-123",C1e="444455556666-eeeeffffgggghhhhiiii.apps.googleusercontent.com",D1e="GOCSPX-other-fake-secret-456",MPa=["https://www.googleapis.com/auth/cloud-platform"]}}"#;
        let clients = extract_oauth_clients(main_js);
        assert_eq!(clients.len(), 2);
        assert_eq!(
            clients[0].client_id,
            "111122223333-aaaaaaaabbbbccccdddd.apps.googleusercontent.com"
        );
        assert_eq!(clients[0].client_secret, "GOCSPX-fake-secret-value-123");
        assert_eq!(
            clients[1].client_id,
            "444455556666-eeeeffffgggghhhhiiii.apps.googleusercontent.com"
        );
        assert_eq!(clients[1].client_secret, "GOCSPX-other-fake-secret-456");
    }

    #[test]
    fn oauth_clients_require_a_real_ide_module_shape() {
        assert!(extract_oauth_clients("").is_empty());
        assert!(
            extract_oauth_clients("some unrelated bundle without the module").is_empty(),
            "the oauthClient.js module marker is required"
        );
        let module_only =
            r#"{"oauthClient.js"(){"use strict";A1e="not-a-client-id",B1e="not-a-secret"}}"#;
        assert!(
            extract_oauth_clients(module_only).is_empty(),
            "values that do not match the client id/secret shape are ignored"
        );
    }

    #[test]
    fn client_id_and_secret_shapes_are_strict() {
        assert!(is_client_id(
            "111122223333-aaaaaaaabbbbccccdddd.apps.googleusercontent.com"
        ));
        assert!(!is_client_id("apps.googleusercontent.com"));
        assert!(!is_client_id("id.apps.googleusercontent.com"));
        assert!(!is_client_id("abc-123.apps.googleusercontent.com"));
        assert!(is_client_secret("GOCSPX-fake-secret-value-123"));
        assert!(!is_client_secret("GOCSPX-short"));
        assert!(!is_client_secret("not-a-secret"));
    }

    #[test]
    fn refresh_without_a_bundled_client_reports_source_unavailable() {
        // No Antigravity IDE on this machine: refresh must fail cleanly
        // instead of guessing a client.
        if !antigravity_main_js_path().is_some() {
            assert_eq!(
                refresh_access_token("1//r", Duration::from_millis(200)),
                Err("SOURCE_UNAVAILABLE".to_string())
            );
        }
    }

    #[test]
    fn blank_refresh_tokens_are_rejected_before_any_client_lookup() {
        assert_eq!(
            refresh_access_token("  ", Duration::from_millis(200)),
            Err("AUTH_EXPIRED".to_string())
        );
    }

    #[test]
    fn new_tier_response_is_parsed_from_allowed_tiers() {
        let body = serde_json::json!({
            "allowedTiers": [
                { "id": "standard-tier", "name": "Gemini Code Assist" }
            ],
            "ineligibleTiers": [
                { "reasonCode": "UNSUPPORTED_CLIENT", "tierId": "free-tier" }
            ]
        });
        assert_eq!(tier_from_assist(&body).as_deref(), Some("standard-tier"));
        assert_eq!(plan_label("standard-tier"), Some("Paid"));
    }

    #[test]
    fn legacy_tier_response_is_still_parsed() {
        let body = serde_json::json!({ "currentTier": { "id": "free-tier" } });
        assert_eq!(tier_from_assist(&body).as_deref(), Some("free-tier"));
        assert_eq!(plan_label("free-tier"), Some("Free"));
    }

    /// Live verification that the bundled OAuth clients are read from the
    /// installed Antigravity IDE. Requires the IDE to be installed on this
    /// machine; run with `--ignored`.
    #[test]
    #[ignore]
    fn live_oauth_clients_come_from_the_installed_ide() {
        let clients = load_oauth_clients();
        assert!(
            !clients.is_empty(),
            "the installed Antigravity IDE must expose its OAuth clients"
        );
        for client in &clients {
            eprintln!(
                "oauth client: id={} secret=<{} chars>",
                client.client_id,
                client.client_secret.len()
            );
        }
    }

    /// Live verification against the real Windows Credential Manager entry.
    /// Run with `cargo test -- --ignored live_keyring_quota`; requires the
    /// Antigravity CLI to be logged in on this machine.
    #[test]
    #[ignore]
    fn live_keyring_quota_fetch() {
        let home = std::env::var_os("USERPROFILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let auth_path = home.join(".gemini").join("oauth_creds.json");
        let report = fetch_windows(&auth_path, Duration::from_secs(20), true)
            .expect("quota fetch must not error");
        let report = report.expect("credentials must be present when Antigravity is logged in");
        println!("plan={:?}", report.plan);
        for window in &report.windows {
            println!(
                "window key={} label={} used={:?} reset={:?}",
                window.window_key, window.label, window.used_percent, window.reset_at
            );
        }
        assert!(
            !report.windows.is_empty(),
            "at least one quota window must be published"
        );
    }
}
