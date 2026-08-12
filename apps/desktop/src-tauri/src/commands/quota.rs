use crate::commands::pipeline::{ensure_registry, load_or_create_hash_key};
use crate::state::AppState;
use lnwdeck_application::quota::QuotaDashboard;
use tauri::Manager;

/// Returns the normalized quota dashboard (latest report per provider,
/// display names and ordering resolved from the adapter registry).
///
/// Async on purpose: the query is a blocking SQLite read that can take
/// hundreds of milliseconds on a large database, and a synchronous command
/// would freeze the main thread (and with it the window) during the reload
/// burst right after a refresh cycle.
#[tauri::command]
pub async fn get_quota_dashboard(app: tauri::AppHandle) -> Result<QuotaDashboard, String> {
    let state = app.state::<AppState>();
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(&state, &hash_key)?;

    lnwdeck_application::quota::QueryQuotaDashboard::execute(&storage.conn, &registry)
        .map_err(|e| format!("quota dashboard: {e}"))
}
