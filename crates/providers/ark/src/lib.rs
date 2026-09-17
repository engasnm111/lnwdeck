//! Volcengine Ark Coding Plan and Agent Plan quota adapters.
//!
//! Both products expose their account quota through the already-installed
//! `arkcli` command. lnwdeck invokes only read-only quota/profile commands,
//! never stores Ark credentials, and enforces a bounded command timeout.

use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::command::{resolve_binary, run_command};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

const ADAPTER_VERSION: &str = "0.1.0";
const CLI_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArkProduct {
    Coding,
    Agent,
}

impl ArkProduct {
    fn provider_id(self) -> &'static str {
        match self {
            Self::Coding => "ark_coding_plan",
            Self::Agent => "ark_agent_plan",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Coding => "Ark Coding Plan",
            Self::Agent => "Ark Agent Plan",
        }
    }

    fn product_key(self) -> &'static str {
        match self {
            Self::Coding => "coding-plan",
            Self::Agent => "agent-plan",
        }
    }

    fn accepts_period(self, label: &str) -> Option<(&'static str, &'static str, QuotaWindowScope)> {
        match label {
            "session" | "5h" => Some(("5h", "5-hour", QuotaWindowScope::Rolling)),
            "weekly" => Some(("weekly", "Weekly", QuotaWindowScope::Weekly)),
            "monthly" => Some(("monthly", "Monthly", QuotaWindowScope::Monthly)),
            // Agent Plan additionally publishes a visual-only daily bucket.
            // It is intentionally not mixed into the three account quota bars.
            "daily" if self == Self::Agent => None,
            _ => None,
        }
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

fn reset_at(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let value = value?;
    if let Some(raw) = number(Some(value)) {
        if raw <= 0.0 {
            return None;
        }
        let milliseconds = if raw > 10_000_000_000.0 {
            raw as i64
        } else {
            (raw * 1000.0) as i64
        };
        return DateTime::from_timestamp_millis(milliseconds);
    }
    DateTime::parse_from_rfc3339(value.as_str()?.trim())
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn used_percent(period: &Value) -> Option<f64> {
    if let Some(percent) = number(period.get("percent")) {
        return Some(percent.clamp(0.0, 100.0));
    }
    let used = number(period.get("used"))?;
    let total = number(period.get("total"))?;
    if used < 0.0 || total <= 0.0 {
        return None;
    }
    Some((used / total * 100.0).clamp(0.0, 100.0))
}

fn tier_label(value: Option<&Value>) -> Option<String> {
    let raw = value?.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    let known = match lower.as_str() {
        "lite" => "Lite",
        "pro" => "Pro",
        "small" => "Small",
        "medium" => "Medium",
        "large" => "Large",
        "max" => "Max",
        _ => raw,
    };
    Some(known.to_string())
}

fn windows_from_usage(
    payload: &Value,
    product: ArkProduct,
) -> Result<(Vec<QuotaWindow>, Option<String>), String> {
    let items = payload
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let Some(item) = items.iter().find(|entry| {
        entry.get("product").and_then(Value::as_str) == Some(product.product_key())
            && entry.get("subscribed").and_then(Value::as_bool) == Some(true)
    }) else {
        return Ok((Vec::new(), None));
    };
    let periods = item
        .get("periods")
        .and_then(Value::as_array)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let mut windows = Vec::new();
    for period in periods {
        let Some(label) = period.get("label").and_then(Value::as_str) else {
            continue;
        };
        let Some((key, display, scope)) = product.accepts_period(label) else {
            continue;
        };
        let Some(percent) = used_percent(period) else {
            continue;
        };
        if windows
            .iter()
            .any(|window: &QuotaWindow| window.window_key == key)
        {
            continue;
        }
        windows.push(QuotaWindow::from_percent(
            key,
            display,
            scope,
            QuotaKind::Requests,
            percent,
            reset_at(period.get("reset_at")),
            Confidence::High,
        ));
    }
    if windows.is_empty() {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    Ok((windows, tier_label(item.get("tier"))))
}

fn tier_from_plans(payload: &Value, product: ArkProduct) -> Option<String> {
    payload
        .get("plans")
        .and_then(Value::as_array)?
        .iter()
        .find(|entry| entry.get("key").and_then(Value::as_str) == Some(product.product_key()))
        .and_then(|entry| tier_label(entry.get("tier")))
}

fn run_json(binary: &PathBuf, args: &[&str], timeout: Duration) -> Result<Value, String> {
    let output = run_command(binary, args, timeout)?;
    if output.timed_out {
        return Err("PROVIDER_TIMEOUT".to_string());
    }
    if output.status != Some(0) {
        return Err("SOURCE_UNAVAILABLE".to_string());
    }
    serde_json::from_str(&output.stdout).map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())
}

struct ArkAdapterCore {
    product: ArkProduct,
    binary: Option<PathBuf>,
}

impl ArkAdapterCore {
    fn new(product: ArkProduct) -> Self {
        Self {
            product,
            binary: resolve_binary("arkcli"),
        }
    }

    #[cfg(test)]
    fn without_cli(product: ArkProduct) -> Self {
        Self {
            product,
            binary: None,
        }
    }

    fn fetch(&self) -> Result<Option<QuotaReport>, String> {
        let Some(binary) = &self.binary else {
            return Ok(None);
        };
        let payload = run_json(binary, &["usage", "plan", "--format", "json"], CLI_TIMEOUT)?;
        let (windows, mut plan) = windows_from_usage(&payload, self.product)?;
        if windows.is_empty() {
            return Ok(None);
        }
        if plan.is_none() {
            if let Ok(plans) = run_json(
                binary,
                &["plans", "get", "--format", "json"],
                Duration::from_secs(4),
            ) {
                plan = tier_from_plans(&plans, self.product);
            }
        }
        let mut report = QuotaReport::new(
            self.product.provider_id(),
            "provider_cli",
            windows,
            DEFAULT_FRESHNESS,
        );
        report.plan = plan;
        Ok(Some(report))
    }

    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: self.product.provider_id(),
            display_name: self.product.display_name(),
            vendor: "Volcengine",
            source_kind: SourceKind::RemoteApi,
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn health(&self) -> AdapterHealth {
        if self.binary.is_none() {
            return AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "arkcli not found".to_string(),
            };
        }
        match self.fetch() {
            Ok(Some(_)) => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: format!("{} quota available", self.product.display_name()),
            },
            Ok(None) => AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: format!("{} subscription not found", self.product.display_name()),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("{} quota failed ({code})", self.product.display_name()),
            },
        }
    }

    fn detection(&self) -> DetectionResult {
        let installed = self.binary.is_some();
        DetectionResult {
            provider_id: self.product.provider_id().to_string(),
            display_name: self.product.display_name().to_string(),
            enabled: true,
            detected: installed,
            detection_method: "cli".to_string(),
            source_type: "provider_cli".to_string(),
            source_exists: installed,
            permission_state: if installed {
                "cli_found".to_string()
            } else {
                "not_found".to_string()
            },
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: if installed {
                String::new()
            } else {
                "NOT_CONFIGURED".to_string()
            },
        }
    }
}

