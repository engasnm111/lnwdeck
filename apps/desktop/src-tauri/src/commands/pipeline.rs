use crate::state::AppState;
use lnwdeck_application::refresh::RefreshCycleOutcome;
use lnwdeck_provider_claude::ClaudeAdapter;
use lnwdeck_provider_codebuddy::CodebuddyAdapter;
use lnwdeck_provider_codex::CodexAdapter;
use lnwdeck_provider_copilot::CopilotAdapter;
use lnwdeck_provider_cursor::CursorAdapter;
use lnwdeck_provider_gemini::GeminiAdapter;
use lnwdeck_provider_grok::GrokAdapter;
use lnwdeck_provider_hermes::HermesAdapter;
use lnwdeck_provider_kilo_cli::KiloCliAdapter;
use lnwdeck_provider_kilo_code::KiloCodeAdapter;
use lnwdeck_provider_kimi::KimiAdapter;
use lnwdeck_provider_kiro::KiroAdapter;
use lnwdeck_provider_mimo::MimoAdapter;
use lnwdeck_provider_ollama::OllamaAdapter;
use lnwdeck_provider_omp::OmpAdapter;
use lnwdeck_provider_opencode::OpenCodeAdapter;
use lnwdeck_provider_openrouter::OpenRouterAdapter;
use lnwdeck_provider_pi_agent::PiAdapter;
use lnwdeck_provider_roo::RooAdapter;
use lnwdeck_provider_runtime::{AdapterRegistry, ProviderAdapter};
use lnwdeck_provider_workbuddy::WorkbuddyAdapter;
use lnwdeck_provider_zai::ZaiAdapter;
use lnwdeck_provider_zcode::ZCodeAdapter;
use lnwdeck_storage::repositories::{
    AppSettingsRepository, CollectorRunRow, DiagnosticsRepository, PipelineTotals, ProviderStateRow,
};
use serde::Serialize;
use tauri::{Emitter, Manager, State};

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

const MANUAL_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Progress emitted by the shared background refresh job. The payload is
/// deliberately metadata-only: it contains provider ids and outcome codes,
/// never prompts, responses, paths or credentials.
#[derive(Debug, Clone, Serialize)]
pub struct RefreshProgressEvent {
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub provider_id: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshStartResult {
    pub started: bool,
    pub already_running: bool,
}

struct RefreshRunningGuard<'a>(&'a std::sync::atomic::AtomicBool);

impl Drop for RefreshRunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub fn load_or_create_hash_key(conn: &rusqlite::Connection) -> Result<Vec<u8>, String> {
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

/// Builds the provider registry.
///
/// Registration validates each descriptor and rejects duplicate ids, so a
/// broken adapter surfaces here instead of silently shadowing another
/// provider. The returned error is reported to the caller, never swallowed.
fn build_registry(hash_key: &[u8]) -> Result<AdapterRegistry, String> {
    let adapters: Vec<Box<dyn ProviderAdapter>> = vec![
        Box::new(OpenCodeAdapter::new(hash_key)),
        Box::new(CodexAdapter::new()),
        Box::new(ClaudeAdapter::new()),
        Box::new(GeminiAdapter::new()),
        Box::new(CursorAdapter::new()),
        Box::new(CopilotAdapter::new()),
        Box::new(KiroAdapter::new()),
        Box::new(KimiAdapter::new()),
        Box::new(KiloCliAdapter::new()),
        Box::new(KiloCodeAdapter::new()),
        Box::new(MimoAdapter::new()),
        Box::new(RooAdapter::new()),
        Box::new(CodebuddyAdapter::new()),
        Box::new(WorkbuddyAdapter::new()),
        Box::new(PiAdapter::new()),
        Box::new(OmpAdapter::new()),
        Box::new(HermesAdapter::new()),
        Box::new(ZaiAdapter::new()),
        Box::new(ZCodeAdapter::new()),
        Box::new(OllamaAdapter::new()),
        Box::new(OpenRouterAdapter::new()),
        Box::new(GrokAdapter::new()),
    ];
    let mut registry = AdapterRegistry::new();
    for adapter in adapters {
        let id = adapter.id();
        registry
            .register(adapter)
            .map_err(|err| format!("adapter {id} could not be registered: {err}"))?;
    }
    Ok(registry)
}

/// Returns the registry, building it on first use.
pub fn ensure_registry<'a>(
    state: &'a AppState,
    hash_key: &[u8],
) -> Result<std::sync::MutexGuard<'a, AdapterRegistry>, String> {
    let mut guard = state.registry.lock().map_err(|e| e.to_string())?;
    if guard.is_empty() {
        *guard = build_registry(hash_key)?;
    }
    Ok(guard)
}

