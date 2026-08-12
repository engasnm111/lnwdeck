use lnwdeck_storage::Storage;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, RwLock};

use lnwdeck_provider_runtime::AdapterRegistry;

pub struct AppState {
    pub storage: Mutex<Option<Storage>>,
    pub db_path: PathBuf,
    /// Provider registry, built once on first use. It is the single source of
    /// provider identity for every read model. A read-write lock: the refresh
    /// cycle holds a read guard for its whole duration (collection is slow),
    /// and dashboard commands take read guards too, so a running cycle never
    /// blocks the main thread's UI commands.
    pub registry: RwLock<AdapterRegistry>,
    /// Screen rectangle (logical px: x, y, width, height) the pet sprite and
    /// its tooltip currently occupy. Outside it the pet window is click-through
    /// so it never blocks the desktop underneath.
    pub pet_hit_rect: Mutex<Option<[f64; 4]>>,
    /// Whether the pet window should ignore cursor events, computed by the
    /// background cursor-polling thread and applied by the webview.
    pub pet_click_through: AtomicBool,
    /// True while a refresh cycle is running (manual or background). Prevents
    /// overlapping cycles that pile up and freeze the machine.
    pub refresh_running: AtomicBool,
    /// Set by the native cancel command and checked between provider jobs.
    pub refresh_cancel_requested: AtomicBool,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            storage: Mutex::new(None),
            db_path,
            registry: RwLock::new(AdapterRegistry::new()),
            pet_hit_rect: Mutex::new(None),
            pet_click_through: AtomicBool::new(true),
            refresh_running: AtomicBool::new(false),
            refresh_cancel_requested: AtomicBool::new(false),
        }
    }

    pub fn ensure_storage(&self) -> Result<std::sync::MutexGuard<'_, Option<Storage>>, String> {
        let mut guard = self.storage.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            let storage = Storage::open(&self.db_path).map_err(|e| format!("storage open: {e}"))?;
            lnwdeck_storage::migrations::apply_all(&storage.conn)
                .map_err(|e| format!("migration: {e}"))?;
            *guard = Some(storage);
        }
        Ok(guard)
    }
}

impl Default for AppState {
    fn default() -> Self {
        let db_path = dirs_local_data_dir().join("lnwdeck").join("lnwdeck.db");
        Self::new(db_path)
    }
}

fn dirs_local_data_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|_| PathBuf::from("."))
}
