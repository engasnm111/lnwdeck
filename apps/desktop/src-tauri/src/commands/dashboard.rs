use crate::commands::pipeline::{ensure_registry, load_or_create_hash_key};
use crate::state::AppState;
use lnwdeck_application::dashboard::{DashboardQuery, QueryDashboard, UsageDashboard};
use tauri::Manager;

/// Returns the TokenTracker-style usage read model for the requested range.
///
/// The application query applies the provider filter to every section and
/// converts user-calendar boundaries to UTC before touching SQLite.
///
/// Async on purpose: the read model runs four aggregating queries that can
/// take hundreds of milliseconds on a large database, and a synchronous
/// command would freeze the main thread (and with it the window) during the
/// reload burst right after a refresh cycle.
#[tauri::command]
pub async fn get_usage_dashboard(
    app: tauri::AppHandle,
    query: DashboardQuery,
) -> Result<UsageDashboard, String> {
    let state = app.state::<AppState>();
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(&state, &hash_key)?;
    QueryDashboard::execute_with_registry(&storage.conn, query, &registry)
        .map_err(|error| error.to_string())
}
