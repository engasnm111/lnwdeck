use crate::state::AppState;
use lnwdeck_application::providers::{DetailedProviderInfo, ScanProviders};
use tauri::State;

#[tauri::command]
pub fn get_providers(state: State<'_, AppState>) -> Result<Vec<DetailedProviderInfo>, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;

    ScanProviders::execute(&storage.conn).map_err(|e| e.to_string())
}
