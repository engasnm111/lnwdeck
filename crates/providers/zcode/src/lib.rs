//! ZCode (Z.AI) adapter.
//!
//! ZCode is Z.ai's coding agent. It keeps an OpenCode-fork SQLite store at
//! `~/.zcode/cli/db/db.sqlite` whose `message` table records every assistant
//! turn. Only turns that belong to ZCode itself are counted: bundled
//! Claude/Codex/Gemini sub-agents (providerID `anthropic` / `openai` /
//! `google`) are excluded because the dedicated adapters already count them.
//!
//! Quota is the real GLM Coding Plan quota:
//!
//! - When `~/.zcode/v2/config.json` holds a plaintext API key for the
//!   built-in coding-plan provider, the Z.AI (or BigModel) monitor API
//!   (`/api/monitor/usage/quota/limit`) publishes the used percentage and
//!   reset time of the 5-hour, weekly and tool-call windows.
//! - Otherwise the local ZCode logs (`~/.zcode/v2/logs/*.log`) carry the
//!   `billing/balance` records ZCode itself fetched, with real credit totals,
//!   used and remaining amounts plus the period end.
//! - When neither is available the adapter reports no quota. Token totals from
//!   the local message store remain available through the separate usage
//!   channel and are never presented as a subscription limit.
//!
//! The API key is read from ZCode's own config file and sent only over HTTPS
//! to the declared Z.AI / BigModel endpoints; it never appears in errors or
//! diagnostics.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, JsonRequest};

mod credentials;
pub use credentials::{
    credential_secret, decrypt_credential_value, node_platform, parse_account_provider_credential,
    AccountCredential, CredentialStore, TeamContext,
};
use lnwdeck_provider_runtime::opencode_fork::{self, MessageSample};
use lnwdeck_provider_runtime::token_scan::{usage_events, ScanBounds};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

const PROVIDER_ID: &str = "zcode_ai";
const ADAPTER_VERSION: &str = "0.1.0";

const ZAI_MONITOR_QUOTA_URL: &str = "https://api.z.ai/api/monitor/usage/quota/limit";
const BIGMODEL_MONITOR_QUOTA_URL: &str = "https://bigmodel.cn/api/monitor/usage/quota/limit";

/// ZCode config entries, tried in order. Coding-plan entries expose the
/// monitor quota API; start-plan entries only publish balances in logs.
const CONFIG_CANDIDATES: &[&str] = &[
    "builtin:zai-coding-plan",
    "builtin:bigmodel-coding-plan",
    "builtin:zai-start-plan",
    "builtin:bigmodel-start-plan",
];

/// Provider ids of bundled sub-agents whose turns are counted by their own
/// adapters and must not be double-counted here.
const EXCLUDED_PROVIDERS: &[&str] = &["anthropic", "openai", "google"];

struct MonitorCredential {
    endpoint: &'static str,
    key: String,
    team: Option<TeamContext>,
}

/// A plan selected in ZCode's settings, with its optional team scope.
#[derive(Debug, Clone)]
struct SelectedPlan {
    provider_key: String,
    team: Option<TeamContext>,
}

pub struct ZCodeAdapter {
    home: PathBuf,
    timeout: Duration,
    /// Test-only override so credential decryption can be exercised without
    /// the OS-derived secret. Production always loads the store lazily.
    credentials: Option<CredentialStore>,
}