/// Runs detection and collection for every registered adapter, then
/// returns the sanitized usage and quota outcomes.
///
/// Collection can take many seconds (network calls, transcript reads). It runs
/// on its own SQLite connection from a worker thread, so the shared storage
/// mutex — which every UI command locks — is never held during a cycle.
pub fn refresh_now(state: &AppState) -> Result<RefreshCycleOutcome, String> {
    refresh_now_with_progress(state, |_, _, _| true)
}

/// Runs a refresh and reports provider completion to the caller. The callback
/// is invoked from the worker thread and may stop the cycle before the next
/// provider by returning `false`.
pub fn refresh_now_with_progress<F>(
    state: &AppState,
    mut on_progress: F,
) -> Result<RefreshCycleOutcome, String>
where
    F: FnMut(&str, usize, usize) -> bool,
{
    let hash_key = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        load_or_create_hash_key(&storage.conn)?
    };
    let storage =
        lnwdeck_storage::Storage::open(&state.db_path).map_err(|e| format!("storage open: {e}"))?;
    let registry = ensure_registry(state, &hash_key)?;
    Ok(
        lnwdeck_application::refresh::RefreshAll::execute_with_progress_and_hash_key(
            &storage.conn,
            &registry.refs(),
            &hash_key,
            |provider_id, completed, total| {
                let keep_running = on_progress(provider_id, completed, total);
                keep_running
                    && !state
                        .refresh_cancel_requested
                        .load(std::sync::atomic::Ordering::SeqCst)
            },
        ),
    )
}

/// Re-evaluates alerts from current state. Used by the background loop after a
/// refresh cycle; returns the failure reason instead of swallowing it.
pub fn evaluate_alerts_now(state: &AppState) -> Result<(), String> {
    let storage_guard = state.ensure_storage()?;
    let storage = storage_guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(state, &hash_key)?;
    let overrides = AppSettingsRepository::new(&storage.conn)
        .get("pricing_overrides")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!([]));
    let resolver = lnwdeck_pricing::catalog::PriceResolver::new_with_overrides(&overrides);
    let display_name = |id: &str| registry.display_name(id).unwrap_or(id).to_string();
    lnwdeck_application::alerts::EvaluateAlerts::execute(&storage.conn, &resolver, &display_name)
        .map(|_| ())
        .map_err(|e| format!("alert evaluation: {e}"))
}

#[tauri::command]
pub async fn refresh_all(app: tauri::AppHandle) -> Result<RefreshCycleOutcome, String> {
    // Never run two cycles at once: overlapping collection (network reads,
    // transcript scans) piles up and can freeze the machine. A busy cycle
    // reports immediately instead of queueing.
    {
        let state = app.state::<AppState>();
        if state
            .refresh_running
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            return Err("refresh already in progress".to_string());
        }
    }
    app.state::<AppState>()
        .refresh_cancel_requested
        .store(false, std::sync::atomic::Ordering::SeqCst);
    // Collection touches the network and reads tool transcripts: it must never
    // run on the UI thread or the window freezes and Windows reports it as
    // unresponsive. The blocking work runs on the async runtime's pool, and a
    // hard timeout guarantees the command always answers even if a collector
    // hangs.
    let app_work = app.clone();
    let cycle = match tokio::time::timeout(
        MANUAL_REFRESH_TIMEOUT,
        tauri::async_runtime::spawn_blocking(move || -> Result<RefreshCycleOutcome, String> {
            let state = app_work.state::<AppState>();
            let _running_guard = RefreshRunningGuard(&state.refresh_running);
            refresh_now(&state)
        }),
    )
    .await
    {
        Ok(Ok(inner)) => inner,
        Ok(Err(e)) => Err(format!("refresh task failed: {e}")),
        Err(_) => Err(format!(
            "refresh timed out after {} seconds",
            MANUAL_REFRESH_TIMEOUT.as_secs()
        )),
    };
    let cycle = cycle?;
    crate::record_sync_time(&app);
    crate::tray::update_sync_label(&app);
    let _ = app.emit("quota-updated", ());
    Ok(cycle)
}

