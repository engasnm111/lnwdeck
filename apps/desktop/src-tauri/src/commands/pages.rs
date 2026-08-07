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
use lnwdeck_pricing::catalog::PriceResolver;
use lnwdeck_storage::repositories::{
    AlertRepository, AppEventRepository, BudgetPeriod, BudgetRepository, BudgetRow, BudgetScope,
};
use lnwdeck_windows_integration::{CredentialState, CredentialStore, StartupRegistration};
use serde::{Deserialize, Serialize};
use tauri::State;

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

#[tauri::command]
pub fn get_usage_history(
    window: Option<String>,
    provider_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<UsageHistory, String> {
    let window = parse_window(window)?;
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    QueryUsageHistory::execute(&storage.conn, window, provider_id.as_deref())
        .map_err(|e| format!("usage history: {e}"))
}

#[tauri::command]
pub fn get_costs(
    window: Option<String>,
    state: State<'_, AppState>,
) -> Result<CostBreakdown, String> {
    let window = parse_window(window)?;
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let resolver = price_resolver(&storage.conn);
    QueryCosts::execute(&storage.conn, window, &resolver).map_err(|e| format!("costs: {e}"))
}

#[tauri::command]
pub fn get_budgets(state: State<'_, AppState>) -> Result<BudgetOverview, String> {
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
pub fn save_budget(budget: BudgetInput, state: State<'_, AppState>) -> Result<i64, String> {
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
pub fn delete_budget(id: i64, state: State<'_, AppState>) -> Result<(), String> {
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

#[tauri::command]
pub fn get_alerts(state: State<'_, AppState>) -> Result<AlertsView, String> {
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
pub fn acknowledge_alert(id: i64, state: State<'_, AppState>) -> Result<(), String> {
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

fn credential_label(state: CredentialState) -> &'static str {
    match state {
        CredentialState::Missing => "missing",
        CredentialState::Configured => "configured",
        CredentialState::Expired => "expired",
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsView, String> {
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    let hash_key = load_or_create_hash_key(&storage.conn)?;
    let registry = ensure_registry(state.inner(), &hash_key)?;

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
        .filter(|descriptor| descriptor.needs_credentials())
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
pub fn save_settings(
    settings: AppSettings,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsView, String> {
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
    get_settings(state)
}

/// Stores an API key for a provider in the Windows Credential Manager.
#[tauri::command]
pub fn set_provider_key(
    provider_id: String,
    api_key: String,
    state: State<'_, AppState>,
) -> Result<SettingsView, String> {
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
        if !descriptor.needs_credentials() {
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
    get_settings(state)
}

#[tauri::command]
pub fn delete_provider_key(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<SettingsView, String> {
    CredentialStore::delete(&provider_id).map_err(|e| e.to_string())?;
    get_settings(state)
}

/// Background events (refresh loop, updater, migrations) for the System page.
#[tauri::command]
pub fn get_app_events(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<lnwdeck_storage::repositories::AppEventRow>, String> {
    let guard = state.ensure_storage()?;
    let storage = guard.as_ref().ok_or("storage not initialized")?;
    AppEventRepository::new(&storage.conn)
        .recent(limit.unwrap_or(50))
        .map_err(|e| format!("app events: {e}"))
}
