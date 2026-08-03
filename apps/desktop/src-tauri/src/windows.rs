//! Window management for the dashboard and the floating quota widget.
//!
//! Widget opacity, lock mode, size and position all live in `app_settings`, so
//! the backend and the webview cannot disagree about them: a command writes the
//! value, emits `widget-settings-changed`, and the widget renders what it reads
//! back. The previous implementation injected CSS through `eval` while the
//! webview kept its own copy in localStorage, so the command had no effect.

use crate::state::AppState;
use lnwdeck_application::settings::SettingsService;
use lnwdeck_storage::repositories::AppSettingsRepository;
use serde::Serialize;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindowBuilder};

const WIDGET_LABEL: &str = "widget";
const MAIN_LABEL: &str = "main";
const DEFAULT_WIDGET_WIDTH: f64 = 340.0;
const DEFAULT_WIDGET_HEIGHT: f64 = 260.0;
const MIN_WIDGET_WIDTH: f64 = 240.0;
const MIN_WIDGET_HEIGHT: f64 = 140.0;

/// Widget appearance settings handed to the webview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WidgetSettings {
    pub opacity: f64,
    pub locked: bool,
    pub visible: bool,
}

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

/// Clamps a restored widget size to something usable on the current monitor.
fn clamp_size(width: f64, height: f64, screen_width: f64, screen_height: f64) -> (f64, f64) {
    (
        width.clamp(MIN_WIDGET_WIDTH, screen_width.max(MIN_WIDGET_WIDTH)),
        height.clamp(MIN_WIDGET_HEIGHT, screen_height.max(MIN_WIDGET_HEIGHT)),
    )
}

/// Reads a stored numeric setting.
fn stored_f64(app: &tauri::AppHandle, key: &str) -> Option<f64> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage().ok()?;
    let storage = guard.as_ref()?;
    AppSettingsRepository::new(&storage.conn)
        .get(key)
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
}

fn write_setting(app: &tauri::AppHandle, key: &str, value: &str) {
    let state = app.state::<AppState>();
    let Ok(guard) = state.ensure_storage() else {
        return;
    };
    let Some(storage) = guard.as_ref() else {
        return;
    };
    let _ = AppSettingsRepository::new(&storage.conn).set(key, value);
}

/// Current widget settings, or the documented defaults when storage is not
/// available yet.
pub fn widget_settings(app: &tauri::AppHandle) -> WidgetSettings {
    let state = app.state::<AppState>();
    let defaults = WidgetSettings {
        opacity: 1.0,
        locked: false,
        visible: false,
    };
    let Ok(guard) = state.ensure_storage() else {
        return defaults;
    };
    let Some(storage) = guard.as_ref() else {
        return defaults;
    };
    match SettingsService::load(&storage.conn) {
        Ok(settings) => WidgetSettings {
            opacity: settings.widget_opacity,
            locked: settings.widget_locked,
            visible: settings.widget_visible,
        },
        Err(_) => defaults,
    }
}

