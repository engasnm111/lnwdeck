//! Devin subscription quota adapter.
//!
//! Reads the Devin CLI sign-in token from its local credentials file and calls
//! the same fixed seat-management RPC used by the official client. The token
//! is never sent to a configurable host and is never persisted by lnwdeck.

use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{post_json, JsonRequest};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

const PROVIDER_ID: &str = "devin";
const ADAPTER_VERSION: &str = "0.1.0";
const DEVIN_API_ORIGIN: &str = "https://server.codeium.com";
const DEVIN_PLAN_STATUS_URL: &str =
    "https://server.codeium.com/exa.seat_management_pb.SeatManagementService/GetPlanStatus";

fn parse_root_toml(raw: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value
            .split('#')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        if !value.is_empty() {
            fields.insert(key.to_string(), value.to_string());
        }
    }
    fields
}

fn credentials_path(home: &Path) -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("devin").join("credentials.toml");
    }
    home.join(".local")
        .join("share")
        .join("devin")
        .join("credentials.toml")
}

fn devin_token(home: &Path) -> Result<Option<String>, String> {
    let path = credentials_path(home);
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("SOURCE_UNAVAILABLE".to_string()),
    };
    let fields = parse_root_toml(&raw);
    if let Some(origin) = fields.get("api_server_url") {
        if origin.trim_end_matches('/') != DEVIN_API_ORIGIN {
            return Err("UNSUPPORTED_CONFIGURATION".to_string());
        }
    }
    Ok(fields
        .get("windsurf_api_key")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn number(value: Option<&Value>) -> Result<Option<f64>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let number = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) if !text.trim().is_empty() => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
    .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    Ok(Some(number))
}

fn reset_at(value: Option<&Value>) -> Result<Option<DateTime<Utc>>, String> {
    let Some(seconds) = number(value)? else {
        return Ok(None);
    };
    if seconds == 0.0 {
        return Ok(None);
    }
    if seconds < 0.0 {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    DateTime::from_timestamp(seconds as i64, 0)
        .map(Some)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())
}

fn quota_window(
    remaining: Option<&Value>,
    reset: Option<&Value>,
    hidden: bool,
    key: &str,
    label: &str,
    scope: QuotaWindowScope,
) -> Result<Option<QuotaWindow>, String> {
    if hidden {
        return Ok(None);
    }
    let remaining = number(remaining)?;
    let reset = reset_at(reset)?;
    if remaining.is_none() && reset.is_none() {
        return Ok(None);
    }
    // Proto3 omits scalar zeroes. When a reset exists but remainingPercent is
    // absent, the provider means 0% remaining, i.e. fully used.
    let used = 100.0 - remaining.unwrap_or(0.0).clamp(0.0, 100.0);
    Ok(Some(QuotaWindow::from_percent(
        key,
        label,
        scope,
        QuotaKind::Requests,
        used,
        reset,
        Confidence::High,
    )))
}

/// Parses the official GetPlanStatus response into daily and weekly windows.
pub fn windows_from_plan_status(
    payload: &Value,
) -> Result<(Vec<QuotaWindow>, Option<String>), String> {
    let status = payload
        .get("planStatus")
        .and_then(Value::as_object)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let plan_info = status.get("planInfo").and_then(Value::as_object);
    if plan_info
        .and_then(|info| info.get("billingStrategy"))
        .and_then(Value::as_str)
        .is_some_and(|strategy| strategy != "BILLING_STRATEGY_QUOTA")
    {
        return Ok((
            Vec::new(),
            plan_info
                .and_then(|info| info.get("planName"))
                .and_then(Value::as_str)
                .map(str::to_string),
        ));
    }
    let mut windows = Vec::new();
    if let Some(window) = quota_window(
        status.get("dailyQuotaRemainingPercent"),
        status.get("dailyQuotaResetAtUnix"),
        plan_info
            .and_then(|info| info.get("hideDailyQuota"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "daily",
        "Daily",
        QuotaWindowScope::Daily,
    )? {
        windows.push(window);
    }
    if let Some(window) = quota_window(
        status.get("weeklyQuotaRemainingPercent"),
        status.get("weeklyQuotaResetAtUnix"),
        plan_info
            .and_then(|info| info.get("hideWeeklyQuota"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "weekly",
        "Weekly",
        QuotaWindowScope::Weekly,
    )? {
        windows.push(window);
    }
    let plan = plan_info
        .and_then(|info| info.get("planName"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok((windows, plan))
}

pub struct DevinAdapter {
    home: PathBuf,
    timeout: Duration,
}

impl Default for DevinAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DevinAdapter {
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

    fn fetch(&self) -> Result<Option<QuotaReport>, String> {
        let Some(token) = devin_token(&self.home)? else {
            return Ok(None);
        };
        let headers = [
            ("connect-protocol-version", "1"),
            ("x-auth-token", token.as_str()),
        ];
        let response = post_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(DEVIN_PLAN_STATUS_URL)
            }
            .with_headers(&headers),
            &serde_json::json!({}),
        )?;
        let (windows, plan) = windows_from_plan_status(&response.body)?;
        if windows.is_empty() {
            return Ok(None);
        }
        let mut report = QuotaReport::new(PROVIDER_ID, "provider_api", windows, DEFAULT_FRESHNESS);
        report.plan = plan;
        Ok(Some(report))
    }
}

impl ProviderAdapter for DevinAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Devin",
            vendor: "Cognition",
            source_kind: SourceKind::RemoteApi,
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.fetch()
    }

    fn account_identity(&self) -> Option<String> {
        devin_token(&self.home).ok().flatten()
    }

    fn health_check(&self) -> AdapterHealth {
        match devin_token(&self.home) {
            Ok(None) => AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "Devin login not found".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("Devin credentials unavailable ({code})"),
            },
            Ok(Some(_)) => match self.fetch() {
                Ok(Some(_)) => AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "Devin quota available".to_string(),
                },
                Ok(None) => AdapterHealth {
                    status: AdapterHealthStatus::Degraded,
                    message: "Devin plan has no published quota windows".to_string(),
                },
                Err(code) => AdapterHealth {
                    status: AdapterHealthStatus::Unhealthy,
                    message: format!("Devin request failed ({code})"),
                },
            },
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        let state = devin_token(&self.home);
        let configured = matches!(state, Ok(Some(_)));
        let error = state.err();
        Ok(DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Devin".to_string(),
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
            detection_error_code: error.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_daily_weekly_and_proto_default_exhaustion() {
        let payload = serde_json::json!({
            "planStatus": {
                "planInfo": {
                    "planName": "Pro",
                    "billingStrategy": "BILLING_STRATEGY_QUOTA"
                },
                "dailyQuotaResetAtUnix": 1790000000,
                "weeklyQuotaRemainingPercent": 40,
                "weeklyQuotaResetAtUnix": 1790500000
            }
        });
        let (windows, plan) = windows_from_plan_status(&payload).expect("quota windows");
        assert_eq!(plan.as_deref(), Some("Pro"));
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].used_percent, Some(100.0));
        assert_eq!(windows[1].used_percent, Some(60.0));
    }

    #[test]
    fn credentials_are_detected_without_network() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(".local").join("share").join("devin");
        std::fs::create_dir_all(&root).expect("auth dir");
        std::fs::write(
            root.join("credentials.toml"),
            "windsurf_api_key = \"fixture\"\napi_server_url = \"https://server.codeium.com\"\n",
        )
        .expect("auth file");
        let adapter = DevinAdapter::with_home(dir.path().to_path_buf());
        assert!(adapter.detect().expect("detect").detected);
    }
}
