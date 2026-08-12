mod commands;
mod pets;
mod state;
mod tray;
mod updater;
mod windows;

use crate::state::AppState;
use lnwdeck_storage::repositories::{AppEventLevel, AppEventRepository};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

/// Delay before the first background refresh, so startup stays responsive.
const FIRST_REFRESH_DELAY: Duration = Duration::from_secs(15);
/// How often the loop re-reads the interval while refreshing is disabled.
const DISABLED_POLL_INTERVAL: Duration = Duration::from_secs(60);
/// A background cycle is skipped when a refresh completed inside this window.
/// Manual refreshes reset the cooldown, so the loop never piles a background
/// cycle on top of a button press. With the shortest allowed interval (5 min)
/// and a ~10s fast cycle this makes the effective background spacing ~10 min,
/// which is the point of the setting. Chrono duration on purpose: it is
/// compared against `signed_duration_since`.
const BACKGROUND_REFRESH_COOLDOWN: chrono::Duration = chrono::Duration::minutes(5);

#[cfg(windows)]
const SINGLE_INSTANCE_NAME: &str = "Local\\lnwdeck.app.lnwdeck.desktop";

#[cfg(windows)]
struct SingleInstanceGuard(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // The mutex handle is owned by this process for the whole Tauri run.
        // Closing it releases the named mutex when the process exits.
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
struct SingleInstanceGuard;

/// Acquires the process-wide named mutex. A second launch exits before it can
/// create windows, tray state, or a second refresh loop.
fn acquire_single_instance() -> Option<SingleInstanceGuard> {
    #[cfg(windows)]
    {
        use std::ptr::null;
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;

        let name: Vec<u16> = SINGLE_INSTANCE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: the name is a nul-terminated UTF-16 string owned for the
        // duration of the call; the returned handle is closed by the guard.
        let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return None;
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
            }
            return None;
        }
        Some(SingleInstanceGuard(handle))
    }

    #[cfg(not(windows))]
    {
        Some(SingleInstanceGuard)
    }
}

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

/// Last successful full refresh, local time; `None` when never synced.
pub fn last_sync_time(app: &tauri::AppHandle) -> Option<chrono::DateTime<chrono::Local>> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage().ok()?;
    let storage = guard.as_ref()?;
    let stored = lnwdeck_storage::repositories::AppSettingsRepository::new(&storage.conn)
        .get("last_sync_time")
        .ok()
        .flatten()?;
    chrono::DateTime::parse_from_rfc3339(&stored)
        .ok()
        .map(|time| time.with_timezone(&chrono::Local))
}

