use crate::state::AppState;
use lnwdeck_application::dashboard::{DashboardQuery, QueryDashboard, UsageDashboard};
use tauri::State;

/// Returns the TokenTracker-style usage read model for the requested range.
///
/// The application query applies the provider filter to every section and
/// converts user-calendar boundaries to UTC before touching SQLite.
#[tauri::command]
pub fn get_usage_dashboard(
    state: State<'_, AppState>,
    query: DashboardQuery,
) -> Result<UsageDashboard, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    QueryDashboard::execute(&storage.conn, query).map_err(|error| error.to_string())
}