impl Default for ZCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZCodeAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".zcode");
        Self::with_home(home)
    }

    /// Adapter pinned to an explicit ZCode home (used by tests).
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            home,
            timeout: Duration::from_secs(10),
            credentials: None,
        }
    }

    /// Adapter pinned to a decoded credential store (used by tests).
    pub fn with_credentials(home: PathBuf, credentials: CredentialStore) -> Self {
        Self {
            home,
            timeout: Duration::from_secs(10),
            credentials: Some(credentials),
        }
    }

    fn db_path(&self) -> PathBuf {
        self.home.join("cli").join("db").join("db.sqlite")
    }

    fn config_path(&self) -> PathBuf {
        self.home.join("v2").join("config.json")
    }

    fn logs_dir(&self) -> PathBuf {
        self.home.join("v2").join("logs")
    }

    fn credential_store(&self) -> CredentialStore {
        match &self.credentials {
            Some(store) => store.clone(),
            None => CredentialStore::load(&self.home),
        }
    }

    fn setting(&self) -> Option<serde_json::Value> {
        let raw = std::fs::read_to_string(self.home.join("v2").join("setting.json")).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Plans the user selected, active region first. ZCode 3.14+ records the
    /// choice as `providerFamilyConnectionSelections` instead of
    /// `modelProviderFamilySelectedKeys`; both formats are honored.
    fn selected_plans(&self) -> Vec<SelectedPlan> {
        let Some(setting) = self.setting() else {
            return Vec::new();
        };
        let active_domain = setting
            .get("providerFamilyDomain")
            .and_then(serde_json::Value::as_str);
        let mut plans: Vec<SelectedPlan> = Vec::new();
        fn push_plan(
            plans: &mut Vec<SelectedPlan>,
            provider_key: String,
            team: Option<TeamContext>,
        ) {
            if !plans.iter().any(|plan| plan.provider_key == provider_key) {
                plans.push(SelectedPlan { provider_key, team });
            }
        }

        if let Some(selected) = setting
            .get("modelProviderFamilySelectedKeys")
            .and_then(serde_json::Value::as_object)
        {
            let mut domains: Vec<&str> = active_domain.into_iter().collect();
            domains.extend(selected.keys().map(String::as_str));
            for domain in domains {
                let Some(raw) = selected.get(domain).and_then(serde_json::Value::as_str) else {
                    continue;
                };
                let Some(candidate) = CONFIG_CANDIDATES
                    .iter()
                    .find(|candidate| raw.contains(**candidate))
                else {
                    continue;
                };
                push_plan(
                    &mut plans,
                    (*candidate).to_string(),
                    team_context_from_selection(raw),
                );
            }
            if !plans.is_empty() {
                return plans;
            }
        }

        if let Some(selections) = setting
            .get("providerFamilyConnectionSelections")
            .and_then(serde_json::Value::as_object)
        {
            let mut families: Vec<&str> = active_domain.into_iter().collect();
            families.extend(selections.keys().map(String::as_str));
            for family in families {
                if family != "bigmodel" && family != "zai" {
                    continue;
                }
                let Some(kind) = selections
                    .get(family)
                    .and_then(|entry| entry.get("kind"))
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let Some(plan_kind) = plan_kind_from_id(kind) else {
                    continue;
                };
                push_plan(
                    &mut plans,
                    format!("builtin:{family}-{plan_kind}-plan"),
                    None,
                );
            }
        }
        plans
    }

    /// Region whose plan should be served first: the model settings domain the
    /// user picked, falling back to the OAuth login region.
    fn active_region(&self, store: &CredentialStore) -> Option<String> {
        if let Some(domain) = self
            .setting()
            .and_then(|setting| {
                setting
                    .get("providerFamilyDomain")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .filter(|domain| domain == "bigmodel" || domain == "zai")
        {
            return Some(domain);
        }
        store
            .get("oauth:active_provider")
            .filter(|provider| *provider == "bigmodel" || *provider == "zai")
            .map(str::to_string)
    }

    fn login_account_id(&self, store: &CredentialStore, family: &str) -> Option<String> {
        let raw = store.get(&format!("oauth:{family}:user_info"))?;
        let info: serde_json::Value = serde_json::from_str(raw).ok()?;
        info.get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
    }

    fn config_key(providers: Option<&serde_json::Value>, candidate: &str) -> Option<String> {
        let key = providers?
            .get(candidate)?
            .get("options")?
            .get("apiKey")?
            .as_str()?
            .trim();
        if key.is_empty() || key.starts_with("enc:v1:") {
            return None;
        }
        Some(key.to_string())
    }

    /// Picks the best usable coding-plan credential.
    ///
    /// Coding plans precede promotional start-plan grants; within each plan
    /// kind the active region's provider comes first so switching regions never
    /// keeps serving the previous region's plan. Keys come from ZCode 3.14+
    /// `credentials.json` first, then from a legacy `config.json` entry. Start
    /// plans publish no monitor quota, so they keep the local-log fallback.
    fn api_key(&self) -> Option<MonitorCredential> {
        let store = self.credential_store();
        let config = std::fs::read_to_string(self.config_path())
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        let providers = config.as_ref().and_then(|config| config.get("provider"));
        let active_region = self.active_region(&store);

        let mut candidates = self.selected_plans();
        for default in CONFIG_CANDIDATES {
            if !candidates.iter().any(|plan| plan.provider_key == *default) {
                candidates.push(SelectedPlan {
                    provider_key: (*default).to_string(),
                    team: None,
                });
            }
        }
        let region_rank = |provider_key: &str| {
            let family = provider_family(provider_key);
            usize::from(
                active_region
                    .as_deref()
                    .is_some_and(|active| active != family),
            )
        };
        candidates.sort_by_key(|plan| {
            (
                usize::from(!plan.provider_key.ends_with("coding-plan")),
                region_rank(&plan.provider_key),
            )
        });

        for plan in candidates {
            if !plan.provider_key.ends_with("coding-plan") {
                continue;
            }
            let family = provider_family(&plan.provider_key);
            let endpoint = if family == "zai" {
                ZAI_MONITOR_QUOTA_URL
            } else {
                BIGMODEL_MONITOR_QUOTA_URL
            };
            let login_account = self.login_account_id(&store, family);
            let mut accounts: Vec<AccountCredential> = store
                .account_credentials()
                .into_iter()
                .filter(|account| account.provider_key == plan.provider_key)
                .collect();
            accounts.sort_by_key(|account| {
                usize::from(login_account.as_deref() != Some(account.account_id.as_str()))
            });
            if let Some(account) = accounts.into_iter().next() {
                return Some(MonitorCredential {
                    endpoint,
                    key: account.api_key,
                    team: account.team.or(plan.team),
                });
            }
            if let Some(key) = Self::config_key(providers, &plan.provider_key) {
                return Some(MonitorCredential {
                    endpoint,
                    key,
                    team: plan.team,
                });
            }
        }
        None
    }

    /// True when the provider id belongs to a bundled sub-agent that the
    /// dedicated adapters already count.
    fn is_excluded_provider(provider: Option<&str>) -> bool {
        let Some(provider) = provider else {
            return true;
        };
        EXCLUDED_PROVIDERS
            .iter()
            .any(|excluded| provider.to_lowercase().contains(excluded))
    }

    fn samples(&self) -> Result<Vec<MessageSample>, String> {
        let bounds = ScanBounds::default();
        let all = opencode_fork::read_messages(&self.db_path(), &bounds)?;
        Ok(all
            .into_iter()
            .filter(|sample| !Self::is_excluded_provider(sample.provider_id.as_deref()))
            .collect())
    }

    /// Fetches the coding-plan quota from the monitor API.
    ///
    /// `Ok(None)` means the payload carried no usable windows; an expired
    /// credential or a rate limit is reported so the UI does not mask a real
    /// signal behind a local estimate.
    fn monitor_quota(&self, credential: &MonitorCredential) -> Result<Option<QuotaReport>, String> {
        let mut url = credential.endpoint.to_string();
        let mut headers: Vec<(&str, &str)> = Vec::new();
        if let Some(team) = &credential.team {
            // Team subscriptions are selected with `type=2` plus the scope
            // headers; the credential itself stays the same.
            url.push_str("?type=2");
            headers.push(("bigmodel-organization", team.organization_id.as_str()));
            headers.push(("bigmodel-project", team.project_id.as_str()));
        }
        let request = JsonRequest {
            timeout: self.timeout,
            ..JsonRequest::new(&url).raw_auth(&credential.key)
        };
        let request = if headers.is_empty() {
            request
        } else {
            request.with_headers(&headers)
        };
        let response = get_json(request)?;
        let Some((windows, plan)) = windows_from_monitor_payload(&response.body)? else {
            return Ok(None);
        };
        let mut report = QuotaReport::new(PROVIDER_ID, "provider_api", windows, DEFAULT_FRESHNESS);
        report.plan = plan;
        Ok(Some(report))
    }

    /// Reads the most recent `billing/balance` records ZCode wrote to its own
    /// logs and turns them into credit windows with real limits.
    fn balance_from_logs(&self) -> Result<Option<QuotaReport>, String> {
        let logs_dir = self.logs_dir();
        let mut windows = Vec::new();
        for offset_days in [0i64, -1] {
            let date = chrono::Local::now() + chrono::Duration::days(offset_days);
            let path = logs_dir.join(format!("{}.log", date.format("%Y-%m-%d")));
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            windows.extend(balance_windows_from_log(&content));
        }
        if windows.is_empty() {
            return Ok(None);
        }
        windows.sort_by_key(|window| window.limit.unwrap_or(0));
        windows.reverse();
        windows.truncate(3);
        Ok(Some(QuotaReport::new(
            PROVIDER_ID,
            "local_log",
            windows,
            DEFAULT_FRESHNESS,
        )))
    }

    /// Quota with the best evidence first: monitor API, then the local log
    /// balance. Local token totals are usage history, not quota evidence.
    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        if let Some(credential) = self.api_key() {
            match self.monitor_quota(&credential) {
                Ok(Some(report)) => return Ok(Some(report)),
                Ok(None) => {}
                Err(code) if code == "AUTH_EXPIRED" || code == "RATE_LIMITED" => return Err(code),
                Err(_) => {}
            }
        }
        if let Some(report) = self.balance_from_logs()? {
            return Ok(Some(report));
        }
        Ok(None)
    }

    fn detection(&self) -> DetectionResult {
        let source_exists = self.db_path().is_file();
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "ZCode".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_sqlite".to_string(),
            source_type: "sqlite".to_string(),
            source_exists,
            permission_state: "n/a".to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(chrono::Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        };
        if !source_exists {
            result.permission_state = "not_found".to_string();
            return result;
        }
        match self.samples() {
            Ok(samples) if !samples.is_empty() => {
                result.detected = true;
                result.permission_state = "read_ok".to_string();
            }
            Ok(_) => result.permission_state = "no_sessions".to_string(),
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
            }
        }
        result
    }
}

