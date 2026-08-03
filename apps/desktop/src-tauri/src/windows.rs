use tauri::{Manager, WebviewWindowBuilder};

pub fn setup_windows(app: &tauri::App) {
    let _main = WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("inwdeck")
        .inner_size(1200.0, 800.0)
        .build()
        .expect("failed to build main window");

    let _widget = WebviewWindowBuilder::new(
        app,
        "widget",
        tauri::WebviewUrl::App("index.html#/widget".into()),
    )
    .title("inwdeck Widget")
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

#[tauri::command]
pub fn set_widget_opacity(app: tauri::AppHandle, opacity: f64) -> Result<(), String> {
    let _clamped = opacity.clamp(0.1, 1.0);
    if let Some(w) = app.get_webview_window("widget") {
        w.set_always_on_top(true).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("widget window not found".to_string())
    }
}
