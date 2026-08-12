//! Commands backing the Costs, Models, Budgets, Alerts and Settings pages.
//!
//! Each command reads or writes real state and propagates failures to the
//! caller. None of them return a placeholder value, so a page can only render
//! data that exists or an explicit empty/error state.

use crate::commands::pipeline::{ensure_registry, load_or_create_hash_key};
use crate::state::AppState;
use lnwdeck_application::alerts::{AlertsView, EvaluateAlerts};
use lnwdeck_application::budgets::{BudgetOverview, QueryBudgets};
use lnwdeck_application::costs::{CostBreakdown, QueryCosts};
use lnwdeck_application::settings::{AppSettings, SettingsService};
use lnwdeck_application::usage_history::{HistoryWindow, QueryUsageHistory, UsageHistory};
use lnwdeck_domain::QuotaReport;
use lnwdeck_pricing::catalog::PriceResolver;
use lnwdeck_provider_opencode::{encode_go_config, go_config_state, OPENCODE_GO_CREDENTIAL_ID};
use lnwdeck_provider_runtime::AuthKind;
use lnwdeck_storage::repositories::{
    AlertRepository, AppEventRepository, BudgetPeriod, BudgetRepository, BudgetRow, BudgetScope,
    QuotaRepository,
};
use lnwdeck_windows_integration::{CredentialState, CredentialStore, StartupRegistration};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

/// Parses a window name, reporting the accepted values on a typo instead of
/// silently falling back to a different window.
fn parse_window(window: Option<String>) -> Result<HistoryWindow, String> {
    match window {
        None => Ok(HistoryWindow::Last30d),
        Some(value) => HistoryWindow::parse(&value).ok_or_else(|| {
            format!("unknown window \"{value}\": use last_24h, last_7d, last_30d or all")
        }),
    }
}

/// Pricing catalog plus user overrides stored in settings.
fn price_resolver(conn: &rusqlite::Connection) -> PriceResolver {
    let overrides = lnwdeck_storage::repositories::AppSettingsRepository::new(conn)
        .get("pricing_overrides")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!([]));
    PriceResolver::new_with_overrides(&overrides)
}

/// Async on purpose: a blocking SQLite read on the main thread would freeze
/// the window during the reload burst after a refresh cycle.
#[tauri::command]
pub async fn get_usage_history(
    app: tauri::AppHandle,
    window: Option<String>,
    provider_id: Option<String>,
) -> Result<UsageHistory, String> {
    let state = app.state::<AppState>();
    let window = parse_window(window)?;
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    QueryUsageHistory::execute(&storage.conn, window, provider_id.as_deref())
        .map_err(|e| format!("usage history: {e}"))
}

#[tauri::command]
pub async fn get_costs(
    app: tauri::AppHandle,
    window: Option<String>,
    provider_id: Option<String>,
) -> Result<CostBreakdown, String> {
    let state = app.state::<AppState>();
    let window = parse_window(window)?;
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let resolver = price_resolver(&storage.conn);
    QueryCosts::execute_with_provider(&storage.conn, window, provider_id.as_deref(), &resolver)
        .map_err(|e| format!("costs: {e}"))
}

#[tauri::command]
pub async fn get_budgets(app: tauri::AppHandle) -> Result<BudgetOverview, String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let resolver = price_resolver(&storage.conn);
    QueryBudgets::execute(&storage.conn, &resolver).map_err(|e| format!("budgets: {e}"))
}

/// Budget as submitted by the form.
#[derive(Debug, Clone, Deserialize)]
pub struct BudgetInput {
    /// "global" or "provider".
    pub scope: String,
    pub provider_id: Option<String>,
    /// "daily", "weekly" or "monthly".
    pub period: String,
    /// Decimal string; empty when only a token limit is set.
    pub cost_limit: String,
    pub token_limit: Option<u64>,
    pub warn_percent: u8,
    pub enabled: bool,
}

