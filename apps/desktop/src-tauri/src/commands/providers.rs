use crate::commands::pipeline::{ensure_registry, load_or_create_hash_key};
use crate::state::AppState;
use lnwdeck_application::providers::{DetailedProviderInfo, ScanProviders};
use tauri::Manager;

/// One card per registered provider, built from stored detection, run, quota
/// and usage state. No live network or filesystem probing happens here, so the
/// page cannot block on an unreachable provider.
///
/// The blocking body runs inside `spawn_blocking` so the future stays `Send`
/// and Tauri executes it off the main thread. Holding the `MutexGuard` from
/// `ensure_storage()` directly would make the future `!Send` and freeze the
/// window during the reload burst after a refresh cycle.
#[tauri::command]
pub async fn get_providers(app: tauri::AppHandle) -> Result<Vec<DetailedProviderInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let storage_guard = state.ensure_storage()?;
        let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
        let hash_key = load_or_create_hash_key(&storage.conn)?;
        let registry = ensure_registry(&state, &hash_key)?;

        ScanProviders::execute(&storage.conn, &registry).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| format!("providers task failed: {error}"))?
}
