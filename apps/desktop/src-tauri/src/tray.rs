use std::sync::Mutex;
use tauri::menu::Menu;
use tauri::Manager;

/// The tray menu, kept so its items can be relabelled after a sync.
static TRAY_MENU: Mutex<Option<Menu<tauri::Wry>>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct TrayLabels {
    show: &'static str,
    widget: &'static str,
    pet_hide: &'static str,
    pet_show: &'static str,
    sync_now: &'static str,
    check_update: &'static str,
    quit: &'static str,
    tooltip: &'static str,
}

/// Native tray strings mirror the nine WebView locales. Keeping this table in
/// Rust makes the tray usable even when no WebView has been opened yet.
fn labels(language: &str) -> TrayLabels {
    match language {
        "th" => TrayLabels {
            show: "แสดง lnwdeck",
            widget: "สลับวิดเจ็ต",
            pet_hide: "ซ่อนสัตว์เลี้ยง",
            pet_show: "แสดงสัตว์เลี้ยง",
            sync_now: "ซิงค์ตอนนี้ ({time})",
            check_update: "ตรวจสอบอัปเดต",
            quit: "ออกจาก lnwdeck",
            tooltip: "lnwdeck กำลังทำงาน",
        },
        "zh" => TrayLabels {
            show: "显示 lnwdeck",
            widget: "切换小组件",
            pet_hide: "隐藏宠物",
            pet_show: "显示宠物",
            sync_now: "立即同步 ({time})",
            check_update: "检查更新",
            quit: "退出 lnwdeck",
            tooltip: "lnwdeck 正在运行",
        },
        "ja" => TrayLabels {
            show: "lnwdeckを表示",
            widget: "ウィジェットを切り替え",
            pet_hide: "ペットを隠す",
            pet_show: "ペットを表示",
            sync_now: "今すぐ同期 ({time})",
            check_update: "更新を確認",
            quit: "lnwdeckを終了",
            tooltip: "lnwdeck 実行中",
        },
        "ko" => TrayLabels {
            show: "lnwdeck 표시",
            widget: "위젯 전환",
            pet_hide: "펫 숨기기",
            pet_show: "펫 표시",
            sync_now: "지금 동기화 ({time})",
            check_update: "업데이트 확인",
            quit: "lnwdeck 종료",
            tooltip: "lnwdeck 실행 중",
        },
        "de" => TrayLabels {
            show: "lnwdeck anzeigen",
            widget: "Widget umschalten",
            pet_hide: "Pet ausblenden",
            pet_show: "Pet einblenden",
            sync_now: "Jetzt synchronisieren ({time})",
            check_update: "Nach Updates suchen",
            quit: "lnwdeck beenden",
            tooltip: "lnwdeck läuft",
        },
        "fr" => TrayLabels {
            show: "Afficher lnwdeck",
            widget: "Basculer le widget",
            pet_hide: "Masquer l’animal",
            pet_show: "Afficher l’animal",
            sync_now: "Synchroniser maintenant ({time})",
            check_update: "Rechercher des mises à jour",
            quit: "Quitter lnwdeck",
            tooltip: "lnwdeck est actif",
        },
        "es" => TrayLabels {
            show: "Mostrar lnwdeck",
            widget: "Alternar widget",
            pet_hide: "Ocultar mascota",
            pet_show: "Mostrar mascota",
            sync_now: "Sincronizar ahora ({time})",
            check_update: "Buscar actualizaciones",
            quit: "Salir de lnwdeck",
            tooltip: "lnwdeck está en ejecución",
        },
        "ru" => TrayLabels {
            show: "Показать lnwdeck",
            widget: "Переключить виджет",
            pet_hide: "Скрыть питомца",
            pet_show: "Показать питомца",
            sync_now: "Синхронизировать сейчас ({time})",
            check_update: "Проверить обновления",
            quit: "Выйти из lnwdeck",
            tooltip: "lnwdeck работает",
        },
        _ => TrayLabels {
            show: "Show lnwdeck",
            widget: "Toggle widget",
            pet_hide: "Hide pet",
            pet_show: "Show pet",
            sync_now: "Sync now ({time})",
            check_update: "Check for updates",
            quit: "Quit lnwdeck",
            tooltip: "lnwdeck is running",
        },
    }
}

/// Time shown next to "Sync now" in the tray menu, in the user's local time.
fn sync_label(app: &tauri::AppHandle, stored: Option<chrono::DateTime<chrono::Local>>) -> String {
    let template = labels(&ui_language(app)).sync_now;
    let time = stored
        .map(|value| value.format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string());
    template.replace("{time}", &time)
}

fn tray_tooltip(app: &tauri::AppHandle) -> String {
    labels(&ui_language(app)).tooltip.to_string()
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
    let text = labels(&ui_language(app));
    match key {
        "show" => text.show,
        "widget" => text.widget,
        "pet-hide" => text.pet_hide,
        "pet-show" => text.pet_show,
        "sync-now" => return sync_label(app, crate::last_sync_time(app)),
        "check-update" => text.check_update,
        "quit" => text.quit,
        _ => key,
    }
    .to_string()
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
        let _ = item.set_text(sync_label(app, stored));
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

/// Relabels the native tray immediately after the UI language changes.
pub fn update_language_labels(app: &tauri::AppHandle) {
    use tauri::menu::MenuItemKind;

    let menu = TRAY_MENU.lock().ok().and_then(|guard| guard.clone());
    if let Some(menu) = menu {
        for (id, key) in [
            ("show", "show"),
            ("widget", "widget"),
            ("check-update", "check-update"),
            ("quit", "quit"),
        ] {
            if let Some(MenuItemKind::MenuItem(item)) = menu.get(id) {
                let _ = item.set_text(tray_text(app, key));
            }
        }
    }
    update_sync_label(app);
    update_pet_toggle_label(app);
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(tray_tooltip(app)));
    }
}

/// Kicks off a full refresh in the background and updates the tray label when
/// it finishes. Never blocks the UI thread.
pub fn run_sync_now(app: tauri::AppHandle) {
    if let Err(error) = crate::commands::pipeline::start_refresh(app.clone()) {
        crate::record_tray_event("SYNC_NOW_FAILED", &error, &app);
    }
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
        .tooltip(tray_tooltip(handle));

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                if let Err(error) = crate::windows::show_main_window(app.clone()) {
                    crate::record_tray_event("MAIN_WINDOW_SHOW_FAILED", &error, app);
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
                position,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Err(error) = crate::windows::show_tray_popup(app.clone(), position) {
                    crate::record_tray_event("TRAY_POPUP_SHOW_FAILED", &error, app);
                }
            }
        })
        .build(app)?;

    // Show the stored last-sync time in the label right away.
    update_sync_label(app.handle());
    update_pet_toggle_label(app.handle());

    Ok(())
}
