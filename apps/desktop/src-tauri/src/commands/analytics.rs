use crate::state::AppState;
use lnwdeck_application::analytics::{AnalyticsFilter, AnalyticsResult, QueryAnalytics};
use tauri::State;

#[tauri::command]
pub fn get_analytics(
    state: State<'_, AppState>,
    filter: Option<AnalyticsFilter>,
) -> Result<AnalyticsResult, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;

    QueryAnalytics::execute(&storage.conn, filter.unwrap_or_default()).map_err(|e| e.to_string())
}
