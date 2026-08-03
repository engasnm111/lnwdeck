mod commands;
mod state;
mod tray;
mod updater;
mod windows;

use lnwdeck_storage::repositories::{AppEventLevel, AppEventRepository};
use state::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{Emitter, Manager};

/// Delay before the first background refresh, so startup stays responsive.
const FIRST_REFRESH_DELAY: Duration = Duration::from_secs(15);
/// How often the loop re-reads the interval while refreshing is disabled.
const DISABLED_POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Records a background event so a failure outside a user action is visible on
/// the System page instead of disappearing.
fn record_event(
    app: &tauri::AppHandle,
    source: &str,
    level: AppEventLevel,
    code: &str,
    detail: &str,
) {
    let state = app.state::<AppState>();
    let Ok(guard) = state.ensure_storage() else {
        return;
    };
    let Some(storage) = guard.as_ref() else {
        return;
    };
    let _ = AppEventRepository::new(&storage.conn).record(source, level, code, detail);
}

/// Records a tray failure. Called from the tray menu handlers, which cannot
/// return an error to the user directly.
pub fn record_tray_event(code: &str, detail: &str, app: &tauri::AppHandle) {
    record_event(app, "tray", AppEventLevel::Warning, code, detail);
}

/// Refresh interval configured by the user. `None` disables the loop.
fn configured_interval(app: &tauri::AppHandle) -> Option<Duration> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage().ok()?;
    let storage = guard.as_ref()?;
    let settings = lnwdeck_application::settings::SettingsService::load(&storage.conn).ok()?;
    if settings.refresh_interval_seconds == 0 {
        None
    } else {
        Some(Duration::from_secs(settings.refresh_interval_seconds))
    }
}

/// Background collection loop.
///
/// The interval comes from the user's setting and is re-read every cycle, so
/// changing it in Settings takes effect without a restart. A failed cycle is
/// recorded in `app_events` and the loop continues; it is never silent.
fn spawn_refresh_loop(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_REFRESH_DELAY);
        loop {
            let interval = configured_interval(&app);
            match interval {
                None => {
                    // Refreshing is disabled; keep checking whether that changes.
                    std::thread::sleep(DISABLED_POLL_INTERVAL);
                    continue;
                }
                Some(interval) => {
                    run_refresh_cycle(&app);
                    std::thread::sleep(interval);
                }
            }
        }
    });
}

/// One refresh cycle plus alert evaluation, with every failure recorded.
fn run_refresh_cycle(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    match commands::pipeline::refresh_now(&state) {
        Ok(cycle) => {
            let failed: Vec<&str> = cycle
                .usage
                .iter()
                .filter(|outcome| !outcome.error_code.is_empty() && !outcome.is_not_supported())
                .map(|outcome| outcome.error_code.as_str())
                .collect();
            if !failed.is_empty() {
                record_event(
                    app,
                    "refresh_loop",
                    AppEventLevel::Warning,
                    "COLLECTION_INCOMPLETE",
                    &format!("{} collector(s) reported an error", failed.len()),
                );
            }
            let _ = app.emit("quota-updated", ());
            let _ = app.emit("usage-updated", ());
            if let Err(code) = commands::pipeline::evaluate_alerts_now(&state) {
                record_event(
                    app,
                    "refresh_loop",
                    AppEventLevel::Warning,
                    "ALERT_EVALUATION_FAILED",
                    &code,
                );
            }
        }
        Err(error) => {
            record_event(
                app,
                "refresh_loop",
                AppEventLevel::Error,
                "REFRESH_FAILED",
                &error,
            );
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = default_db_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::new(db_path))
        .setup(|app| {
            let handle = app.handle().clone();
            // Storage must be usable before any window renders; a failure here
            // is fatal and reported rather than hidden behind empty pages.
            let state = app.state::<AppState>();
            {
                let guard = state.ensure_storage().map_err(std::io::Error::other)?;
                if guard.as_ref().is_none() {
                    return Err(Box::new(std::io::Error::other(
                        "storage could not be initialized",
                    )));
                }
            }

            windows::setup_windows(app);
            windows::restore_widget_state(app);
            if let Err(error) = tray::setup_tray(app) {
                record_event(
                    &handle,
                    "tray",
                    AppEventLevel::Warning,
                    "TRAY_UNAVAILABLE",
                    &error.to_string(),
                );
            }
            spawn_refresh_loop(handle.clone());
            updater::spawn_update_check(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                windows::handle_close_request(window);
            }
            if let tauri::WindowEvent::Moved { .. } = event {
                if window.label() == "widget" {
                    windows::save_widget_position(window.app_handle());
                }
            }
            if let tauri::WindowEvent::Resized { .. } = event {
                if window.label() == "widget" {
                    windows::save_widget_size(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview::get_overview,
            commands::analytics::get_analytics,
            commands::providers::get_providers,
            commands::pipeline::refresh_all,
            commands::pipeline::refresh_provider,
            commands::pipeline::get_pipeline_diagnostics,
            commands::quota::get_quota_dashboard,
            commands::pages::get_usage_history,
            commands::pages::get_costs,
            commands::pages::get_budgets,
            commands::pages::save_budget,
            commands::pages::delete_budget,
            commands::pages::get_alerts,
            commands::pages::acknowledge_alert,
            commands::pages::get_settings,
            commands::pages::save_settings,
            commands::pages::set_provider_key,
            commands::pages::delete_provider_key,
            commands::pages::get_app_events,
            windows::show_widget,
            windows::hide_widget,
            windows::get_widget_settings,
            windows::set_widget_opacity,
            windows::set_widget_locked,
            windows::show_main_window,
            updater::check_for_update,
            updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn default_db_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_default();
    if !base.is_empty() {
        let dir = PathBuf::from(base).join("lnwdeck");
        std::fs::create_dir_all(&dir).ok();
        dir.join("lnwdeck.db")
    } else {
        PathBuf::from("lnwdeck.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_intervals_are_sane() {
        assert!(
            FIRST_REFRESH_DELAY.as_secs() > 0,
            "startup must not be blocked by an immediate refresh"
        );
        assert!(DISABLED_POLL_INTERVAL.as_secs() > 0);
        assert!(
            lnwdeck_application::settings::ALLOWED_REFRESH_INTERVALS.contains(&0),
            "the user must be able to disable background refreshing"
        );
    }

    #[test]
    fn default_db_path_lands_in_an_application_directory() {
        let path = default_db_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("lnwdeck.db")
        );
        if std::env::var("LOCALAPPDATA").is_ok() || std::env::var("APPDATA").is_ok() {
            assert!(
                path.parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    == Some("lnwdeck"),
                "the database must live in its own directory: {}",
                path.display()
            );
        }
    }
}