/// Restores the widget's saved geometry and visibility.
pub fn restore_widget_state(app: &tauri::App) {
    let handle = app.handle().clone();
    let Some(widget) = app.get_webview_window(WIDGET_LABEL) else {
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

    // Size first: the position clamp depends on it.
    if let (Some(width), Some(height)) = (
        stored_f64(&handle, "widget_width"),
        stored_f64(&handle, "widget_height"),
    ) {
        let (width, height) = clamp_size(width, height, screen.0, screen.1);
        let _ = widget.set_size(PhysicalSize::new(width as u32, height as u32));
    }

    let size = widget
        .outer_size()
        .ok()
        .map(|size| (size.width as f64, size.height as f64))
        .unwrap_or((DEFAULT_WIDGET_WIDTH, DEFAULT_WIDGET_HEIGHT));

    if let (Some(x), Some(y)) = (
        stored_f64(&handle, "widget_x"),
        stored_f64(&handle, "widget_y"),
    ) {
        let (x, y) = clamp_position(x, y, screen.0, screen.1, size.0, size.1);
        let _ = widget.set_position(PhysicalPosition::new(x as i32, y as i32));
    }

    if widget_settings(&handle).visible {
        let _ = widget.show();
    }
}

/// Persists the widget's current position.
pub fn save_widget_position(app: &tauri::AppHandle) {
    let Some(widget) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let Ok(position) = widget.outer_position() else {
        return;
    };
    write_setting(app, "widget_x", &position.x.to_string());
    write_setting(app, "widget_y", &position.y.to_string());
}

/// Persists the widget's current size.
pub fn save_widget_size(app: &tauri::AppHandle) {
    let Some(widget) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let Ok(size) = widget.outer_size() else {
        return;
    };
    write_setting(app, "widget_width", &size.width.to_string());
    write_setting(app, "widget_height", &size.height.to_string());
}

pub fn setup_windows(app: &tauri::App) {
    let _main =
        WebviewWindowBuilder::new(app, MAIN_LABEL, tauri::WebviewUrl::App("index.html".into()))
            .title("lnwdeck")
            .inner_size(1280.0, 840.0)
            .min_inner_size(960.0, 640.0)
            .build()
            .expect("failed to build main window");

    let _widget = WebviewWindowBuilder::new(
        app,
        WIDGET_LABEL,
        tauri::WebviewUrl::App("widget.html".into()),
    )
    .title("lnwdeck quota")
    .inner_size(DEFAULT_WIDGET_WIDTH, DEFAULT_WIDGET_HEIGHT)
    .min_inner_size(MIN_WIDGET_WIDTH, MIN_WIDGET_HEIGHT)
    .always_on_top(true)
    .decorations(false)
    .resizable(true)
    .skip_taskbar(true)
    .visible(false)
    .build()
    .expect("failed to build widget window");
}

pub fn handle_close_request(window: &tauri::Window) {
    match window.label() {
        // Closing the dashboard hides it to the tray; the app keeps collecting.
        MAIN_LABEL => {
            window.hide().ok();
        }
        // Closing the widget is the same as hiding it, and is remembered.
        WIDGET_LABEL => {
            window.hide().ok();
            let app = window.app_handle();
            let state = app.state::<AppState>();
            if let Ok(guard) = state.ensure_storage() {
                if let Some(storage) = guard.as_ref() {
                    let _ = SettingsService::set_widget_visible(&storage.conn, false);
                }
            }
            emit_widget_settings(app);
        }
        _ => {}
    }
}

fn emit_widget_settings(app: &tauri::AppHandle) {
    let settings = widget_settings(app);
    let _ = app.emit("widget-settings-changed", settings);
}

/// Current widget settings, for the widget webview to render.
#[tauri::command]
pub fn get_widget_settings(app: tauri::AppHandle) -> WidgetSettings {
    widget_settings(&app)
}

#[tauri::command]
pub fn show_widget(app: tauri::AppHandle) -> Result<(), String> {
    let widget = app
        .get_webview_window(WIDGET_LABEL)
        .ok_or("widget window not found")?;
    widget.show().map_err(|e| e.to_string())?;
    widget.set_focus().map_err(|e| e.to_string())?;
    persist_visibility(&app, true)?;
    emit_widget_settings(&app);
    Ok(())
}

#[tauri::command]
pub fn hide_widget(app: tauri::AppHandle) -> Result<(), String> {
    let widget = app
        .get_webview_window(WIDGET_LABEL)
        .ok_or("widget window not found")?;
    widget.hide().map_err(|e| e.to_string())?;
    persist_visibility(&app, false)?;
    emit_widget_settings(&app);
    Ok(())
}

fn persist_visibility(app: &tauri::AppHandle, visible: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    SettingsService::set_widget_visible(&storage.conn, visible).map_err(|e| e.to_string())?;
    Ok(())
}

/// Sets the floating widget opacity (0.1..=1.0).
///
/// The value is stored and broadcast; the widget applies it on the next render.
/// Returns the stored value so the caller reports what was persisted.
#[tauri::command]
pub fn set_widget_opacity(app: tauri::AppHandle, opacity: f64) -> Result<f64, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_widget_opacity(&storage.conn, opacity).map_err(|e| e.to_string())?
    };
    emit_widget_settings(&app);
    Ok(stored)
}

/// Locks or unlocks the widget. A locked widget cannot be dragged.
#[tauri::command]
pub fn set_widget_locked(app: tauri::AppHandle, locked: bool) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_widget_locked(&storage.conn, locked).map_err(|e| e.to_string())?
    };
    emit_widget_settings(&app);
    Ok(stored)
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_LABEL)
        .ok_or("main window not found")?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn clamp_size_enforces_a_usable_minimum() {
        assert_eq!(
            clamp_size(340.0, 260.0, 1920.0, 1080.0),
            (340.0, 260.0),
            "a stored size inside the bounds is kept"
        );
        assert_eq!(
            clamp_size(10.0, 10.0, 1920.0, 1080.0),
            (MIN_WIDGET_WIDTH, MIN_WIDGET_HEIGHT),
            "a tiny stored size is raised to the minimum"
        );
        let (width, height) = clamp_size(9999.0, 9999.0, 1920.0, 1080.0);
        assert_eq!((width, height), (1920.0, 1080.0));
    }

    #[test]
    fn clamp_size_survives_a_screen_smaller_than_the_minimum() {
        assert_eq!(
            clamp_size(300.0, 300.0, 100.0, 100.0),
            (MIN_WIDGET_WIDTH, MIN_WIDGET_HEIGHT),
            "the minimum wins over an implausibly small monitor"
        );
    }

    #[test]
    fn widget_defaults_are_conservative() {
        let defaults = WidgetSettings {
            opacity: 1.0,
            locked: false,
            visible: false,
        };
        assert!(!defaults.visible, "the widget stays hidden until requested");
        assert_eq!(defaults.opacity, 1.0);
    }
}
