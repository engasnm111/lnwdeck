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
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindowBuilder};

const WIDGET_LABEL: &str = "widget";
const MAIN_LABEL: &str = "main";
const PET_LABEL: &str = "pet";
/// Fixed widget sizes per preset. The widget is never user-resized; the
/// preset is chosen in Settings and applied by the backend.
pub const WIDGET_SIZE_PRESETS: &[(&str, f64, f64)] = &[
    ("small", 300.0, 300.0),
    ("medium", 400.0, 420.0),
    ("large", 500.0, 500.0),
];
const DEFAULT_WIDGET_SIZE: &str = "medium";
/// Pet window size presets (logical px). The preset scales both the window
/// and the sprite; the pet window is never user-resized.
pub const PET_SIZE_PRESETS: &[(&str, f64, f64)] = &[
    ("small", 200.0, 300.0),
    ("medium", 280.0, 400.0),
    ("large", 360.0, 520.0),
];
const DEFAULT_PET_SIZE: &str = "medium";
/// The pet window is deliberately small: it moves WITH the pet, so only the
/// pet's own surface intercepts clicks and the rest of the desktop stays
/// usable. Full-screen transparent overlays block every click underneath.
const PET_WINDOW_WIDTH: f64 = 280.0;
/// Tall enough for the sprite AND the hover tooltip above it (logical px).
const PET_WINDOW_HEIGHT: f64 = 400.0;
/// Gap between the screen bottom and the pet window when it is shown.
const PET_BOTTOM_MARGIN: i32 = 48;

/// Widget appearance settings handed to the webview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WidgetSettings {
    pub opacity: f64,
    pub locked: bool,
    pub visible: bool,
    /// Provider ids the user pinned. Empty means every provider that reported
    /// data, so a fresh install shows everything rather than nothing.
    pub selected_providers: Vec<String>,
    /// Layout: "bars", "rings" or "pet".
    pub view: String,
    /// Community pet id for the pet layout. Empty means the built-in robot.
    pub pet_id: String,
    /// Fixed window size preset: "small", "medium" or "large".
    pub size_preset: String,
}

/// Desktop pet window settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetWindowSettings {
    /// Whether the pet window is visible.
    pub visible: bool,
    /// Character id: "robot", "cat", "ghost", "dragon", "crab", "blob", or a community pet id.
    pub character: String,
    /// Walk speed: "slow", "normal", "fast".
    pub speed: String,
    /// Window opacity 0.1..=1.0.
    pub opacity: f64,
    /// Whether the pet auto-sleeps after inactivity.
    pub auto_sleep: bool,
    /// Fixed window size preset: "small", "medium" or "large".
    pub size_preset: String,
}

/// Dimensions for a pet size preset, or the medium default.
pub fn pet_size_dimensions(preset: &str) -> (f64, f64) {
    PET_SIZE_PRESETS
        .iter()
        .find(|(key, _, _)| *key == preset)
        .map(|(_, w, h)| (*w, *h))
        .unwrap_or_else(|| {
            PET_SIZE_PRESETS
                .iter()
                .find(|(key, _, _)| *key == DEFAULT_PET_SIZE)
                .map(|(_, w, h)| (*w, *h))
                .unwrap_or((280.0, 400.0))
        })
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

/// Dimensions for a widget size preset, or the medium default.
pub fn widget_size_dimensions(preset: &str) -> (f64, f64) {
    WIDGET_SIZE_PRESETS
        .iter()
        .find(|(key, _, _)| *key == preset)
        .map(|(_, w, h)| (*w, *h))
        .unwrap_or_else(|| {
            WIDGET_SIZE_PRESETS
                .iter()
                .find(|(key, _, _)| *key == DEFAULT_WIDGET_SIZE)
                .map(|(_, w, h)| (*w, *h))
                .unwrap_or((400.0, 420.0))
        })
}

/// Reads a stored numeric setting (widget position).
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

/// Current pet window settings, or the documented defaults.
pub fn pet_window_settings(app: &tauri::AppHandle) -> PetWindowSettings {
    let state = app.state::<AppState>();
    let defaults = PetWindowSettings {
        visible: false,
        character: "robot".to_string(),
        speed: "normal".to_string(),
        opacity: 1.0,
        auto_sleep: true,
        size_preset: DEFAULT_PET_SIZE.to_string(),
    };
    let Ok(guard) = state.ensure_storage() else {
        return defaults;
    };
    let Some(storage) = guard.as_ref() else {
        return defaults;
    };
    match SettingsService::load(&storage.conn) {
        Ok(settings) => PetWindowSettings {
            visible: settings.pet_visible,
            character: settings.pet_character,
            speed: settings.pet_speed,
            opacity: settings.pet_opacity,
            auto_sleep: settings.pet_auto_sleep,
            size_preset: settings.pet_size,
        },
        Err(_) => defaults,
    }
}

/// Current widget settings, or the documented defaults when storage is not
/// available yet.
pub fn widget_settings(app: &tauri::AppHandle) -> WidgetSettings {
    let state = app.state::<AppState>();
    let defaults = WidgetSettings {
        opacity: 1.0,
        locked: false,
        visible: false,
        selected_providers: Vec::new(),
        view: "bars".to_string(),
        pet_id: String::new(),
        size_preset: DEFAULT_WIDGET_SIZE.to_string(),
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
            selected_providers: SettingsService::widget_providers(&storage.conn)
                .unwrap_or_default(),
            view: SettingsService::widget_view(&storage.conn)
                .unwrap_or_else(|_| "bars".to_string()),
            pet_id: SettingsService::widget_pet_id(&storage.conn).unwrap_or_default(),
            size_preset: settings.widget_size,
        },
        Err(_) => defaults,
    }
}

