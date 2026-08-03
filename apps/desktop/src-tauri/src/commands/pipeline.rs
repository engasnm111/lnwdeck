use crate::state::AppState;
use lnwdeck_provider_opencode::OpenCodeAdapter;
use lnwdeck_provider_runtime::{CollectionOutcome, ProviderAdapter};
use lnwdeck_storage::repositories::{
    AppSettingsRepository, CollectorRunRow, DiagnosticsRepository, PipelineTotals, ProviderStateRow,
};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct PipelineDiagnostics {
    pub app_version: String,
    pub db_ok: bool,
    pub integrity_ok: bool,
    pub migration_version: i64,
    pub total_events: i64,
    pub totals: PipelineTotals,
    pub providers: Vec<ProviderStateRow>,
    pub runs: Vec<CollectorRunRow>,
}

fn load_or_create_hash_key(conn: &rusqlite::Connection) -> Result<Vec<u8>, String> {
    let settings = AppSettingsRepository::new(conn);
    if let Some(stored) = settings.get("hash_key").map_err(|e| e.to_string())? {
        if stored.len() == 64 {
            if let Ok(bytes) = hex::decode(&stored) {
                return Ok(bytes);
            }
        }
    }
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| format!("random key: {e}"))?;
    settings
        .set("hash_key", &hex::encode(key))
        .map_err(|e| e.to_string())?;
    Ok(key.to_vec())
}

fn builtin_adapters(hash_key: &[u8]) -> Vec<Box<dyn ProviderAdapter>> {
    vec![Box::new(OpenCodeAdapter::new(hash_key))]
}

/// Runs detection and collection for every registered adapter, then
/// returns the sanitized outcomes.
pub fn refresh_now(state: &AppState) -> Result<Vec<CollectionOutcome>, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;

    let mut adapter_guard = state.adapters.lock().map_err(|e| e.to_string())?;
    if adapter_guard.is_empty() {
        *adapter_guard = builtin_adapters(&hash_key);
    }
    let adapters = adapter_guard;
    let refs: Vec<&dyn ProviderAdapter> = adapters.iter().map(|a| a.as_ref()).collect();
    Ok(lnwdeck_application::refresh::RefreshAll::execute(
        &storage.conn,
        &refs,
    ))
}

#[tauri::command]
pub fn refresh_all(state: State<'_, AppState>) -> Result<Vec<CollectionOutcome>, String> {
    refresh_now(&state)
}