#[tauri::command]
pub async fn save_budget(budget: BudgetInput, app: tauri::AppHandle) -> Result<i64, String> {
    let state = app.state::<AppState>();
    let scope = match budget.scope.as_str() {
        "global" => BudgetScope::Global,
        "provider" => BudgetScope::Provider(
            budget
                .provider_id
                .clone()
                .ok_or("a provider budget needs a provider id")?,
        ),
        other => return Err(format!("unknown budget scope: {other}")),
    };
    let period = BudgetPeriod::parse(&budget.period)
        .ok_or_else(|| format!("unknown budget period: {}", budget.period))?;

    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    BudgetRepository::new(&storage.conn)
        .upsert(&BudgetRow {
            id: 0,
            scope,
            period,
            cost_limit: budget.cost_limit,
            token_limit: budget.token_limit,
            warn_percent: budget.warn_percent,
            enabled: budget.enabled,
            created_at: String::new(),
            updated_at: String::new(),
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_budget(id: i64, app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let removed = BudgetRepository::new(&storage.conn)
        .delete(id)
        .map_err(|e| e.to_string())?;
    if removed {
        Ok(())
    } else {
        Err(format!("budget {id} does not exist"))
    }
}

/// Async on purpose: a blocking SQLite read on the main thread would freeze
/// the window during the reload burst after a refresh cycle.
#[tauri::command]
pub async fn get_alerts(app: tauri::AppHandle) -> Result<AlertsView, String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(state.inner(), &hash_key)?;
    let resolver = price_resolver(&storage.conn);
    let display_name = |id: &str| registry.display_name(id).unwrap_or(id).to_string();
    EvaluateAlerts::execute(&storage.conn, &resolver, &display_name)
        .map_err(|e| format!("alerts: {e}"))
}

#[tauri::command]
pub async fn acknowledge_alert(id: i64, app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let updated = AlertRepository::new(&storage.conn)
        .acknowledge(id)
        .map_err(|e| e.to_string())?;
    if updated {
        Ok(())
    } else {
        Err(format!("alert {id} is unknown or already acknowledged"))
    }
}

/// Marks all open alerts as read in one storage transaction.
#[tauri::command]
pub async fn acknowledge_all_alerts(app: tauri::AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    AlertRepository::new(&storage.conn)
        .acknowledge_all_open()
        .map_err(|error| format!("mark all alerts read: {error}"))
}

/// Settings plus the platform facts the page needs to render honestly.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub settings: AppSettings,
    /// Whether this build can register a startup entry at all.
    pub startup_supported: bool,
    /// Whether Windows currently holds a startup entry, read from the registry.
    pub startup_registered: bool,
    /// Whether an API key can be stored on this platform.
    pub credential_store_supported: bool,
    /// Credential state per provider that requires a key.
    pub provider_credentials: Vec<ProviderCredentialState>,
    /// Secret-free state for the OpenCode Go workspace/cookie pair.
    pub opencode_go: OpenCodeGoCredentialState,
    pub allowed_refresh_intervals: Vec<u64>,
    pub allowed_themes: Vec<String>,
    pub allowed_retention_days: Vec<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderCredentialState {
    pub provider_id: String,
    pub display_name: String,
    /// "missing", "configured" or "expired". Never the key itself.
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenCodeGoCredentialState {
    /// "missing", "configured" or "expired". Never the workspace id or
    /// cookie itself.
    pub state: String,
}

fn credential_label(state: CredentialState) -> &'static str {
    match state {
        CredentialState::Missing => "missing",
        CredentialState::Configured => "configured",
        CredentialState::Expired => "expired",
    }
}

fn get_settings_view(state: &AppState) -> Result<SettingsView, String> {
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(state, &hash_key)?;

    let settings = SettingsService::load(&storage.conn).map_err(|e| e.to_string())?;
    let startup_supported = StartupRegistration::is_supported();
    // The registry is the source of truth; a read failure is reported.
    let startup_registered = if startup_supported {
        StartupRegistration::is_enabled().map_err(|e| e.to_string())?
    } else {
        false
    };

    let provider_credentials = registry
        .descriptors()
        .into_iter()
        .filter(|descriptor| descriptor.needs_credentials() && descriptor.auth == AuthKind::ApiKey)
        .map(|descriptor| ProviderCredentialState {
            provider_id: descriptor.id.to_string(),
            display_name: descriptor.display_name.to_string(),
            state: credential_label(CredentialStore::state(descriptor.id)).to_string(),
        })
        .collect();

    Ok(SettingsView {
        settings,
        startup_supported,
        startup_registered,
        credential_store_supported: CredentialStore::is_supported(),
        provider_credentials,
        opencode_go: OpenCodeGoCredentialState {
            state: go_config_state().to_string(),
        },
        allowed_refresh_intervals: lnwdeck_application::settings::ALLOWED_REFRESH_INTERVALS
            .to_vec(),
        allowed_themes: lnwdeck_application::settings::ALLOWED_THEMES
            .iter()
            .map(|value| value.to_string())
            .collect(),
        allowed_retention_days: lnwdeck_application::settings::ALLOWED_RETENTION_DAYS.to_vec(),
    })
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    get_settings_view(&state)
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    app: tauri::AppHandle,
) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;

        // Apply the startup entry first: if Windows refuses, nothing is saved
        // and the reported state stays truthful.
        if StartupRegistration::is_supported() {
            StartupRegistration::set_enabled(settings.launch_at_startup)
                .map_err(|e| e.to_string())?;
        } else if settings.launch_at_startup {
            return Err("launch at startup is not supported on this platform".to_string());
        }

        SettingsService::save(&storage.conn, &settings).map_err(|e| e.to_string())?;
    }
    // The widget size preset is stored with the rest of the settings, but it
    // only takes effect when the window is resized — do that here so saving
    // the form changes the widget immediately.
    crate::windows::apply_widget_size(&app);
    let view = get_settings_view(&state)?;
    let _ = app.emit("settings-changed", view.settings.theme.clone());
    Ok(view)
}

