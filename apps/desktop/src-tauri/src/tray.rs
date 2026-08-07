use std::sync::Mutex;
use tauri::menu::Menu;
use tauri::Manager;

/// The tray menu, kept so its items can be relabelled after a sync.
static TRAY_MENU: Mutex<Option<Menu<tauri::Wry>>> = Mutex::new(None);

/// Time shown next to "Sync now" in the tray menu, in the user's local time.
fn sync_label(stored: Option<chrono::DateTime<chrono::Local>>) -> String {
    match stored {
        Some(time) => format!("Sync now ({})", time.format("%H:%M")),
        None => "Sync now (--:--)".to_string(),
    }
}

/// Reads the stored UI language (falls back to English).
fn ui_language(app: &tauri::AppHandle) -> String {
    let state = app.state::<crate::state::AppState>();
    let Ok(guard) = state.ensure_storage() else {
        return "en".to_string();
    };
    let Some(storage) = guard.as_ref() else {
        return "en".to_string();
    };
    lnwdeck_storage::repositories::AppSettingsRepository::new(&storage.conn)
        .get("language")
        .ok()
        .flatten()
        .filter(|lang| !lang.is_empty())
        .unwrap_or_else(|| "en".to_string())
}

/// Localized tray menu labels: the UI language when supported, else English.
fn tray_text(app: &tauri::AppHandle, key: &str) -> String {
    let lang = ui_language(app);
    let th: &[(&str, &str)] = &[
        ("show", "แสดง lnwdeck"),
        ("widget", "สลับวิดเจ็ต"),
        ("pet-hide", "ซ่อนสัตว์เลี้ยง"),
        ("pet-show", "แสดงสัตว์เลี้ยง"),
        ("sync-now", "ซิงค์ตอนนี้ ({time})"),
        ("check-update", "ตรวจสอบอัปเดต"),
        ("quit", "ออกจาก lnwdeck"),
    ];
    if lang == "th" {
        if let Some((_, value)) = th.iter().find(|(k, _)| *k == key) {
            return value.to_string();
        }
    }
    match key {
        "show" => "Show lnwdeck".to_string(),
        "widget" => "Toggle Widget".to_string(),
        "pet-hide" => "Hide Pet".to_string(),
        "pet-show" => "Show Pet".to_string(),
        "sync-now" => sync_label(crate::last_sync_time(app)),
        "check-update" => "Check for updates".to_string(),
        "quit" => "Quit lnwdeck".to_string(),
        _ => key.to_string(),
    }
}

/// Updates the tray menu item with the newest sync time, if the menu exists.
pub fn update_sync_label(app: &tauri::AppHandle) {
    use tauri::menu::MenuItemKind;
    let menu = TRAY_MENU.lock().ok().and_then(|guard| guard.clone());
    let Some(menu) = menu else {
        return;
    };
    let Some(item) = menu.get("sync-now") else {
        return;
    };
    let stored = crate::last_sync_time(app);
    if let MenuItemKind::MenuItem(item) = item {
        let _ = item.set_text(sync_label(stored));
    }
}

/// Keeps the "Hide/Show Pet" tray item in step with the pet window.
pub fn update_pet_toggle_label(app: &tauri::AppHandle) {
    use tauri::menu::MenuItemKind;
    let menu = TRAY_MENU.lock().ok().and_then(|guard| guard.clone());
    let Some(menu) = menu else {
        return;
    };
    let Some(item) = menu.get("pet-toggle") else {
        return;
    };
    let visible = app
        .get_webview_window("pet")
        .and_then(|pet| pet.is_visible().ok())
        .unwrap_or(false);
    let label = if visible {
        tray_text(app, "pet-hide")
    } else {
        tray_text(app, "pet-show")
    };
    if let MenuItemKind::MenuItem(item) = item {
        let _ = item.set_text(label);
    }
}

/// Kicks off a full refresh in the background and updates the tray label when
/// it finishes. Never blocks the UI thread.
pub fn run_sync_now(app: tauri::AppHandle) {
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::commands::pipeline::refresh_all(app2.clone()).await {
            Ok(_) => {
                crate::record_sync_time(&app2);
                update_sync_label(&app2);
            }
            Err(error) => {
                crate::record_tray_event("SYNC_NOW_FAILED", &error, &app2);
            }
        }
    });
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let handle = app.handle();
    let show = MenuItemBuilder::with_id("show", tray_text(handle, "show")).build(app)?;
    let widget_toggle =
        MenuItemBuilder::with_id("widget", tray_text(handle, "widget")).build(app)?;
    let pet_toggle =
        MenuItemBuilder::with_id("pet-toggle", tray_text(handle, "pet-hide")).build(app)?;
    let sync_now =
        MenuItemBuilder::with_id("sync-now", tray_text(handle, "sync-now")).build(app)?;
    let check_update =
        MenuItemBuilder::with_id("check-update", tray_text(handle, "check-update")).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", tray_text(handle, "quit")).build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&widget_toggle)
        .item(&pet_toggle)
        .item(&sync_now)
        .item(&check_update)
        .item(&quit)
        .build()?;
    if let Ok(mut guard) = TRAY_MENU.lock() {
        *guard = Some(menu.clone());
    }

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("lnwdeck");

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
            "pet-toggle" => {
                // Shows or hides the desktop pet; the setting is remembered so
                // it survives a restart.
                let visible = app
                    .get_webview_window("pet")
                    .and_then(|pet| pet.is_visible().ok())
                    .unwrap_or(false);
                let result = if visible {
                    crate::windows::hide_pet_window(app.clone())
                } else {
                    crate::windows::show_pet_window(app.clone())
                };
                if let Err(error) = result {
                    crate::record_tray_event("PET_TOGGLE_FAILED", &error, app);
                }
                update_pet_toggle_label(app);
            }
            "sync-now" => {
                // Manual sync: refresh every provider in the background; the
                // menu label shows the newest successful sync time.
                run_sync_now(app.clone());
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

    // Show the stored last-sync time in the label right away.
    update_sync_label(app.handle());
    update_pet_toggle_label(app.handle());

    Ok(())
}
