use lnwdeck_storage::Storage;
use std::path::PathBuf;
use std::sync::Mutex;

use lnwdeck_provider_runtime::AdapterRegistry;

pub struct AppState {
    pub storage: Mutex<Option<Storage>>,
    pub db_path: PathBuf,
    /// Provider registry, built once on first use. It is the single source of
    /// provider identity for every read model.
    pub registry: Mutex<AdapterRegistry>,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            storage: Mutex::new(None),
            db_path,
            registry: Mutex::new(AdapterRegistry::new()),
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
