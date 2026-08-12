use crate::state::AppState;
use lnwdeck_application::analytics::{AnalyticsFilter, AnalyticsResult, QueryAnalytics};
use lnwdeck_pricing::catalog::PriceResolver;
use lnwdeck_storage::repositories::AppSettingsRepository;
use tauri::Manager;

fn price_resolver(conn: &rusqlite::Connection) -> PriceResolver {
    let overrides = AppSettingsRepository::new(conn)
        .get("pricing_overrides")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!([]));
    PriceResolver::new_with_overrides(&overrides)
}

/// The blocking body runs inside `spawn_blocking` so the future stays `Send`
/// and Tauri executes it off the main thread. Holding the `MutexGuard` from
/// `ensure_storage()` directly would make the future `!Send` and freeze the
/// window during the reload burst after a refresh cycle.
#[tauri::command]
pub async fn get_analytics(
    app: tauri::AppHandle,
    filter: Option<AnalyticsFilter>,
) -> Result<AnalyticsResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let storage_guard = state.ensure_storage()?;
        let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
        let resolver = price_resolver(&storage.conn);

        QueryAnalytics::execute(&storage.conn, filter.unwrap_or_default(), &resolver)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| format!("analytics task failed: {error}"))?
}