/// Quota windows parsed from a monitor payload, with an optional plan label.
pub type MonitorWindows = (Vec<QuotaWindow>, Option<String>);

/// Parses the monitor API payload into quota windows plus an optional plan
/// label. Returns `Ok(None)` when the payload has no usable window.
pub fn windows_from_monitor_payload(
    payload: &serde_json::Value,
) -> Result<Option<MonitorWindows>, String> {
    let code = payload
        .get("code")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    if code != 0 && code != 200 {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    if payload.get("success").and_then(|value| value.as_bool()) == Some(false) {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    let data = payload
        .get("data")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let limits = data
        .get("limits")
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let named = [
        window_from_limit(
            find_limit(limits, &["TOKENS_LIMIT", "CREDIT_LIMIT"], 3, Some(5)),
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Credits,
        ),
        window_from_limit(
            find_limit(limits, &["TOKENS_LIMIT", "CREDIT_LIMIT"], 6, None),
            "7d",
            "Weekly",
            QuotaWindowScope::Weekly,
            QuotaKind::Credits,
        ),
        window_from_limit(
            find_limit(limits, &["TIME_LIMIT"], 5, Some(1)),
            "tools",
            "Tool calls",
            QuotaWindowScope::Other,
            QuotaKind::Requests,
        ),
    ];
    let named: Vec<QuotaWindow> = named.into_iter().flatten().collect();
    let windows = if named.is_empty() {
        limits
            .iter()
            .filter_map(|limit| {
                window_from_limit(
                    Some(limit),
                    "quota",
                    "Quota",
                    QuotaWindowScope::Other,
                    QuotaKind::Credits,
                )
            })
            .collect()
    } else {
        named
    };
    if windows.is_empty() {
        return Ok(None);
    }
    let plan = data
        .get("level")
        .and_then(|value| value.as_str())
        .map(plan_label)
        .or_else(|| Some("Coding".to_string()));
    Ok(Some((windows, plan)))
}

fn find_limit<'a>(
    limits: &'a [serde_json::Value],
    limit_types: &[&str],
    unit: i64,
    number: Option<i64>,
) -> Option<&'a serde_json::Value> {
    limits.iter().find(|limit| {
        limit_types.iter().any(|limit_type| {
            limit.get("type").and_then(|value| value.as_str()) == Some(*limit_type)
        }) && limit.get("unit").and_then(|value| value.as_i64()) == Some(unit)
            && (number.is_none() || limit.get("number").and_then(|value| value.as_i64()) == number)
    })
}

