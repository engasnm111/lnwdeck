use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// Payload emitted to the frontend when a new update is available.
#[derive(Clone, Serialize)]
pub struct UpdateAvailablePayload {
    pub version: String,
    pub body: String,
}

/// Spawn a background task that checks for updates after a short startup delay.
/// If an update is available, it emits an `update-available` event to the
/// frontend so the UI can show a notification banner.
pub fn spawn_update_check(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Wait 10 seconds after startup before checking
        std::thread::sleep(std::time::Duration::from_secs(10));

        let handle = app.clone();
        tauri::async_runtime::block_on(async move {
            let updater = match handle.updater() {
                Ok(updater) => updater,
                Err(_e) => return,
            };
            match updater.check().await {
                Ok(Some(update)) => {
                    let version = update.version.clone();
                    let body = update.body.clone().unwrap_or_default();
                    let _ =
                        handle.emit("update-available", UpdateAvailablePayload { version, body });
                }
                Ok(None) => {
                    // Already on the latest version — nothing to do
                }
                Err(_e) => {
                    // Network error or endpoint unreachable — silently skip
                    // Never surface update errors to the user on startup
                }
            }
        });
    });
}

/// Tauri command: manually check for updates and install if available.
/// Returns a human-readable status message.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<String, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Update check failed: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {e}"))?;

    match update {
        Some(update) => {
            let version = update.version.clone();

            // Download and install
            let mut downloaded: u64 = 0;
            update
                .download_and_install(
                    |chunk_length, content_length| {
                        downloaded += chunk_length as u64;
                        if let Some(total) = content_length {
                            let _ = app.emit(
                                "update-progress",
                                serde_json::json!({
                                    "downloaded": downloaded,
                                    "total": total,
                                }),
                            );
                        }
                    },
                    || {
                        // Download finished callback
                    },
                )
                .await
                .map_err(|e| format!("Update install failed: {e}"))?;

            Ok(format!(
                "Update v{version} downloaded. Restart the app to apply."
            ))
        }
        None => Ok("You are already on the latest version.".to_string()),
    }
}