#[tauri::command]
pub fn get_pipeline_diagnostics(state: State<'_, AppState>) -> Result<PipelineDiagnostics, String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let diag = DiagnosticsRepository::new(&storage.conn);
    let totals = diag.pipeline_totals().map_err(|e| e.to_string())?;
    let providers = diag.provider_states().map_err(|e| e.to_string())?;
    let runs = diag.latest_runs().map_err(|e| e.to_string())?;
    let migration_version = diag.migration_version().map_err(|e| e.to_string())?;
    let total_events: i64 = storage
        .conn
        .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let integrity_ok = storage.integrity_check().is_ok();

    Ok(PipelineDiagnostics {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        db_ok: true,
        integrity_ok,
        migration_version,
        total_events,
        totals,
        providers,
        runs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use lnwdeck_domain::{Confidence, QuotaSnapshot, UsageBatch, UsageEvent};
    use lnwdeck_provider_runtime::{
        AdapterHealth, AdapterHealthStatus, DetectionResult, Permission,
    };
    use tempfile::tempdir;

    struct FakeAdapter;

    impl ProviderAdapter for FakeAdapter {
        fn id(&self) -> &str {
            "fake_provider"
        }
        fn name(&self) -> &str {
            "Fake Provider"
        }
        fn collect_usage(&self) -> Result<UsageBatch, String> {
            Ok(UsageBatch {
                batch_id: "fake".to_string(),
                events: vec![
                    UsageEvent {
                        id: "evt_1".to_string(),
                        timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                        provider_id: "fake_provider".to_string(),
                        model: "model-a".to_string(),
                        tokens_input: 100,
                        tokens_output: 50,
                        confidence: Confidence::High,
                        data_source: "fixture".to_string(),
                        cost: "0.001".to_string(),
                    },
                    UsageEvent {
                        id: "evt_2".to_string(),
                        timestamp: DateTime::from_timestamp(1_700_000_100, 0).unwrap(),
                        provider_id: "fake_provider".to_string(),
                        model: "model-b".to_string(),
                        tokens_input: 200,
                        tokens_output: 60,
                        confidence: Confidence::High,
                        data_source: "fixture".to_string(),
                        cost: "0.002".to_string(),
                    },
                ],
            })
        }
        fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
            Ok(None)
        }
        fn health_check(&self) -> AdapterHealth {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "ok".to_string(),
            }
        }
        fn required_permissions(&self) -> Vec<Permission> {
            vec![]
        }
        fn detect(&self) -> Result<DetectionResult, String> {
            Ok(DetectionResult {
                provider_id: "fake_provider".to_string(),
                display_name: "Fake Provider".to_string(),
                enabled: true,
                detected: true,
                detection_method: "fixture".to_string(),
                source_type: "fixture".to_string(),
                source_exists: true,
                permission_state: "read_ok".to_string(),
                adapter_version: "0.1.0".to_string(),
                last_detection_at: Some("2026-08-03T00:00:00Z".to_string()),
                detection_error_code: String::new(),
            })
        }
    }

    fn test_state() -> AppState {
        // Keep the temp directory alive for the whole test process: storage
        // opens lazily, so the directory must outlive this function.
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let db_path = dir.path().join("test.db");
        let state = AppState::new(db_path);
        *state.adapters.lock().expect("lock") = vec![Box::new(FakeAdapter)];
        state
    }

    #[test]
    fn hash_key_is_created_once_and_stable() {
        let state = test_state();
        let guard = state.ensure_storage().expect("storage");
        let storage = guard.as_ref().expect("storage value");
        let first = load_or_create_hash_key(&storage.conn).expect("first");
        let second = load_or_create_hash_key(&storage.conn).expect("second");
        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn refresh_all_records_evidence_and_ingests() {
        let state = test_state();
        let outcomes = refresh_now(&state).expect("refresh");
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].events_inserted, 2);
        assert_eq!(outcomes[0].error_code, "");

        let guard = state.ensure_storage().expect("storage");
        let storage = guard.as_ref().expect("storage value");
        let diag = DiagnosticsRepository::new(&storage.conn);
        let totals = diag.pipeline_totals().expect("totals");
        assert_eq!(totals.events_inserted, 2);
        assert!(totals.last_successful_sync.is_some());
        assert_eq!(diag.provider_states().expect("states").len(), 1);
        assert_eq!(diag.latest_runs().expect("runs").len(), 1);
        assert_eq!(diag.migration_version().expect("migration"), 3);
    }

    #[test]
    fn refresh_twice_is_idempotent() {
        let state = test_state();
        refresh_now(&state).expect("refresh 1");
        let outcomes = refresh_now(&state).expect("refresh 2");
        assert_eq!(outcomes[0].duplicates_skipped, 2);
        assert_eq!(outcomes[0].events_inserted, 0);
    }

    #[test]
    fn diagnostics_reports_empty_pipeline_for_fresh_database() {
        let state = test_state();
        let guard = state.ensure_storage().expect("storage");
        let storage = guard.as_ref().expect("storage value");
        let diag = DiagnosticsRepository::new(&storage.conn);
        let totals = diag.pipeline_totals().expect("totals");
        assert_eq!(totals.events_seen, 0);
        assert_eq!(totals.events_inserted, 0);
        assert_eq!(diag.provider_states().expect("states").len(), 0);
        assert_eq!(diag.latest_runs().expect("runs").len(), 0);
    }
}
