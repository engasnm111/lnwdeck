//! Auto-update wiring.
//!
//! The check and the install are separate commands: a check never downloads
//! anything, and an install reports real progress and then restarts the
//! application. Signature verification is performed by
//! `tauri-plugin-updater` against the public key in `tauri.conf.json`; this
//! module deliberately contains no verification logic of its own, because a
//! hand-written check that always succeeds is worse than none.
//!
//! Failures are never swallowed. A background check that fails emits
//! `update-check-failed` and is recorded in `app_events`, so an unreachable
//! endpoint is distinguishable from "you are up to date".

use crate::state::AppState;
use lnwdeck_storage::repositories::{AppEventLevel, AppEventRepository};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Result of an update check.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheck {
    pub available: bool,
    pub current_version: String,
    /// Version offered by the endpoint, when one is available.
    pub version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

/// Payload emitted while an update downloads.
#[derive(Debug, Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Payload emitted when a check fails.
#[derive(Debug, Clone, Serialize)]
struct UpdateCheckFailed {
    code: String,
}

/// Payload emitted when the running version is already the newest one.
#[derive(Debug, Clone, Serialize)]
struct UpdateUpToDate {
    version: String,
}

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Records a background failure so it is visible on the System page.
fn record_event(app: &tauri::AppHandle, level: AppEventLevel, code: &str, detail: &str) {
    let state = app.state::<AppState>();
    let Ok(guard) = state.ensure_storage() else {
        return;
    };
    let Some(storage) = guard.as_ref() else {
        return;
    };
    let _ = AppEventRepository::new(&storage.conn).record("updater", level, code, detail);
}

/// Reduces an updater error to a stable code. The message may contain a URL,
/// so it is never propagated to storage or to the UI verbatim.
fn error_code(error: &tauri_plugin_updater::Error) -> &'static str {
    match error {
        tauri_plugin_updater::Error::Network(_) => "UPDATE_ENDPOINT_UNREACHABLE",
        tauri_plugin_updater::Error::Io(_) => "UPDATE_IO_ERROR",
        tauri_plugin_updater::Error::Serialization(_) | tauri_plugin_updater::Error::Semver(_) => {
            "UPDATE_MANIFEST_INVALID"
        }
        tauri_plugin_updater::Error::SignatureUtf8(_)
        | tauri_plugin_updater::Error::Minisign(_)
        | tauri_plugin_updater::Error::Base64(_) => "UPDATE_SIGNATURE_INVALID",
        _ => "UPDATE_FAILED",
    }
}

/// Builds an updater, honouring `LNWDECK_UPDATE_ENDPOINT` when set.
///
/// The override exists so a release can be verified end to end against a local
/// endpoint before it is published; the signature requirement is unchanged.
fn updater(app: &tauri::AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app.updater_builder();
    // Release assets are served through the GitHub API (api.github.com), which
    // only redirects to the binary blob when the client accepts
    // application/octet-stream; without it the API returns JSON metadata and
    // the download fails. The updater plugin reuses these headers for both the
    // check and the download, and every endpoint listed in tauri.conf.json
    // serves the manifest as a plain file, so a fixed Accept header is safe.
    builder = builder
        .header("Accept", "application/octet-stream")
        .map_err(|error| format!("update header rejected: {}", error_code(&error)))?;
    if let Ok(endpoint) = std::env::var("LNWDECK_UPDATE_ENDPOINT") {
        let parsed = endpoint
            .parse()
            .map_err(|_| format!("invalid LNWDECK_UPDATE_ENDPOINT: {endpoint}"))?;
        builder = builder
            .endpoints(vec![parsed])
            .map_err(|error| format!("update endpoint rejected: {}", error_code(&error)))?;
    }
    builder
        .build()
        .map_err(|error| format!("updater unavailable: {}", error_code(&error)))
}

/// Background check shortly after startup.
///
/// Runs only when the user has left automatic checks enabled.
pub fn spawn_update_check(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));

        if !auto_check_enabled(&app) {
            return;
        }

        let handle = app.clone();
        tauri::async_runtime::block_on(async move {
            match check(&handle).await {
                Ok(result) => {
                    if result.available {
                        let _ = handle.emit("update-available", result);
                    }
                }
                Err(code) => {
                    record_event(
                        &handle,
                        AppEventLevel::Warning,
                        &code,
                        "the automatic update check did not complete",
                    );
                    let _ = handle.emit("update-check-failed", UpdateCheckFailed { code });
                }
            }
        });
    });
}

/// Whether automatic update checks are enabled in settings.
fn auto_check_enabled(app: &tauri::AppHandle) -> bool {
    let state = app.state::<AppState>();
    let Ok(guard) = state.ensure_storage() else {
        // Without storage the documented default applies.
        return true;
    };
    let Some(storage) = guard.as_ref() else {
        return true;
    };
    lnwdeck_application::settings::SettingsService::load(&storage.conn)
        .map(|settings| settings.auto_update_check)
        .unwrap_or(true)
}

