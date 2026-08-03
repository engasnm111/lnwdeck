mod commands;
mod state;
mod tray;
mod windows;

use state::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

#[tauri::command]
fn greet() -> String {
    "Hello from lnwdeck!".to_string()
}

/// Background collection loop: refresh once shortly after startup, then on
/// the adaptive interval. Failures are recorded per provider and never
/// terminate the loop.
fn spawn_refresh_loop(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        let state = app.state::<AppState>();
        let _ = commands::pipeline::refresh_now(&state);
        std::thread::sleep(Duration::from_secs(285));
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = default_db_path();

    tauri::Builder::default()
        .manage(AppState::new(db_path))
        .setup(|app| {
            windows::setup_windows(app);
            tray::setup_tray(app).ok();
            spawn_refresh_loop(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                windows::handle_close_request(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::overview::get_overview,
            commands::analytics::get_analytics,
            commands::providers::get_providers,
            commands::pipeline::refresh_all,
            commands::pipeline::get_pipeline_diagnostics,
            windows::show_widget,
            windows::hide_widget,
            windows::set_widget_opacity,
            windows::show_main_window,
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
    fn greet_returns_lnwdeck_message() {
        let result = greet();
        assert!(result.contains("lnwdeck"));
    }
}
