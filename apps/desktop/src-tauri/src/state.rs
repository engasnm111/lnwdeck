use lnwdeck_storage::Storage;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub storage: Mutex<Option<Storage>>,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            storage: Mutex::new(None),
            db_path,
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
        let db_path = dirs_document_dir().join("lnwdeck").join("lnwdeck.db");
        Self::new(db_path)
    }
}

fn dirs_document_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