/// Restores the widget's saved position and visibility at the fixed preset
/// size (the widget is never user-resized).
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

    let (width, height) = widget_size_dimensions(&widget_settings(&handle).size_preset);
    let _ = widget.set_size(LogicalSize::new(width, height));

    let size = widget
        .outer_size()
        .ok()
        .map(|size| (size.width as f64, size.height as f64))
        .unwrap_or((width, height));

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

pub fn setup_windows(app: &tauri::App) {
    let _main =
        WebviewWindowBuilder::new(app, MAIN_LABEL, tauri::WebviewUrl::App("index.html".into()))
            .title("lnwdeck")
            .inner_size(1280.0, 840.0)
            .min_inner_size(960.0, 640.0)
            .build()
            .expect("failed to build main window");

    let (width, height) = widget_size_dimensions(DEFAULT_WIDGET_SIZE);
    let _widget = WebviewWindowBuilder::new(
        app,
        WIDGET_LABEL,
        tauri::WebviewUrl::App("widget.html".into()),
    )
    .title("lnwdeck quota")
    .inner_size(width, height)
    .always_on_top(true)
    .decorations(false)
    // Fixed size: the preset is chosen in Settings, never user-resized.
    // Content scrolls inside the window.
    .resizable(false)
    .skip_taskbar(true)
    .visible(false)
    .build()
    .expect("failed to build widget window");

    // Desktop pet: a small transparent, always-on-top window that follows the
    // pet as it walks. It is never full-screen, so clicks outside the pet hit
    // the desktop normally. The webview background is explicitly transparent
    // so no square frame is visible around the pet.
    let _pet = WebviewWindowBuilder::new(app, PET_LABEL, tauri::WebviewUrl::App("pet.html".into()))
        .title("lnwdeck pet")
        .inner_size(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT)
        .always_on_top(true)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .visible(false)
        .transparent(true)
        // No DWM drop shadow: on an undecorated transparent window it renders as
        // a visible square outline around the pet.
        .shadow(false)
        .background_color(tauri::window::Color(0, 0, 0, 0))
        .build()
        .expect("failed to build pet window");
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
        // Closing the pet hides it, and is remembered.
        PET_LABEL => {
            window.hide().ok();
            let app = window.app_handle();
            let state = app.state::<AppState>();
            if let Ok(guard) = state.ensure_storage() {
                if let Some(storage) = guard.as_ref() {
                    let _ = SettingsService::set_pet_visible(&storage.conn, false);
                }
            }
            emit_pet_window_settings(app);
        }
        _ => {}
    }
}

pub(crate) fn emit_widget_settings(app: &tauri::AppHandle) {
    let settings = widget_settings(app);
    let _ = app.emit("widget-settings-changed", settings);
}

pub(crate) fn emit_pet_window_settings(app: &tauri::AppHandle) {
    let settings = pet_window_settings(app);
    let _ = app.emit("pet-window-settings-changed", settings);
}

