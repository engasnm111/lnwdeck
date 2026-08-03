use crate::commands::pipeline::{ensure_registry, load_or_create_hash_key};
use crate::state::AppState;
use lnwdeck_application::quota::QuotaDashboard;
use tauri::State;

/// Returns the normalized quota dashboard (latest report per provider,
/// display names and ordering resolved from the adapter registry).
#[tauri::command]
pub fn get_quota_dashboard(state: State<'_, AppState>) -> Result<QuotaDashboard, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(&state, &hash_key)?;

    lnwdeck_application::quota::QueryQuotaDashboard::execute(&storage.conn, &registry)
        .map_err(|e| format!("quota dashboard: {e}"))
}
