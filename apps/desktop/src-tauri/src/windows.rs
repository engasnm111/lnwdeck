use crate::state::AppState;
use lnwdeck_storage::repositories::AppSettingsRepository;
use tauri::{Manager, PhysicalPosition, WebviewWindowBuilder};

/// Clamps a widget top-left position so the whole widget stays on screen.
fn clamp_position(
    x: f64,
    y: f64,
    screen_width: f64,
    screen_height: f64,
    widget_width: f64,
    widget_height: f64,
) -> (f64, f64) {
    let max_x = (screen_width - widget_width).max(0.0);
    let max_y = (screen_height - widget_height).max(0.0);
    (x.clamp(0.0, max_x), y.clamp(0.0, max_y))
}

/// Restores the widget window's last saved position, clamped to the current
/// monitor. No-op when no position was saved yet.
pub fn restore_widget_position(app: &tauri::App) {
    let Some(widget) = app.get_webview_window("widget") else {
        return;
    };
    let state = app.state::<AppState>();
    let Ok(storage_guard) = state.ensure_storage() else {
        return;
    };
    let storage = storage_guard.as_ref();
    let Some(storage) = storage else {
        return;
    };
    let settings = AppSettingsRepository::new(&storage.conn);
    let Ok(Some(x_str)) = settings.get("widget_x") else {
        return;
    };
    let Ok(Some(y_str)) = settings.get("widget_y") else {
        return;
    };
    let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) else {
        return;
    };

    let screen = widget
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| {
            let size = monitor.size();
            (size.width as f64, size.height as f64)
        })
        .unwrap_or((1920.0, 1080.0));
    let size = widget
        .outer_size()
        .ok()
        .map(|s| (s.width as f64, s.height as f64))
        .unwrap_or((320.0, 200.0));

    let (cx, cy) = clamp_position(x, y, screen.0, screen.1, size.0, size.1);
    let _ = widget.set_position(PhysicalPosition::new(cx as i32, cy as i32));
}

/// Persists the widget window's current position so it can be restored on the
/// next launch. No-op when the window or storage is unavailable.
pub fn save_widget_position(app: &tauri::AppHandle) {
    let Some(widget) = app.get_webview_window("widget") else {
        return;
    };
    let Ok(position) = widget.outer_position() else {
        return;
    };
    let state = app.state::<AppState>();
    let Ok(storage_guard) = state.ensure_storage() else {
        return;
    };
    let storage = storage_guard.as_ref();
    let Some(storage) = storage else {
        return;
    };
    let settings = AppSettingsRepository::new(&storage.conn);
    let _ = settings.set("widget_x", &position.x.to_string());
    let _ = settings.set("widget_y", &position.y.to_string());
}

pub fn setup_windows(app: &tauri::App) {
    let _main = WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("lnwdeck")
        .inner_size(1200.0, 800.0)
        .build()
        .expect("failed to build main window");

    let _widget =
        WebviewWindowBuilder::new(app, "widget", tauri::WebviewUrl::App("widget.html".into()))
            .title("lnwdeck Widget")
            .inner_size(320.0, 200.0)
            .always_on_top(true)
            .decorations(false)
            .resizable(true)
            .min_inner_size(200.0, 100.0)
            .visible(false)
            .build()
            .expect("failed to build widget window");
}

pub fn handle_close_request(window: &tauri::Window) {
    let label = window.label();
    if label == "main" {
        window.hide().ok();
    }
}

#[tauri::command]
pub fn show_widget(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("widget") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("widget window not found".to_string())
    }
}

#[tauri::command]
pub fn hide_widget(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("widget") {
        w.hide().map_err(|e| e.to_string())
    } else {
        Err("widget window not found".to_string())
    }
}

/// Sets the floating widget opacity (0.1..=1.0). Native window opacity is
/// not exposed by the current Tauri runtime, so the value drives the widget
/// root's CSS opacity inside the webview instead of being discarded.
#[tauri::command]
pub fn set_widget_opacity(app: tauri::AppHandle, opacity: f64) -> Result<(), String> {
    let clamped = opacity.clamp(0.1, 1.0);
    if let Some(w) = app.get_webview_window("widget") {
        let js = format!("document.querySelector('.widget-root').style.opacity = '{clamped}'");
        w.eval(js).map_err(|e| e.to_string())
    } else {
        Err("widget window not found".to_string())
    }
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("main window not found".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_main_window_exists() {
        assert_eq!(std::mem::size_of_val(&show_main_window), 0);
    }

    #[test]
    fn clamp_position_keeps_widget_on_screen() {
        assert_eq!(
            clamp_position(100.0, 100.0, 1920.0, 1080.0, 320.0, 200.0),
            (100.0, 100.0)
        );
        assert_eq!(
            clamp_position(-80.0, -50.0, 1920.0, 1080.0, 320.0, 200.0),
            (0.0, 0.0)
        );
        assert_eq!(
            clamp_position(2000.0, 2000.0, 1920.0, 1080.0, 320.0, 200.0),
            (1600.0, 880.0)
        );
    }

    #[test]
    fn clamp_position_handles_widget_larger_than_screen() {
        assert_eq!(
            clamp_position(0.0, 0.0, 200.0, 100.0, 320.0, 200.0),
            (0.0, 0.0),
            "clamps to zero rather than negative"
        );
    }
}