/// Performs a check without downloading anything.
async fn check(app: &tauri::AppHandle) -> Result<UpdateCheck, String> {
    let updater = updater(app)?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateCheck {
            available: true,
            current_version: current_version(),
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            published_at: update.date.map(|date| date.to_string()),
        }),
        Ok(None) => Ok(UpdateCheck {
            available: false,
            current_version: current_version(),
            version: None,
            notes: None,
            published_at: None,
        }),
        Err(error) => Err(error_code(&error).to_string()),
    }
}

/// Checks for an update. Never downloads or installs.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateCheck, String> {
    check(&app).await
}

/// Silently checks for and installs the newest version, then restarts.
///
/// Used by the tray "Check for updates" action: when an update is available it
/// is downloaded, signature-verified and installed without further prompts.
/// Failures are recorded in `app_events` and emitted so the UI can surface
/// them; nothing is ever reported as success unless the installer ran.
pub fn check_and_install_silent(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let result = tauri::async_runtime::block_on(async {
            let updater = updater(&app)?;
            let update = match updater.check().await {
                Ok(Some(update)) => update,
                Ok(None) => {
                    record_event(
                        &app,
                        AppEventLevel::Info,
                        "UPDATE_CHECKED",
                        "no update is available",
                    );
                    // "Already up to date" is a normal result, reported to the
                    // tray popup as its own event, not as a failed check.
                    let _ = app.emit(
                        "update-up-to-date",
                        UpdateUpToDate {
                            version: current_version(),
                        },
                    );
                    return Ok(());
                }
                Err(error) => return Err(error_code(&error).to_string()),
            };
            let version = update.version.clone();
            let progress_app = app.clone();
            let mut downloaded: u64 = 0;
            update
                .download_and_install(
                    move |chunk_length, content_length| {
                        downloaded += chunk_length as u64;
                        let _ = progress_app.emit(
                            "update-progress",
                            UpdateProgress {
                                downloaded,
                                total: content_length,
                            },
                        );
                    },
                    || {},
                )
                .await
                .map_err(|error| error_code(&error).to_string())?;
            record_event(
                &app,
                AppEventLevel::Info,
                "UPDATE_INSTALLED",
                &format!("installed version {version}"),
            );
            app.restart();
        });
        // `app.restart()` never returns on success; only failures reach here.
        if let Err(code) = result {
            record_event(
                &app,
                AppEventLevel::Error,
                &code,
                "the silent update did not complete",
            );
            let _ = app.emit("update-check-failed", UpdateCheckFailed { code });
        }
    });
}

/// Downloads and installs the available update, then restarts the app.
///
/// Progress is emitted as `update-progress`. The signature is verified by the
/// updater plugin before the installer runs; a bad signature fails here and is
/// reported, never ignored.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    let updater = updater(&app)?;
    let update = updater
        .check()
        .await
        .map_err(|error| error_code(&error).to_string())?
        .ok_or_else(|| "no update is available".to_string())?;

    let version = update.version.clone();
    let progress_app = app.clone();
    let mut downloaded: u64 = 0;

    let result = update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                let _ = progress_app.emit(
                    "update-progress",
                    UpdateProgress {
                        downloaded,
                        total: content_length,
                    },
                );
            },
            || {},
        )
        .await;

    match result {
        Ok(()) => {
            record_event(
                &app,
                AppEventLevel::Info,
                "UPDATE_INSTALLED",
                &format!("installed version {version}"),
            );
            // The installer has run; restart into the new build.
            app.restart();
        }
        Err(error) => {
            let code = error_code(&error).to_string();
            record_event(
                &app,
                AppEventLevel::Error,
                &code,
                &format!("installing version {version} failed"),
            );
            Err(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_matches_the_crate_version() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
        assert!(
            current_version().split('.').count() >= 3,
            "the version must be a semantic version: {}",
            current_version()
        );
    }

    #[test]
    fn error_codes_are_stable_and_carry_no_url() {
        let network = tauri_plugin_updater::Error::Network(
            "https://example.com/latest.json refused".to_string(),
        );
        let code = error_code(&network);
        assert_eq!(code, "UPDATE_ENDPOINT_UNREACHABLE");
        assert!(!code.contains("http"), "codes must not leak the endpoint");

        let signature =
            tauri_plugin_updater::Error::SignatureUtf8("not valid utf8 signature".to_string());
        assert_eq!(error_code(&signature), "UPDATE_SIGNATURE_INVALID");

        let manifest = tauri_plugin_updater::Error::InvalidUpdaterFormat;
        assert_eq!(error_code(&manifest), "UPDATE_FAILED");
    }
}
