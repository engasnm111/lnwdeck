//! Gemini subscription quota, read from the running Antigravity IDE
//! Language Server.
//!
//! The Antigravity IDE keeps a local gRPC server (its Language Server) that
//! holds the user's Google session and answers the same per-model quota
//! windows the IDE shows in Settings → Models. lnwdeck discovers that process
//! and fetches the windows over loopback; nothing is sent outside the machine.
//!
//! The IDE is deliberately the *only* quota source. Google's public
//! `retrieveUserQuota` endpoint returns placeholder fractions for accounts it
//! does not track in detail (observed as a flat "100% remaining" on real
//! machines), so a fallback would fabricate percentages instead of telling the
//! truth. When the IDE is closed the fetch fails with
//! `SOURCE_REQUIRES_IDE`, which the UI presents as "open Antigravity IDE to
//! update quota" instead of stale numbers.
//!
//! The Gemini CLI stores a legacy OAuth payload in `~/.gemini/oauth_creds.json`,
//! while the current Antigravity CLI keeps its token in the OS credential
//! manager under `gemini:antigravity`. Both are read only to derive the
//! account identity for fingerprinting; the raw values never leave the
//! machine and are never logged or serialized.

use lnwdeck_domain::{QuotaReport, DEFAULT_FRESHNESS};
use lnwdeck_windows_integration::antigravity_ls::{
    discover as discover_ls, LanguageServer, LsDiscoveryError,
};
use std::path::Path;
use std::time::Duration;

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

/// Reads the OAuth identity of the credential that would actually serve
/// quota on this machine: the Antigravity keyring blob when present (and
/// allowed), the legacy CLI creds file otherwise.
pub fn read_oauth_preferring_keyring(auth_path: &Path, use_keyring: bool) -> Option<GeminiOAuth> {
    if use_keyring {
        if let Some(oauth) = read_oauth_from_keyring() {
            return Some(oauth);
        }
    }
    read_oauth(auth_path)
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

/// Fetches the Gemini quota windows from the running Antigravity IDE
/// Language Server.
///
/// `Err("NOT_INSTALLED")` means the IDE is not installed on this machine;
/// `Err("SOURCE_REQUIRES_IDE")` means it is installed but not running, so no
/// quota can be read until the user opens it; other `LsDiscoveryError` states
/// surface as `SOURCE_UNAVAILABLE`. No token, credential or request is needed
/// for the Language Server call itself, so nothing is fetched when the IDE is
/// closed.
pub fn fetch_windows(timeout: Duration) -> Result<QuotaReport, String> {
    fetch_windows_with(timeout, antigravity_main_js_path().is_some(), discover_ls)
}

/// `fetch_windows` with the two environment probes injected, so tests can
/// exercise every state without a real IDE.
fn fetch_windows_with(
    timeout: Duration,
    ide_installed: bool,
    discover: impl Fn() -> Result<LanguageServer, LsDiscoveryError>,
) -> Result<QuotaReport, String> {
    if !ide_installed {
        return Err("NOT_INSTALLED".to_string());
    }
    let ls = match discover() {
        Ok(ls) => ls,
        Err(LsDiscoveryError::NotRunning) => {
            return Err("SOURCE_REQUIRES_IDE".to_string());
        }
        Err(_) => return Err("SOURCE_UNAVAILABLE".to_string()),
    };
    // The LS is a localhost process, so a short per-port budget is plenty.
    // Each port probed with the full remote timeout would stack (three ports
    // at 15s each = 45s) and stall the whole refresh cycle.
    let ls_timeout = timeout.min(Duration::from_secs(3));
    let windows = super::ls_quota::fetch_ls_windows(&ls.ports, &ls.csrf_token, ls_timeout)?;
    if windows.is_empty() {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    Ok(QuotaReport::new(
        "google_gemini",
        "antigravity_ls",
        windows,
        DEFAULT_FRESHNESS,
    ))
}

/// Default timeout for one quota fetch.
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
    fn no_ide_installed_means_quota_is_not_available() {
        let result = fetch_windows_with(Duration::from_millis(100), false, || {
            Err(LsDiscoveryError::NotRunning)
        });
        assert_eq!(
            result,
            Err("NOT_INSTALLED".to_string()),
            "a machine without the IDE must not fall back to guessed quota"
        );
    }

    #[test]
    fn closed_ide_is_reported_as_requiring_the_ide() {
        let result = fetch_windows_with(Duration::from_millis(100), true, || {
            Err(LsDiscoveryError::NotRunning)
        });
        assert_eq!(
            result,
            Err("SOURCE_REQUIRES_IDE".to_string()),
            "the UI maps this code to 'open Antigravity IDE' guidance"
        );
    }

    #[test]
    fn broken_discovery_is_a_source_unavailable_state() {
        for error in [
            LsDiscoveryError::UnreadableCommandLine,
            LsDiscoveryError::MissingCsrfToken,
            LsDiscoveryError::NoHttpListener,
        ] {
            let result =
                fetch_windows_with(Duration::from_millis(100), true, || Err(error.clone()));
            assert_eq!(result, Err("SOURCE_UNAVAILABLE".to_string()));
        }
    }

    #[test]
    fn a_reachable_ls_yields_a_report_without_any_token() {
        let ls = LanguageServer {
            pid: 4242,
            ports: vec![9_999],
            csrf_token: "csrf-for-tests".to_string(),
        };
        // No LS actually listens on the fake port, so the fetch must fail
        // with a sanitized code instead of hanging or panicking; the point
        // is that no stored OAuth credential is required to attempt it.
        let result = fetch_windows_with(Duration::from_millis(300), true, || Ok(ls.clone()));
        assert!(
            result.is_err(),
            "the probe against a closed port must fail cleanly, not fabricate data"
        );
        let code = result.expect_err("code");
        assert!(
            !code.is_empty() && !code.contains("csrf") && !code.contains("9999"),
            "the code is sanitized: {code}"
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
    fn preferring_keyring_falls_back_to_the_file_without_a_keyring_entry() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("oauth_creds.json");
        std::fs::write(&path, r#"{"access_token":"file-token"}"#).expect("write creds");

        // When the keyring is disallowed the file is always the identity.
        assert_eq!(
            read_oauth_preferring_keyring(&path, false).map(|oauth| oauth.access_token),
            Some("file-token".to_string())
        );
        // With the keyring allowed, either the Credential Manager blob or the
        // file is returned; the precedence itself is covered by
        // `keyring_credentials_take_precedence_over_the_creds_file`.
        let identity = read_oauth_preferring_keyring(&path, true).expect("identity");
        assert!(
            !identity.access_token.is_empty(),
            "the credential that serves quota must be identifiable"
        );
    }

    /// Live verification against the installed IDE and the real Windows
    /// Credential Manager entry. Requires the Antigravity IDE to be open and
    /// logged in; run with `cargo test -- --ignored live_ls_quota`.
    #[test]
    #[ignore]
    fn live_ls_quota_fetch() {
        let report = fetch_windows(Duration::from_secs(20)).expect("quota fetch must not error");
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