fn emit_refresh_event(
    app: &tauri::AppHandle,
    phase: &str,
    completed: usize,
    total: usize,
    provider_id: Option<String>,
    error_code: Option<String>,
) {
    let _ = app.emit(
        "refresh-progress",
        RefreshProgressEvent {
            phase: phase.to_string(),
            completed,
            total,
            provider_id,
            error_code,
        },
    );
}

fn cycle_has_provider_failure(cycle: &RefreshCycleOutcome) -> bool {
    cycle.usage.iter().any(|outcome| {
        !outcome.error_code.is_empty()
            && !outcome.is_not_supported()
            && !is_expected_provider_absence(&outcome.error_code)
    }) || cycle.quota.iter().any(|outcome| {
        !outcome.error_code.is_empty()
            && outcome.error_code != "NOT_SUPPORTED"
            && !is_expected_provider_absence(&outcome.error_code)
    })
}

/// A multi-machine installation normally has providers that are not installed
/// or not configured on every machine. Those outcomes are represented on the
/// provider card and must not turn the shared refresh job into a global error.
/// `SOURCE_REQUIRES_IDE` is the same kind of expected state: the quota source
/// exists but needs the Antigravity IDE running.
fn is_expected_provider_absence(code: &str) -> bool {
    matches!(
        code,
        "SOURCE_UNAVAILABLE"
            | "NOT_INSTALLED"
            | "NOT_CONFIGURED"
            | "UNSUPPORTED"
            | "NOT_SUPPORTED"
            | "SOURCE_REQUIRES_IDE"
    )
}

/// Starts the same refresh job used by the tray and every UI surface. It
/// returns immediately; progress and completion arrive through
/// `refresh-progress`, so WebView2 never waits on provider I/O.
#[tauri::command]
pub fn start_refresh(app: tauri::AppHandle) -> Result<RefreshStartResult, String> {
    let state = app.state::<AppState>();
    if state
        .refresh_running
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return Ok(RefreshStartResult {
            started: false,
            already_running: true,
        });
    }
    state
        .refresh_cancel_requested
        .store(false, std::sync::atomic::Ordering::SeqCst);
    emit_refresh_event(&app, "started", 0, 0, None, None);

    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let timeout_result = tokio::time::timeout(
            MANUAL_REFRESH_TIMEOUT,
            tauri::async_runtime::spawn_blocking({
                let app = worker_app.clone();
                move || -> Result<RefreshCycleOutcome, String> {
                    let state = app.state::<AppState>();
                    let _running_guard = RefreshRunningGuard(&state.refresh_running);
                    refresh_now_with_progress(&state, |provider_id, completed, total| {
                        emit_refresh_event(
                            &app,
                            "progress",
                            completed,
                            total,
                            Some(provider_id.to_string()),
                            None,
                        );
                        // Each persisted provider is immediately visible: windows
                        // reload incrementally (debounced) instead of waiting for
                        // the whole cycle and blanking behind a spinner.
                        let _ = app.emit("quota-updated", ());
                        let _ = app.emit("usage-updated", ());
                        !state
                            .refresh_cancel_requested
                            .load(std::sync::atomic::Ordering::SeqCst)
                    })
                }
            }),
        )
        .await;

        let result = match timeout_result {
            Ok(Ok(inner)) => inner,
            Ok(Err(error)) => Err(format!("refresh task failed: {error}")),
            Err(_) => Err(format!(
                "refresh timed out after {} seconds",
                MANUAL_REFRESH_TIMEOUT.as_secs()
            )),
        };

        match result {
            Ok(cycle) => {
                let state = worker_app.state::<AppState>();
                let cancelled = state
                    .refresh_cancel_requested
                    .load(std::sync::atomic::Ordering::SeqCst);
                crate::record_sync_time(&worker_app);
                crate::tray::update_sync_label(&worker_app);
                let _ = worker_app.emit("quota-updated", ());
                let _ = worker_app.emit("usage-updated", ());
                let _ = evaluate_alerts_now(&state);
                let phase = if cancelled || cycle_has_provider_failure(&cycle) {
                    "partial"
                } else {
                    "completed"
                };
                emit_refresh_event(
                    &worker_app,
                    phase,
                    cycle.usage.len(),
                    cycle.usage.len().max(cycle.quota.len()),
                    None,
                    None,
                );
            }
            Err(error) => {
                emit_refresh_event(&worker_app, "failed", 0, 0, None, Some(error));
            }
        }
        worker_app
            .state::<AppState>()
            .refresh_cancel_requested
            .store(false, std::sync::atomic::Ordering::SeqCst);
    });

    Ok(RefreshStartResult {
        started: true,
        already_running: false,
    })
}

