use tauri::Manager;

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItemBuilder::with_id("show", "Show lnwdeck").build(app)?;
    let widget_toggle = MenuItemBuilder::with_id("widget", "Toggle Widget").build(app)?;
    let check_update = MenuItemBuilder::with_id("check-update", "Check for updates").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit lnwdeck").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&widget_toggle)
        .item(&check_update)
        .item(&quit)
        .build()?;

    let mut builder = TrayIconBuilder::new().menu(&menu).tooltip("lnwdeck");

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
            "widget" => {
                // Toggling through the window commands keeps the stored widget
                // visibility in step with what is on screen.
                let visible = app
                    .get_webview_window("widget")
                    .and_then(|widget| widget.is_visible().ok())
                    .unwrap_or(false);
                let result = if visible {
                    crate::windows::hide_widget(app.clone())
                } else {
                    crate::windows::show_widget(app.clone())
                };
                if let Err(error) = result {
                    crate::record_tray_event("WIDGET_TOGGLE_FAILED", &error, app);
                }
            }
            "check-update" => {
                // Silent update: check, download, verify and install the
                // newest version, then restart the application.
                crate::updater::check_and_install_silent(app.clone());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
        })
        .build(app)?;

    Ok(())
}