pub struct ArkCodingPlanAdapter(ArkAdapterCore);
pub struct ArkAgentPlanAdapter(ArkAdapterCore);

impl Default for ArkCodingPlanAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ArkCodingPlanAdapter {
    pub fn new() -> Self {
        Self(ArkAdapterCore::new(ArkProduct::Coding))
    }
}

impl Default for ArkAgentPlanAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ArkAgentPlanAdapter {
    pub fn new() -> Self {
        Self(ArkAdapterCore::new(ArkProduct::Agent))
    }
}

macro_rules! impl_adapter {
    ($adapter:ty) => {
        impl ProviderAdapter for $adapter {
            fn descriptor(&self) -> AdapterDescriptor {
                self.0.descriptor()
            }

            fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
                self.0.fetch()
            }

            fn health_check(&self) -> AdapterHealth {
                self.0.health()
            }

            fn required_permissions(&self) -> Vec<Permission> {
                vec![Permission::FileSystem, Permission::Network]
            }

            fn detect(&self) -> Result<DetectionResult, String> {
                Ok(self.0.detection())
            }
        }
    };
}

impl_adapter!(ArkCodingPlanAdapter);
impl_adapter!(ArkAgentPlanAdapter);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coding_plan_normalizes_three_windows() {
        let payload = serde_json::json!({
            "items": [{
                "product": "coding-plan",
                "subscribed": true,
                "tier": "pro",
                "periods": [
                    {"label": "session", "percent": 12.5, "reset_at": "2026-09-18T10:00:00Z"},
                    {"label": "weekly", "used": 20, "total": 100},
                    {"label": "monthly", "percent": 55}
                ]
            }]
        });
        let (windows, plan) = windows_from_usage(&payload, ArkProduct::Coding).expect("windows");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].used_percent, Some(12.5));
        assert_eq!(windows[1].used_percent, Some(20.0));
        assert_eq!(plan.as_deref(), Some("Pro"));
    }

    #[test]
    fn agent_plan_skips_visual_daily_bucket() {
        let payload = serde_json::json!({
            "items": [{
                "product": "agent-plan",
                "subscribed": true,
                "tier": "large",
                "periods": [
                    {"label": "daily", "percent": 90},
                    {"label": "5h", "used": 10, "total": 40},
                    {"label": "weekly", "percent": 30}
                ]
            }]
        });
        let (windows, plan) = windows_from_usage(&payload, ArkProduct::Agent).expect("windows");
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "5-hour");
        assert_eq!(windows[0].used_percent, Some(25.0));
        assert_eq!(plan.as_deref(), Some("Large"));
    }

    #[test]
    fn missing_cli_is_not_detected_and_makes_no_request() {
        let adapter = ArkAdapterCore::without_cli(ArkProduct::Coding);
        assert!(!adapter.detection().detected);
        assert!(adapter.fetch().expect("no cli").is_none());
    }
}