fn as_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    value?.as_f64().filter(|number| number.is_finite())
}

/// Builds one percent-only window from a quota limit row. The published
/// `percentage` is authoritative; otherwise it is derived from
/// usage/remaining against the total. A row with no percentage and no usable
/// totals produces no window.
fn window_from_limit(
    limit: Option<&serde_json::Value>,
    key: &str,
    label: &str,
    scope: QuotaWindowScope,
    kind: QuotaKind,
) -> Option<QuotaWindow> {
    let limit = limit?;
    let total = as_f64(limit.get("number"));
    let used = as_f64(limit.get("usage")).or_else(|| as_f64(limit.get("currentValue")));
    let remaining = as_f64(limit.get("remaining"));
    let raw_percent = as_f64(limit.get("percentage"));
    let percent = raw_percent.or_else(|| match (total, used) {
        (Some(total), Some(used)) if total > 0.0 => Some(used / total * 100.0),
        _ => match (total, remaining) {
            (Some(total), Some(remaining)) if total > 0.0 && remaining <= total => {
                Some((total - remaining) / total * 100.0)
            }
            _ => None,
        },
    })?;
    Some(QuotaWindow::from_percent(
        key,
        label,
        scope,
        kind,
        percent,
        parse_reset(limit.get("nextResetTime")),
        Confidence::High,
    ))
}

fn parse_reset(value: Option<&serde_json::Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    match value? {
        serde_json::Value::String(text) => chrono::DateTime::parse_from_rfc3339(text.trim())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok(),
        serde_json::Value::Number(number) => {
            let value = number.as_i64()?;
            if value <= 0 {
                return None;
            }
            if value > 10_000_000_000 {
                chrono::DateTime::from_timestamp_millis(value)
            } else {
                chrono::DateTime::from_timestamp(value, 0)
            }
        }
        _ => None,
    }
}

