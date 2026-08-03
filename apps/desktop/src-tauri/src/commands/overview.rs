use crate::state::AppState;
use inwdeck_application::overview::QueryOverview;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct OverviewResponse {
    pub total_events: i64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub provider_count: i64,
    pub high_confidence_count: i64,
    pub confidence_coverage: f64,
    pub latest_event_at: Option<String>,
    pub oldest_event_at: Option<String>,
}

#[tauri::command]
pub fn get_overview(state: State<'_, AppState>) -> Result<OverviewResponse, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;

    let result = QueryOverview::execute(&storage.conn).map_err(|e| e.to_string())?;

    Ok(OverviewResponse {
        total_events: result.total_events,
        total_tokens_input: result.total_tokens_input,
        total_tokens_output: result.total_tokens_output,
        provider_count: result.provider_count,
        high_confidence_count: result.high_confidence_count,
        confidence_coverage: result.confidence_coverage,
        latest_event_at: result.latest_event_at,
        oldest_event_at: result.oldest_event_at,
    })
}

#[cfg(test)]
mod tests {
    use inwdeck_application::overview::QueryOverview;
    use inwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    #[test]
    fn overview_command_returns_empty_state() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = Storage::open(&db_path).unwrap();
        apply_all(&storage.conn).unwrap();

        let result = QueryOverview::execute(&storage.conn).unwrap();
        assert_eq!(result.total_events, 0);
    }
}