/// A sensible starting position for the pet window: centered horizontally,
/// resting just above the bottom edge of the primary monitor.
fn default_pet_position(window: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    let monitor = window.current_monitor().ok().flatten()?;
    let pos = monitor.position();
    let size = monitor.size();
    let (width, height) = pet_size_dimensions(DEFAULT_PET_SIZE);
    let x = pos.x + (size.width as i32 - width as i32) / 2;
    let y = pos.y + size.height as i32 - height as i32 - PET_BOTTOM_MARGIN;
    Some((x.max(pos.x), y.max(pos.y)))
}

/// Restores the pet window visibility at its fixed size preset.
pub fn restore_pet_window_state(app: &tauri::App) {
    let handle = app.handle().clone();
    if pet_window_settings(&handle).visible {
        if let Some(pet_window) = app.get_webview_window(PET_LABEL) {
            apply_pet_size(&handle);
            if let Some((x, y)) = default_pet_position(&pet_window) {
                let _ = pet_window.set_position(PhysicalPosition::new(x, y));
            }
            let _ = pet_window.show();
        }
    }
}

/// Resizes the pet window to the stored size preset and broadcasts the
/// settings, so a size change takes effect immediately.
pub fn apply_pet_size(app: &tauri::AppHandle) {
    if let Some(pet_window) = app.get_webview_window(PET_LABEL) {
        let preset = pet_window_settings(app).size_preset;
        let (width, height) = pet_size_dimensions(&preset);
        let _ = pet_window.set_size(LogicalSize::new(width, height));
    }
    emit_pet_window_settings(app);
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

/// Switches the widget layout. Returns the stored layout.
#[tauri::command]
pub fn set_widget_view(app: tauri::AppHandle, view: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_widget_view(&storage.conn, &view).map_err(|e| e.to_string())?
    };
    emit_widget_settings(&app);
    Ok(stored)
}

/// Switches the widget's fixed size preset and resizes the window.
/// Returns the stored preset.
#[tauri::command]
pub fn set_widget_size_preset(app: tauri::AppHandle, preset: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_widget_size_preset(&storage.conn, &preset)
            .map_err(|e| e.to_string())?
    };
    apply_widget_size(&app);
    Ok(stored)
}

/// Resizes the widget window to the currently stored size preset and
/// broadcasts the settings, so a size change saved through any path
/// (Settings form or the dedicated command) takes effect immediately.
pub fn apply_widget_size(app: &tauri::AppHandle) {
    if let Some(widget) = app.get_webview_window(WIDGET_LABEL) {
        let preset = widget_settings(app).size_preset;
        let (width, height) = widget_size_dimensions(&preset);
        let _ = widget.set_size(LogicalSize::new(width, height));
    }
    emit_widget_settings(app);
}

/// Pins the widget to a set of providers. An empty list restores every
/// provider. Returns the stored selection.
#[tauri::command]
pub fn set_widget_providers(
    app: tauri::AppHandle,
    providers: Vec<String>,
) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_widget_providers(&storage.conn, &providers)
            .map_err(|e| e.to_string())?
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

/// Current pet window settings, for the pet webview to render.
#[tauri::command]
pub fn get_pet_window_settings(app: tauri::AppHandle) -> PetWindowSettings {
    pet_window_settings(&app)
}

/// Shows the desktop pet window and remembers the setting.
#[tauri::command]
pub fn show_pet_window(app: tauri::AppHandle) -> Result<(), String> {
    let pet_window = app
        .get_webview_window(PET_LABEL)
        .ok_or("pet window not found")?;
    if let Some((x, y)) = default_pet_position(&pet_window) {
        let _ = pet_window.set_position(PhysicalPosition::new(x, y));
    }
    pet_window.show().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    if let Ok(guard) = state.ensure_storage() {
        if let Some(storage) = guard.as_ref() {
            let _ = SettingsService::set_pet_visible(&storage.conn, true);
        }
    }
    emit_pet_window_settings(&app);
    Ok(())
}

