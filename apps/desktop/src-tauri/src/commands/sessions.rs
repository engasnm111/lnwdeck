use crate::state::AppState;
use lnwdeck_application::sessions::{
    QuerySessions, RenameProject, RenameSession, SessionsOverview,
};
use lnwdeck_application::usage_history::HistoryWindow;
use tauri::Manager;

/// Async on purpose: blocking SQLite work on the main thread would freeze the
/// window during the reload burst after a refresh cycle.
#[tauri::command]
pub async fn get_sessions(
    app: tauri::AppHandle,
    window: Option<String>,
    provider_id: Option<String>,
) -> Result<SessionsOverview, String> {
    let state = app.state::<AppState>();
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;

    let parsed = window
        .as_deref()
        .and_then(HistoryWindow::parse)
        .unwrap_or(HistoryWindow::All);
    QuerySessions::execute(&storage.conn, parsed, provider_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn rename_session(
    app: tauri::AppHandle,
    session_hash: String,
    display_name: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    RenameSession::execute(&storage.conn, &session_hash, &display_name)
}

#[tauri::command]
pub async fn rename_project(
    app: tauri::AppHandle,
    project_hash: String,
    display_name: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    RenameProject::execute(&storage.conn, &project_hash, &display_name)
}
