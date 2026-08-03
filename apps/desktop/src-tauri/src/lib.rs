mod commands;
mod state;
mod tray;
mod windows;

use state::AppState;
use std::path::PathBuf;

#[tauri::command]
fn greet() -> String {
    "Hello from inwdeck!".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = default_db_path();

    tauri::Builder::default()
        .manage(AppState::new(db_path))
        .setup(|app| {
            windows::setup_windows(app);
            tray::setup_tray(app).ok();
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
            windows::show_widget,
            windows::hide_widget,
            windows::set_widget_opacity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn default_db_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = std::path::PathBuf::from(appdata).join("inwdeck");
        std::fs::create_dir_all(&dir).ok();
        dir.join("inwdeck.db")
    } else {
        PathBuf::from("inwdeck.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_inwdeck_message() {
        let result = greet();
        assert!(result.contains("inwdeck"));
    }
}