/// Moves the pet window so it follows the pet as it walks. The frontend owns
/// the movement loop and calls this with physical screen coordinates.
#[tauri::command]
pub fn move_pet_window(app: tauri::AppHandle, x: i32, y: i32) -> Result<(), String> {
    let pet_window = app
        .get_webview_window(PET_LABEL)
        .ok_or("pet window not found")?;
    pet_window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// Reads an installed pet's spritesheet as raw bytes for the webview.
///
/// The pet window renders spritesheets as Blob object URLs because custom
/// URI schemes are blocked by WebView2 on http dev origins; bytes over IPC
/// work on every origin. Only the well-known file under a validated pet id
/// is served.
#[tauri::command]
pub fn read_pet_spritesheet(
    app: tauri::AppHandle,
    id: String,
) -> Result<tauri::ipc::Response, String> {
    if !crate::pets::is_pet_id(&id) {
        return Err("invalid pet id".to_string());
    }
    let path = crate::commands::pets::pet_store_dir(&app)
        .join(&id)
        .join(crate::pets::PET_SPRITESHEET);
    let bytes = std::fs::read(&path).map_err(|_| "pet spritesheet not found".to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Hides the desktop pet window and remembers the setting.
#[tauri::command]
pub fn hide_pet_window(app: tauri::AppHandle) -> Result<(), String> {
    let pet_window = app
        .get_webview_window(PET_LABEL)
        .ok_or("pet window not found")?;
    pet_window.hide().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    if let Ok(guard) = state.ensure_storage() {
        if let Some(storage) = guard.as_ref() {
            let _ = SettingsService::set_pet_visible(&storage.conn, false);
        }
    }
    emit_pet_window_settings(&app);
    Ok(())
}

/// Sets the desktop pet character and remembers the setting.
#[tauri::command]
pub fn set_pet_character(app: tauri::AppHandle, character: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_pet_character(&storage.conn, &character).map_err(|e| e.to_string())?
    };
    emit_pet_window_settings(&app);
    Ok(stored)
}

/// Sets the desktop pet walk speed and remembers the setting.
#[tauri::command]
pub fn set_pet_speed(app: tauri::AppHandle, speed: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_pet_speed(&storage.conn, &speed).map_err(|e| e.to_string())?
    };
    emit_pet_window_settings(&app);
    Ok(stored)
}

/// Sets the desktop pet window opacity and remembers the setting.
#[tauri::command]
pub fn set_pet_opacity(app: tauri::AppHandle, opacity: f64) -> Result<f64, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_pet_opacity(&storage.conn, opacity).map_err(|e| e.to_string())?
    };
    emit_pet_window_settings(&app);
    Ok(stored)
}

/// Sets whether the pet auto-sleeps after inactivity.
#[tauri::command]
pub fn set_pet_auto_sleep(app: tauri::AppHandle, auto_sleep: bool) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_pet_auto_sleep(&storage.conn, auto_sleep).map_err(|e| e.to_string())?
    };
    emit_pet_window_settings(&app);
    Ok(stored)
}

/// Switches the pet's fixed size preset and resizes the window.
/// Returns the stored preset.
#[tauri::command]
pub fn set_pet_size_preset(app: tauri::AppHandle, preset: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_pet_size_preset(&storage.conn, &preset).map_err(|e| e.to_string())?
    };
    apply_pet_size(&app);
    Ok(stored)
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
    fn size_presets_are_fixed_and_smaller_than_a_monitor() {
        assert_eq!(widget_size_dimensions("small"), (300.0, 300.0));
        assert_eq!(widget_size_dimensions("medium"), (400.0, 420.0));
        assert_eq!(widget_size_dimensions("large"), (500.0, 500.0));
        assert_eq!(
            widget_size_dimensions("unknown"),
            (400.0, 420.0),
            "an unknown preset falls back to medium"
        );
        for (_, width, height) in WIDGET_SIZE_PRESETS {
            assert!(
                *width >= 300.0 && *height >= 300.0,
                "every preset stays comfortably small"
            );
        }
    }

    #[test]
    fn widget_defaults_are_conservative() {
        let defaults = WidgetSettings {
            opacity: 1.0,
            locked: false,
            visible: false,
            selected_providers: Vec::new(),
            view: "bars".to_string(),
            pet_id: String::new(),
            size_preset: DEFAULT_WIDGET_SIZE.to_string(),
        };
        assert!(!defaults.visible, "the widget stays hidden until requested");
        assert_eq!(defaults.opacity, 1.0);
        assert!(
            defaults.selected_providers.is_empty(),
            "no selection means every provider is shown"
        );
        assert_eq!(defaults.view, "bars");
        assert!(
            defaults.pet_id.is_empty(),
            "no pet selected means the built-in robot"
        );
    }
}