/// Marks a successful full refresh at the current time.
pub fn record_sync_time(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let Ok(guard) = state.ensure_storage() else {
        return;
    };
    let Some(storage) = guard.as_ref() else {
        return;
    };
    let now = chrono::Utc::now().to_rfc3339();
    let _ = lnwdeck_storage::repositories::AppSettingsRepository::new(&storage.conn)
        .set("last_sync_time", &now);
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

/// True when a background cycle should be skipped because a refresh already
/// completed recently. `last_sync_time` is only written by successful cycles,
/// so a failed cycle never suppresses the next retry.
fn should_skip_background_refresh(
    last_sync: Option<chrono::DateTime<chrono::Local>>,
    now: chrono::DateTime<chrono::Local>,
) -> bool {
    last_sync.is_some_and(|last| now.signed_duration_since(last) < BACKGROUND_REFRESH_COOLDOWN)
}

/// One refresh cycle plus alert evaluation, with every failure recorded.
/// Skips the cycle when a manual refresh just completed (cooldown) or when a
/// manual refresh is already running.
fn run_refresh_cycle(app: &tauri::AppHandle) {
    if should_skip_background_refresh(last_sync_time(app), chrono::Local::now()) {
        return;
    }
    if let Err(error) = commands::pipeline::start_refresh(app.clone()) {
        record_event(
            app,
            "refresh_loop",
            AppEventLevel::Warning,
            "REFRESH_START_FAILED",
            &error,
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let Some(_single_instance_guard) = acquire_single_instance() else {
        return;
    };
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
            // Install the bundled default pets from codex-pets.net exactly once,
            // then point the pet at the first one unless the user already chose.
            {
                let state = app.state::<AppState>();
                let guard = state.ensure_storage();
                // Decide everything while the guard is held, but emit the
                // settings change only after it is dropped: emitting reads
                // settings again and would deadlock on the storage mutex.
                let mut changed_character = false;
                if let Ok(guard) = guard {
                    if let Some(storage) = guard.as_ref() {
                        let store = commands::pets::pet_store_dir(app.handle());
                        match pets::seed_default_pets(&store, &storage.conn) {
                            Ok(installed) if !installed.is_empty() => {
                                let settings = lnwdeck_application::settings::SettingsService::load(
                                    &storage.conn,
                                );
                                if settings.is_ok_and(|s| s.pet_character == "robot") {
                                    let _ = lnwdeck_application::settings::SettingsService::
                                        set_pet_character(&storage.conn, &installed[0]);
                                    changed_character = true;
                                }
                            }
                            Ok(_) => {}
                            Err(error) => {
                                record_event(
                                    &handle,
                                    "pet_seed",
                                    AppEventLevel::Warning,
                                    "PET_SEED_FAILED",
                                    &error,
                                );
                            }
                        }
                    }
                }
                if changed_character {
                    windows::emit_pet_window_settings(app.handle());
                }
            }
            windows::restore_widget_state(app);
            windows::restore_pet_window_state(app);
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
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                windows::handle_close_request(window, api);
            }
            if let tauri::WindowEvent::Focused(false) = event {
                if window.label() == windows::TRAY_LABEL {
                    let _ = window.hide();
                }
            }
            if let tauri::WindowEvent::Moved { .. } = event {
                if window.label() == "widget" {
                    windows::save_widget_position(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview::get_overview,
            commands::analytics::get_analytics,
            commands::dashboard::get_usage_dashboard,
            commands::providers::get_providers,
            commands::pipeline::refresh_all,
            commands::pipeline::start_refresh,
            commands::pipeline::cancel_refresh,
            commands::pipeline::refresh_provider,
            commands::pipeline::get_pipeline_diagnostics,
            commands::quota::get_quota_dashboard,
            commands::pages::get_usage_history,
            commands::pages::get_costs,
            commands::sessions::get_sessions,
            commands::sessions::rename_session,
            commands::sessions::rename_project,
            commands::pages::get_budgets,
            commands::pages::save_budget,
            commands::pages::delete_budget,
            commands::pages::get_alerts,
            commands::pages::acknowledge_alert,
            commands::pages::acknowledge_all_alerts,
            commands::pages::get_settings,
            commands::pages::save_settings,
            commands::pages::set_provider_key,
            commands::pages::delete_provider_key,
            commands::pages::set_opencode_go_config,
            commands::pages::delete_opencode_go_config,
            commands::pages::get_app_events,
            windows::show_widget,
            windows::hide_widget,
            windows::get_widget_settings,
            windows::set_widget_opacity,
            windows::set_widget_locked,
            windows::set_widget_providers,
            windows::set_widget_view,
            windows::set_widget_size_preset,
            windows::show_main_window,
            windows::open_dashboard_from_tray,
            windows::hide_tray_popup,
            commands::pets::import_widget_pet,
            commands::pets::import_widget_pet_file,
            commands::pets::list_widget_pets,
            commands::pets::get_widget_pet,
            commands::pets::set_widget_pet,
            commands::pets::remove_widget_pet,
            windows::get_pet_window_settings,
            windows::show_pet_window,
            windows::hide_pet_window,
            windows::move_pet_window,
            windows::read_pet_spritesheet,
            windows::set_pet_character,
            windows::set_pet_speed,
            windows::set_pet_opacity,
            windows::set_pet_auto_sleep,
            windows::set_pet_size_preset,
            windows::set_pet_stay_in_place,
            windows::set_pet_pose,
            windows::set_pet_hit_rect,
            windows::apply_pet_click_through,
            windows::set_language,
            updater::check_for_update,
            updater::install_update,
            commands::pipeline::export_diagnostics,
            commands::pipeline::reveal_in_explorer,
        ])
        // Serves installed pet assets to the widget over petlocal:// so the
        // webview never loads a remote asset.
        .register_uri_scheme_protocol("petlocal", |ctx, request| {
            let store = commands::pets::pet_store_dir(ctx.app_handle());
            let path = request.uri().path().to_string();
            eprintln!("[petlocal] request: {path}");
            commands::pets::serve_pet_asset(&store, &path)
        })
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
    fn background_cycle_skips_when_a_refresh_completed_recently() {
        let now = chrono::Local::now();
        assert!(
            should_skip_background_refresh(Some(now - chrono::Duration::minutes(1)), now),
            "a refresh one minute ago suppresses the background cycle"
        );
        assert!(
            !should_skip_background_refresh(Some(now - chrono::Duration::minutes(20)), now),
            "a refresh twenty minutes ago does not suppress the background cycle"
        );
        assert!(
            !should_skip_background_refresh(None, now),
            "never-synced installs still run the first background cycle"
        );
    }

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
        assert!(
            BACKGROUND_REFRESH_COOLDOWN.num_seconds() > 0,
            "the cooldown must actually suppress a recent background cycle"
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
