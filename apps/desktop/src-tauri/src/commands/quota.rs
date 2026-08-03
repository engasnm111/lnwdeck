use crate::state::AppState;
use lnwdeck_application::quota::QuotaDashboard;
use tauri::State;

/// Returns the normalized quota dashboard (latest report per provider,
/// resolved display names, status and windows).
#[tauri::command]
pub fn get_quota_dashboard(state: State<'_, AppState>) -> Result<QuotaDashboard, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    lnwdeck_application::quota::QueryQuotaDashboard::execute(&storage.conn)
        .map_err(|e| format!("quota dashboard: {e}"))
}
