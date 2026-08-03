use tauri::WebviewWindowBuilder;

pub fn setup_windows(app: &tauri::App) {
    let _main = WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("inwdeck")
        .inner_size(1200.0, 800.0)
        .build()
        .expect("failed to build main window");
}

pub fn handle_close_request(window: &tauri::Window) {
    let label = window.label();
    if label == "main" {
        window.hide().ok();
    }
}