/// Human label from a plan level id (`zcode-v3-start-plan-0615` -> `Start`).
fn plan_label(level: &str) -> String {
    let lower = level.to_lowercase();
    for tier in ["start", "lite", "pro", "max", "team", "enterprise"] {
        if lower.contains(tier) {
            let mut chars = tier.chars();
            return chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_else(|| tier.to_string());
        }
    }
    level.to_string()
}

/// Region a built-in plan provider belongs to.
fn provider_family(provider_key: &str) -> &str {
    if provider_key.starts_with("builtin:zai-") {
        "zai"
    } else {
        "bigmodel"
    }
}

/// Plan class ("coding" | "start") a plan id serves.
fn plan_kind_from_id(plan_id: &str) -> Option<&'static str> {
    if plan_id.ends_with("coding-plan") {
        Some("coding")
    } else if plan_id == "start-plan" {
        Some("start")
    } else {
        None
    }
}

/// Decodes the team scope ZCode stores in a selection:
/// `team-plan:builtin:<family>-coding-plan:<product>:<org>:<project>`.
fn team_context_from_selection(selection: &str) -> Option<TeamContext> {
    let rest = selection.trim().strip_prefix("team-plan:builtin:")?;
    let mut parts = rest.split(':');
    let plan = parts.next()?;
    if !plan.ends_with("-coding-plan") {
        return None;
    }
    let product = credentials::percent_decode(parts.next()?.trim());
    let organization = credentials::percent_decode(parts.next()?.trim());
    let project = credentials::percent_decode(parts.next()?.trim());
    if parts.next().is_some() || product.is_empty() || organization.is_empty() || project.is_empty()
    {
        return None;
    }
    Some(TeamContext {
        organization_id: organization,
        project_id: project,
    })
}

/// Extracts credit windows from ZCode's local `billing/balance` log records.
///
/// A record is a line containing `[usage-stats] billing/balance` followed by
/// a JSON object whose `payload.data.balances` array carries
/// `total_units` / `used_units` / `remaining_units` / `period_end`.
pub fn balance_windows_from_log(content: &str) -> Vec<QuotaWindow> {
    let mut windows = Vec::new();
    for line in content.lines() {
        if !line.contains("[usage-stats] billing/balance") {
            continue;
        }
        let Some(json_start) = line.find('{') else {
            continue;
        };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line[json_start..]) else {
            continue;
        };
        if entry.get("success").and_then(|value| value.as_bool()) != Some(true) {
            continue;
        }
        let Some(balances) = entry
            .get("payload")
            .and_then(|payload| payload.get("data"))
            .and_then(|data| data.get("balances"))
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for balance in balances {
            let Some(total) = as_f64(balance.get("total_units")) else {
                continue;
            };
            let Some(used) = as_f64(balance.get("used_units")) else {
                continue;
            };
            let Some(total_units) = NonZeroU64::new(total.round().max(0.0) as u64) else {
                continue;
            };
            let used_units = used.round().max(0.0) as u64;
            let label = balance
                .get("show_name")
                .and_then(|value| value.as_str())
                .filter(|label| !label.is_empty())
                .unwrap_or("Credits")
                .to_string();
            let reset = balance
                .get("period_end")
                .or_else(|| balance.get("expires_at"))
                .and_then(|value| value.as_i64())
                .and_then(|seconds| {
                    if seconds > 0 {
                        chrono::DateTime::from_timestamp(seconds, 0)
                    } else {
                        None
                    }
                });
            windows.push(QuotaWindow::with_limit(
                &label,
                &label,
                QuotaWindowScope::Other,
                QuotaKind::Credits,
                used_units,
                total_units,
                reset,
                Confidence::High,
            ));
        }
    }
    windows
}