/// Requests a cooperative stop between provider jobs. A provider already in
/// progress is allowed to finish so its data remains consistent.
#[tauri::command]
pub fn cancel_refresh(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .refresh_cancel_requested
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// Refreshes a single provider by id (detection + usage + quota channels).
/// Returns an error for unknown provider ids.
#[tauri::command]
pub async fn refresh_provider(
    provider_id: String,
    app: tauri::AppHandle,
) -> Result<RefreshCycleOutcome, String> {
    let app_work = app.clone();
    let cycle = match tauri::async_runtime::spawn_blocking(
        move || -> Result<RefreshCycleOutcome, String> {
            let state = app_work.state::<AppState>();
            let hash_key = {
                let guard = state.ensure_storage()?;
                let storage = guard.as_ref().ok_or("storage not initialized")?;
                load_or_create_hash_key(&storage.conn)?
            };
            let storage = lnwdeck_storage::Storage::open(&state.db_path)
                .map_err(|e| format!("storage open: {e}"))?;
            let registry = ensure_registry(&state, &hash_key)?;
            let adapter = registry
                .find(&provider_id)
                .ok_or_else(|| format!("unknown provider: {provider_id}"))?;
            Ok(
                lnwdeck_application::refresh::RefreshAll::refresh_provider_with_hash_key(
                    &storage.conn,
                    adapter,
                    &hash_key,
                ),
            )
        },
    )
    .await
    {
        Ok(inner) => inner?,
        Err(e) => return Err(format!("refresh task failed: {e}")),
    };
    let _ = app.emit("quota-updated", ());
    Ok(cycle)
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

/// The user's Downloads folder, or the profile directory as a fallback.
fn downloads_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
        // FOLDERID_Downloads = {374DE290-123F-4565-9164-39C4925E467B}
        const FOLDERID_DOWNLOADS: windows_sys::core::GUID =
            windows_sys::core::GUID::from_u128(0x374de290_123f_4565_9164_39c4925e467b);
        let mut ptr: *mut u16 = std::ptr::null_mut();
        let hr =
            unsafe { SHGetKnownFolderPath(&FOLDERID_DOWNLOADS, 0, std::ptr::null_mut(), &mut ptr) };
        if hr == 0 && !ptr.is_null() {
            let len = (0..).take_while(|&i| unsafe { *ptr.add(i) } != 0).count();
            let wide = unsafe { std::slice::from_raw_parts(ptr, len) };
            let path = std::path::PathBuf::from(String::from_utf16_lossy(wide));
            unsafe { CoTaskMemFree(ptr as *mut _) };
            if path.is_dir() {
                return Ok(path);
            }
        }
    }
    std::env::var("USERPROFILE")
        .map(|profile| std::path::PathBuf::from(profile).join("Downloads"))
        .map_err(|_| "could not locate the Downloads folder".to_string())
}

