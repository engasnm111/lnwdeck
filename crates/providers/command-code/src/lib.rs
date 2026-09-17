//! Command Code subscription quota adapter.
//!
//! Mirrors the Command Code CLI's own account/credits flow. Authentication is
//! read from `COMMAND_CODE_API_KEY` or `~/.commandcode/auth.json`; the secret
//! is sent only to Command Code over HTTPS and never persisted by lnwdeck.

use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, JsonRequest};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

const PROVIDER_ID: &str = "command_code";
const ADAPTER_VERSION: &str = "0.1.0";
const API_BASE: &str = "https://api.commandcode.ai";

fn plan_label(plan_id: &str) -> Option<String> {
    let normalized = plan_id.trim().to_ascii_lowercase().replace('_', "-");
    let label = if normalized.starts_with("individual-pro-v1")
        || normalized.starts_with("individual-pro")
    {
        "Pro"
    } else if normalized.starts_with("individual-provider") {
        "Provider"
    } else if normalized.starts_with("individual-goat") {
        "GOAT"
    } else if normalized.starts_with("individual-go") {
        "Go"
    } else if normalized.starts_with("individual-max") {
        "Max"
    } else if normalized.starts_with("individual-ultra") {
        "Ultra"
    } else if normalized.starts_with("teams-pro") {
        "Teams Pro"
    } else {
        return None;
    };
    Some(label.to_string())
}

fn command_code_key(home: &Path) -> Option<String> {
    if let Ok(value) = std::env::var("COMMAND_CODE_API_KEY") {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    let value: Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".commandcode").join("auth.json")).ok()?,
    )
    .ok()?;
    value
        .get("apiKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

fn reset_at(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let value = value?;
    if let Some(number) = number(Some(value)) {
        if number <= 0.0 {
            return None;
        }
        let millis = if number > 10_000_000_000.0 {
            number as i64
        } else {
            (number * 1000.0) as i64
        };
        return DateTime::from_timestamp_millis(millis);
    }
    DateTime::parse_from_rfc3339(value.as_str()?.trim())
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn quota_window(
    raw: &Value,
    key: &str,
    label: &str,
    scope: QuotaWindowScope,
) -> Option<QuotaWindow> {
    let used = number(raw.get("used"))?;
    let limit = number(
        raw.get("cap")
            .or_else(|| raw.get("total"))
            .or_else(|| raw.get("limit")),
    )?;
    if used < 0.0 || limit <= 0.0 {
        return None;
    }
    Some(QuotaWindow::from_percent(
        key,
        label,
        scope,
        QuotaKind::Credits,
        (used / limit * 100.0).clamp(0.0, 100.0),
        reset_at(
            raw.get("resetAt")
                .or_else(|| raw.get("reset_at"))
                .or_else(|| raw.get("reset")),
        ),
        Confidence::High,
    ))
}

/// Normalizes the `windowLimits` object returned by Command Code's credits API.
pub fn windows_from_credits(payload: &Value) -> Result<Vec<QuotaWindow>, String> {
    let limits = payload
        .get("windowLimits")
        .or_else(|| payload.pointer("/data/windowLimits"))
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let mut windows = Vec::new();
    if let Some(window) = limits
        .get("fiveHour")
        .or_else(|| limits.get("five_hour"))
        .and_then(|raw| quota_window(raw, "5h", "5-hour", QuotaWindowScope::Rolling))
    {
        windows.push(window);
    }
    if let Some(window) = limits
        .get("weekly")
        .and_then(|raw| quota_window(raw, "7d", "7-day", QuotaWindowScope::Weekly))
    {
        windows.push(window);
    }
    if windows.is_empty() {
        Err("SOURCE_SCHEMA_MISMATCH".to_string())
    } else {
        Ok(windows)
    }
}

pub struct CommandCodeAdapter {
    home: PathBuf,
    timeout: Duration,
}

impl Default for CommandCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandCodeAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            home,
            timeout: Duration::from_secs(10),
        }
    }

    #[cfg(test)]
    fn with_home(home: PathBuf) -> Self {
        Self {
            home,
            timeout: Duration::from_secs(1),
        }
    }

    fn fetch(&self) -> Result<QuotaReport, String> {
        let key = command_code_key(&self.home).ok_or_else(|| "NOT_CONFIGURED".to_string())?;
        let whoami = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new("https://api.commandcode.ai/alpha/whoami?limits=1")
            }
            .bearer(&key),
        )?;
        let org_id = whoami
            .body
            .pointer("/data/org/id")
            .or_else(|| whoami.body.pointer("/org/id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let suffix = org_id.map(|id| format!("?orgId={id}")).unwrap_or_default();
        let credits_url = format!("{API_BASE}/alpha/billing/credits{suffix}");
        let subscriptions_url = format!("{API_BASE}/alpha/billing/subscriptions{suffix}");
        let credits = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(&credits_url)
            }
            .bearer(&key),
        )?;
        let subscriptions = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(&subscriptions_url)
            }
            .bearer(&key),
        )?;
        let windows = windows_from_credits(&credits.body)?;
        let mut report = QuotaReport::new(PROVIDER_ID, "provider_api", windows, DEFAULT_FRESHNESS);
        report.plan = subscriptions
            .body
            .pointer("/data/planId")
            .or_else(|| subscriptions.body.get("planId"))
            .and_then(Value::as_str)
            .and_then(plan_label);
        Ok(report)
    }
}

impl ProviderAdapter for CommandCodeAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Command Code",
            vendor: "Command Code",
            source_kind: SourceKind::RemoteApi,
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.fetch().map(Some)
    }

    fn account_identity(&self) -> Option<String> {
        command_code_key(&self.home)
    }

    fn health_check(&self) -> AdapterHealth {
        if command_code_key(&self.home).is_none() {
            return AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "Command Code login not found".to_string(),
            };
        }
        match self.fetch() {
            Ok(_) => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Command Code quota available".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("Command Code request failed ({code})"),
            },
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        let configured = command_code_key(&self.home).is_some();
        Ok(DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Command Code".to_string(),
            enabled: true,
            detected: configured,
            detection_method: "local_auth".to_string(),
            source_type: "remote_api".to_string(),
            source_exists: configured,
            permission_state: if configured {
                "credential_found".to_string()
            } else {
                "credential_required".to_string()
            },
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: if configured {
                String::new()
            } else {
                "NOT_CONFIGURED".to_string()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_payload_yields_5h_and_weekly_windows() {
        let payload = serde_json::json!({
            "windowLimits": {
                "fiveHour": {"used": 25, "cap": 100, "resetAt": 1790000000000_i64},
                "weekly": {"used": 75, "cap": 300, "resetAt": 1790500000000_i64}
            }
        });
        let windows = windows_from_credits(&payload).expect("quota windows");
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].used_percent, Some(25.0));
        assert_eq!(windows[1].used_percent, Some(25.0));
    }

    #[test]
    fn plan_ids_are_rendered_as_human_tiers() {
        assert_eq!(plan_label("individual-pro-v1"), Some("Pro".to_string()));
        assert_eq!(plan_label("individual-goat-2026"), Some("GOAT".to_string()));
        assert_eq!(plan_label("unknown-plan"), None);
    }

    #[test]
    fn auth_file_is_detected_without_network() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(".commandcode");
        std::fs::create_dir_all(&root).expect("auth dir");
        std::fs::write(root.join("auth.json"), r#"{"apiKey":"fixture-key"}"#).expect("auth file");
        let adapter = CommandCodeAdapter::with_home(dir.path().to_path_buf());
        assert!(adapter.detect().expect("detect").detected);
    }
}