impl ProviderAdapter for ZCodeAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "ZCode",
            vendor: "Z.AI",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        let samples = self.samples()?;
        if samples.is_empty() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", chrono::Utc::now().timestamp()),
            events: usage_events(
                PROVIDER_ID,
                "local_sqlite",
                &opencode_fork::to_token_samples(&samples),
                Confidence::Medium,
            ),
        })
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }

    fn account_identity(&self) -> Option<String> {
        self.api_key().map(|credential| credential.key)
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "ZCode local data not found".to_string(),
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "ZCode local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "ZCode local data has no GLM token records".to_string(),
            }
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(self.detection())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::path::Path;

    fn write_db(home: &Path) {
        let db_dir = home.join("cli").join("db");
        std::fs::create_dir_all(&db_dir).expect("db dir");
        let conn = Connection::open(db_dir.join("db.sqlite")).expect("open");
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY, session_id TEXT,
                time_created INTEGER, time_updated INTEGER, data TEXT
             );",
        )
        .expect("schema");
        let message = |id: &str, provider: &str, model: &str, input: u64, output: u64| {
            format!(
                r#"{{"id":"{id}","providerID":"{provider}","modelID":"{model}","role":"assistant","tokens":{{"input":{input},"output":{output},"reasoning":0}},"time":{{"created":1700000000000}}}}"#
            )
        };
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES ('z1','s1',1700000000000,1700000000000,?1),
                    ('z2','s1',1700000000000,1700000001000,?2),
                    ('c1','s1',1700000000000,1700000002000,?3)",
            rusqlite::params![
                message("z1", "builtin:zai-coding-plan", "glm-5.2", 100, 50),
                message("z2", "builtin:zai-start-plan", "glm-4.7", 10, 10),
                message("c1", "anthropic", "claude-3-7-sonnet", 999, 999),
            ],
        )
        .expect("rows");
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(ZCodeAdapter::new().id(), "zcode_ai");
    }

    #[test]
    fn counts_only_zcode_turns() {
        let home = tempfile::tempdir().expect("temp");
        write_db(home.path());
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let samples = adapter.samples().expect("samples");
        assert_eq!(samples.len(), 2, "anthropic sub-agent turns are excluded");
        assert!(
            samples.iter().all(|sample| {
                sample
                    .model
                    .as_deref()
                    .map(|model| model.starts_with("glm-"))
                    .unwrap_or(false)
            }),
            "only GLM turns survive: {samples:?}"
        );
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert!(batch.events.iter().all(|e| e.provider_id == PROVIDER_ID));
    }

    #[test]
    fn local_usage_is_not_presented_as_quota() {
        let home = tempfile::tempdir().expect("temp");
        write_db(home.path());
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());

        assert!(
            adapter.collect_quota().expect("quota").is_none(),
            "local token totals do not prove a subscription quota"
        );
    }

    #[test]
    fn missing_home_reports_no_data() {
        let home = tempfile::tempdir().expect("temp");
        let adapter = ZCodeAdapter::with_home(home.path().join("missing"));
        assert_eq!(adapter.samples(), Err("SOURCE_UNAVAILABLE".to_string()));
        assert!(adapter.collect_quota().expect("quota").is_none());
        assert!(!adapter.detection().source_exists);
    }

    #[test]
    fn monitor_payload_builds_named_windows() {
        let payload = serde_json::json!({
            "code": 0,
            "data": {
                "level": "zcode-v3-pro-plan-202608",
                "limits": [
                    {"type":"TOKENS_LIMIT","unit":3,"number":5,"usage":40,"percentage":66.7,"nextResetTime":1780000000,"usageDetails":[{"displayName":"5h","modelCode":"glm-5.2"}]},
                    {"type":"TOKENS_LIMIT","unit":6,"usage":300,"percentage":10.5,"nextResetTime":"2026-08-15T00:00:00Z"},
                    {"type":"TIME_LIMIT","unit":5,"number":1,"usage":1,"percentage":0.0}
                ]
            }
        });
        let (windows, plan) = windows_from_monitor_payload(&payload)
            .expect("windows")
            .expect("non-empty");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window_key, "5h");
        assert_eq!(windows[0].used_percent, Some(66.7));
        assert_eq!(windows[0].remaining_percent, Some(33.3));
        assert!(windows[0].reset_at.is_some());
        assert_eq!(windows[1].window_key, "7d");
        assert_eq!(windows[1].remaining_percent, Some(89.5));
        assert_eq!(windows[2].window_key, "tools");
        assert_eq!(windows[2].remaining_percent, Some(100.0));
        assert_eq!(plan.as_deref(), Some("Pro"));
        for window in &windows {
            window.check_invariants().expect("consistent window");
        }
    }

    #[test]
    fn monitor_payload_without_named_windows_falls_back_to_all_limits() {
        let payload = serde_json::json!({
            "code": 0,
            "data": { "limits": [
                {"type":"TOKENS_LIMIT","unit":9,"number":100,"usage":25}
            ]}
        });
        let (windows, _) = windows_from_monitor_payload(&payload)
            .expect("windows")
            .expect("non-empty");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, Some(25.0));
    }

    #[test]
    fn monitor_payload_errors_are_reported() {
        for payload in [
            serde_json::json!({"code": 400}),
            serde_json::json!({"success": false}),
        ] {
            assert_eq!(
                windows_from_monitor_payload(&payload),
                Err("SOURCE_SCHEMA_MISMATCH".to_string()),
                "payload must be rejected: {payload}"
            );
        }
        for payload in [
            serde_json::json!({"data": {}}),
            serde_json::json!({"code": 0, "data": {"limits": []}}),
        ] {
            assert_eq!(
                windows_from_monitor_payload(&payload).expect("parses"),
                None,
                "no windows must be reported, not guessed: {payload}"
            );
        }
    }

    #[test]
    fn balance_log_records_become_credit_windows() {
        let line = format!(
            "[2026-08-08 10:00:00.123] [usage-stats] billing/balance ok {}",
            serde_json::json!({
                "success": true, "code": 0, "providerId": "builtin:zai-start-plan",
                "payload": {"data": {"balances": [
                    {"show_name": "GLM-5.2", "total_units": 100, "used_units": 40,
                     "remaining_units": 60, "period_end": 1780000000}
                ]}}
            })
        );
        let windows = balance_windows_from_log(&line);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used, 40);
        assert_eq!(windows[0].limit, Some(100));
        assert_eq!(windows[0].remaining, Some(60));
        assert!(windows[0].reset_at.is_some());
        windows[0].check_invariants().expect("consistent window");
        assert!(balance_windows_from_log("no records here").is_empty());
    }

    #[test]
    fn api_key_prefers_coding_plan_entries() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:zai-start-plan": {"options": {"apiKey": "sk-start"}},
                    "builtin:zai-coding-plan": {"options": {"apiKey": "sk-coding"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let credential = adapter.api_key().expect("key");
        assert_eq!(credential.endpoint, ZAI_MONITOR_QUOTA_URL);
        assert_eq!(credential.key, "sk-coding");
    }

    #[test]
    fn selected_plan_wins_over_stale_provider_entries() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:zai-coding-plan": {"options": {"apiKey": "stale-zai"}},
                    "builtin:bigmodel-coding-plan": {"options": {"apiKey": "selected-bigmodel"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        std::fs::write(
            config_dir.join("setting.json"),
            serde_json::json!({
                "providerFamilyDomain": "bigmodel",
                "modelProviderFamilySelectedKeys": {
                    "bigmodel": "builtin:bigmodel-coding-plan"
                }
            })
            .to_string(),
        )
        .expect("setting");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let credential = adapter.api_key().expect("selected key");
        assert_eq!(credential.endpoint, BIGMODEL_MONITOR_QUOTA_URL);
        assert_eq!(credential.key, "selected-bigmodel");
    }

    #[test]
    fn api_key_selects_bigmodel_endpoint_when_zai_entry_is_absent() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:bigmodel-coding-plan": {"options": {"apiKey": "sk-bigmodel"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        let credential = adapter.api_key().expect("key");
        assert_eq!(credential.endpoint, BIGMODEL_MONITOR_QUOTA_URL);
        assert_eq!(credential.key, "sk-bigmodel");
    }

    #[test]
    fn encrypted_keys_count_as_absent() {
        let home = tempfile::tempdir().expect("temp");
        let config_dir = home.path().join("v2");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(
            config_dir.join("config.json"),
            serde_json::json!({
                "provider": {
                    "builtin:zai-coding-plan": {"options": {"apiKey": "enc:v1:xyz"}}
                }
            })
            .to_string(),
        )
        .expect("config");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        assert!(adapter.api_key().is_none());
    }

    #[test]
    fn without_any_source_quota_reports_none() {
        let home = tempfile::tempdir().expect("temp");
        let adapter = ZCodeAdapter::with_home(home.path().to_path_buf());
        assert!(adapter.collect_quota().expect("quota").is_none());
    }

    #[test]
    fn plan_labels_are_human_readable() {
        assert_eq!(plan_label("zcode-v3-start-plan-0615"), "Start");
        assert_eq!(plan_label("zcode-v3-pro-plan-202608"), "Pro");
        assert_eq!(plan_label("something-else"), "something-else");
    }

    #[test]
    fn credit_limit_rows_map_to_the_named_windows() {
        let payload = serde_json::json!({
            "code": 0,
            "data": {
                "level": "zcode-v3-pro-plan",
                "limits": [
                    {"type": "CREDIT_LIMIT", "unit": 3, "number": 5, "percentage": 42.0,
                     "nextResetTime": 1_790_000_000_i64},
                    {"type": "CREDIT_LIMIT", "unit": 6, "number": 7, "percentage": 80.0,
                     "nextResetTime": 1_790_100_000_i64}
                ]
            }
        });
        let (windows, plan) = windows_from_monitor_payload(&payload)
            .expect("payload")
            .expect("windows");
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].window_key, "5h");
        assert_eq!(windows[0].used_percent, Some(42.0));
        assert_eq!(windows[1].window_key, "7d");
        assert_eq!(windows[1].used_percent, Some(80.0));
        assert_eq!(plan.as_deref(), Some("Pro"));
    }

    #[test]
    fn credentials_file_supplies_the_coding_plan_key() {
        let home = tempfile::tempdir().expect("temp");
        let v2 = home.path().join("v2");
        std::fs::create_dir_all(&v2).expect("v2 dir");
        std::fs::write(
            v2.join("credentials.json"),
            serde_json::json!({
                "account-provider:coding-plan:account:zai-individual-coding-plan:account:acct-1:api-key":
                    "enc:v1:AAECAwQFBgcICQoL.YvjzoOrcSzdrRVYgmQRkEg.NPLn7_S0QvHoyjqjaaQIBgQ"
            })
            .to_string(),
        )
        .expect("credentials");
        let store = CredentialStore::load_with_secret(home.path(), "test-secret");
        let adapter = ZCodeAdapter::with_credentials(home.path().to_path_buf(), store);
        let credential = adapter.api_key().expect("credential");
        assert_eq!(credential.endpoint, ZAI_MONITOR_QUOTA_URL);
        assert_eq!(credential.key, "sk-zcode-test-key");
        assert!(credential.team.is_none());
    }

    #[test]
    fn team_plan_selection_adds_the_quota_scope() {
        let home = tempfile::tempdir().expect("temp");
        let v2 = home.path().join("v2");
        std::fs::create_dir_all(&v2).expect("v2 dir");
        std::fs::write(
            v2.join("credentials.json"),
            serde_json::json!({
                "account-provider:coding-plan:account:zai-individual-coding-plan:account:acct-1:api-key":
                    "enc:v1:AAECAwQFBgcICQoL.YvjzoOrcSzdrRVYgmQRkEg.NPLn7_S0QvHoyjqjaaQIBgQ"
            })
            .to_string(),
        )
        .expect("credentials");
        std::fs::write(
            v2.join("setting.json"),
            serde_json::json!({
                "providerFamilyDomain": "zai",
                "modelProviderFamilySelectedKeys": {
                    "zai": "team-plan:builtin:zai-coding-plan:prod-1:org-9:proj-2"
                }
            })
            .to_string(),
        )
        .expect("setting");
        let store = CredentialStore::load_with_secret(home.path(), "test-secret");
        let adapter = ZCodeAdapter::with_credentials(home.path().to_path_buf(), store);
        let credential = adapter.api_key().expect("credential");
        assert_eq!(
            credential.team,
            Some(TeamContext {
                organization_id: "org-9".to_string(),
                project_id: "proj-2".to_string(),
            })
        );
    }

    #[test]
    fn connection_selections_choose_the_coding_plan() {
        let home = tempfile::tempdir().expect("temp");
        let v2 = home.path().join("v2");
        std::fs::create_dir_all(&v2).expect("v2 dir");
        std::fs::write(
            v2.join("credentials.json"),
            serde_json::json!({
                "account-provider:coding-plan:account:bigmodel-individual-coding-plan:account:acct-b:api-key":
                    "enc:v1:AAECAwQFBgcICQoL.YvjzoOrcSzdrRVYgmQRkEg.NPLn7_S0QvHoyjqjaaQIBgQ",
                "account-provider:coding-plan:account:zai-individual-coding-plan:account:acct-z:api-key":
                    "enc:v1:AAECAwQFBgcICQoL.YvjzoOrcSzdrRVYgmQRkEg.NPLn7_S0QvHoyjqjaaQIBgQ"
            })
            .to_string(),
        )
        .expect("credentials");
        std::fs::write(
            v2.join("setting.json"),
            serde_json::json!({
                "providerFamilyDomain": "zai",
                "providerFamilyConnectionSelections": {
                    "zai": {"kind": "individual-coding-plan"}
                }
            })
            .to_string(),
        )
        .expect("setting");
        let store = CredentialStore::load_with_secret(home.path(), "test-secret");
        let adapter = ZCodeAdapter::with_credentials(home.path().to_path_buf(), store);
        let credential = adapter.api_key().expect("credential");
        assert_eq!(credential.endpoint, ZAI_MONITOR_QUOTA_URL);
    }

    #[test]
    fn team_context_is_only_read_from_team_selections() {
        assert_eq!(
            team_context_from_selection(
                "team-plan:builtin:bigmodel-coding-plan:prod-1:org%3A9:proj%2F2"
            ),
            Some(TeamContext {
                organization_id: "org:9".to_string(),
                project_id: "proj/2".to_string(),
            })
        );
        assert_eq!(team_context_from_selection("builtin:zai-coding-plan"), None);
    }

    #[test]
    fn unsupported_quota_channel_is_never_success() {
        let adapter = ZCodeAdapter::with_home(PathBuf::from("Z:/not/here"));
        let result = adapter.collect_usage_with_cursor(None);
        assert!(result.batch.is_none());
        assert_eq!(result.outcome.error_code, "SOURCE_UNAVAILABLE");
    }
}