/// Writes a sanitized diagnostics snapshot as JSON to the user's Downloads
/// folder and returns the file path.
#[tauri::command]
pub fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    let diagnostics = get_pipeline_diagnostics(state)?;
    let json = serde_json::to_string_pretty(&diagnostics).map_err(|e| e.to_string())?;
    let downloads = downloads_dir()?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = downloads.join(format!("lnwdeck-diagnostics-{stamp}.json"));
    std::fs::write(&path, json).map_err(|e| format!("write diagnostics: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// Opens the file explorer with the given file selected.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
    use lnwdeck_provider_runtime::{
        AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
        DetectionResult, Permission, SourceKind,
    };
    use tempfile::tempdir;

    struct FakeAdapter;

    impl ProviderAdapter for FakeAdapter {
        fn descriptor(&self) -> AdapterDescriptor {
            AdapterDescriptor {
                id: "fake_provider",
                display_name: "Fake Provider",
                vendor: "Fixture",
                source_kind: SourceKind::LocalJsonl,
                usage_support: ChannelSupport::LocalEstimate,
                quota_support: ChannelSupport::Unsupported,
                auth: AuthKind::LocalFiles,
                adapter_version: "0.2.0",
            }
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
                        tokens_cached: 0,
                        tokens_cache_write: 0,
                        tokens_output: 50,
                        tokens_reasoning: 0,
                        confidence: Confidence::High,
                        data_source: "fixture".to_string(),
                        cost: "0.001".to_string(),
                        session_hash: None,
                        project_hash: None,
                        account_fingerprint: None,
                    },
                    UsageEvent {
                        id: "evt_2".to_string(),
                        timestamp: DateTime::from_timestamp(1_700_000_100, 0).unwrap(),
                        provider_id: "fake_provider".to_string(),
                        model: "model-b".to_string(),
                        tokens_input: 200,
                        tokens_cached: 0,
                        tokens_cache_write: 0,
                        tokens_output: 60,
                        tokens_reasoning: 0,
                        confidence: Confidence::High,
                        data_source: "fixture".to_string(),
                        cost: "0.002".to_string(),
                        session_hash: None,
                        project_hash: None,
                        account_fingerprint: None,
                    },
                ],
            })
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
                adapter_version: "0.2.0".to_string(),
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
        {
            let mut registry = state.registry.lock().expect("lock");
            registry
                .register(Box::new(FakeAdapter))
                .expect("register fixture adapter");
        }
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
    fn manual_refresh_timeout_is_bounded_for_ui_responsiveness() {
        assert_eq!(MANUAL_REFRESH_TIMEOUT.as_secs(), 60);
    }

    #[test]
    fn refresh_all_records_evidence_and_ingests() {
        let state = test_state();
        let cycle = refresh_now(&state).expect("refresh");
        assert_eq!(cycle.usage.len(), 1);
        assert_eq!(cycle.usage[0].events_inserted, 2);
        assert_eq!(cycle.usage[0].error_code, "");
        assert_eq!(cycle.quota.len(), 1);
        assert_eq!(cycle.quota[0].error_code, "NOT_SUPPORTED");

        let guard = state.ensure_storage().expect("storage");
        let storage = guard.as_ref().expect("storage value");
        let diag = DiagnosticsRepository::new(&storage.conn);
        let totals = diag.pipeline_totals().expect("totals");
        assert_eq!(totals.events_inserted, 2);
        assert!(totals.last_successful_sync.is_some());
        assert_eq!(diag.provider_states().expect("states").len(), 1);
        assert_eq!(diag.latest_runs().expect("runs").len(), 2);
        assert_eq!(
            diag.migration_version().expect("migration") as usize,
            lnwdeck_storage::migrations::known_migrations().len(),
            "every migration shipped in this build must be recorded"
        );
    }

    #[test]
    fn refresh_twice_is_idempotent() {
        let state = test_state();
        refresh_now(&state).expect("refresh 1");
        let cycle = refresh_now(&state).expect("refresh 2");
        assert_eq!(cycle.usage[0].duplicates_skipped, 2);
        assert_eq!(cycle.usage[0].events_inserted, 0);
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

    #[test]
    fn provider_absence_codes_do_not_make_the_shared_refresh_partial() {
        for code in [
            "SOURCE_UNAVAILABLE",
            "NOT_INSTALLED",
            "NOT_CONFIGURED",
            "UNSUPPORTED",
            "NOT_SUPPORTED",
            "SOURCE_REQUIRES_IDE",
        ] {
            assert!(
                is_expected_provider_absence(code),
                "{code} should stay local to its provider card"
            );
        }
    }

    #[test]
    fn authentication_and_schema_errors_still_make_the_shared_refresh_partial() {
        for code in [
            "AUTH_EXPIRED",
            "PERMISSION_DENIED",
            "SOURCE_SCHEMA_MISMATCH",
        ] {
            assert!(
                !is_expected_provider_absence(code),
                "{code} must remain visible as a real refresh problem"
            );
        }
    }
}