/// Stores an API key for a provider in the Windows Credential Manager.
#[tauri::command]
pub async fn set_provider_key(
    app: tauri::AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        let hash_key = load_or_create_hash_key(&storage.conn)?;
        let registry = ensure_registry(state.inner(), &hash_key)?;
        let descriptor = registry
            .descriptors()
            .into_iter()
            .find(|descriptor| descriptor.id == provider_id)
            .ok_or_else(|| format!("unknown provider: {provider_id}"))?;
        if descriptor.auth != AuthKind::ApiKey {
            return Err(format!(
                "{} does not use an API key",
                descriptor.display_name
            ));
        }
        if api_key.trim().is_empty() {
            return Err("the API key must not be empty".to_string());
        }
        CredentialStore::set(&provider_id, api_key.trim()).map_err(|e| e.to_string())?;
    }
    get_settings_view(&state)
}

#[tauri::command]
pub async fn delete_provider_key(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    if provider_id == "opencode" {
        return Err(
            "OpenCode Go uses the workspace and auth cookie configuration instead of an API key"
                .to_string(),
        );
    }
    CredentialStore::delete(&provider_id).map_err(|e| e.to_string())?;
    get_settings_view(&state)
}

/// Values submitted by the dedicated OpenCode Go settings form.
#[derive(Debug, Clone, Deserialize)]
pub struct OpenCodeGoConfigInput {
    pub workspace_id: String,
    pub auth_cookie: String,
}

fn clear_opencode_go_quota_report(state: &AppState) -> Result<(), String> {
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    QuotaRepository::new(&storage.conn)
        .upsert_report(&QuotaReport::failed(
            "opencode",
            "provider_api",
            "NOT_CONFIGURED",
        ))
        .map(|_| ())
        .map_err(|error| format!("clear OpenCode Go quota: {error}"))
}

/// Stores OpenCode Go's workspace id and auth cookie in Windows Credential
/// Manager. The values are never returned in the response.
#[tauri::command]
pub async fn set_opencode_go_config(
    app: tauri::AppHandle,
    config: OpenCodeGoConfigInput,
) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    let serialized = encode_go_config(&config.workspace_id, &config.auth_cookie)?;
    CredentialStore::set(OPENCODE_GO_CREDENTIAL_ID, &serialized).map_err(|e| e.to_string())?;
    clear_opencode_go_quota_report(state.inner())?;
    get_settings_view(&state)
}

/// Removes the OpenCode Go credential pair and clears any previously stored
/// quota windows so an old estimate cannot remain visible as a fake bar.
#[tauri::command]
pub async fn delete_opencode_go_config(app: tauri::AppHandle) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    match CredentialStore::delete(OPENCODE_GO_CREDENTIAL_ID) {
        Ok(()) | Err(lnwdeck_windows_integration::CredentialError::NotFound) => {}
        Err(error) => return Err(error.to_string()),
    }
    clear_opencode_go_quota_report(state.inner())?;
    get_settings_view(&state)
}

/// Background events (refresh loop, updater, migrations) for the System page.
#[tauri::command]
pub async fn get_app_events(
    app: tauri::AppHandle,
    limit: Option<usize>,
) -> Result<Vec<lnwdeck_storage::repositories::AppEventRow>, String> {
    let state = app.state::<AppState>();
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    AppEventRepository::new(&storage.conn)
        .recent(limit.unwrap_or(50))
        .map_err(|e| format!("app events: {e}"))
}
