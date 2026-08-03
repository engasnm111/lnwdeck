use crate::commands::pipeline::{ensure_registry, load_or_create_hash_key};
use crate::state::AppState;
use lnwdeck_application::providers::{DetailedProviderInfo, ScanProviders};
use tauri::State;

/// One card per registered provider, built from stored detection, run, quota
/// and usage state. No live network or filesystem probing happens here, so the
/// page cannot block on an unreachable provider.
#[tauri::command]
pub fn get_providers(state: State<'_, AppState>) -> Result<Vec<DetailedProviderInfo>, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(&state, &hash_key)?;

    ScanProviders::execute(&storage.conn, &registry).map_err(|e| e.to_string())
}
