//! Qoder and Qoder CN quota adapters.
//!
//! Qoder writes authoritative quota snapshots to its renderer logs. When the
//! user explicitly provides the existing Qoder browser cookie through the
//! process environment, lnwdeck can also call Qoder's read-only account usage
//! endpoint. Secrets are never persisted or surfaced in diagnostics.

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
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

const ADAPTER_VERSION: &str = "0.1.0";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/143 Safari/537.36";
const MAX_LOG_TAIL: u64 = 512 * 1024;
const MAX_RENDERER_LOGS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Site {
    International,
    China,
}

impl Site {
    fn provider_id(self) -> &'static str {
        match self {
            Self::International => "qoder",
            Self::China => "qoder_cn",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::International => "Qoder",
            Self::China => "Qoder CN",
        }
    }

    fn app_dir(self) -> &'static str {
        match self {
            Self::International => "Qoder",
            Self::China => "QoderCN",
        }
    }

    fn env_prefix(self) -> &'static str {
        match self {
            Self::International => "QODER",
            Self::China => "QODER_CN",
        }
    }

    fn origin(self) -> &'static str {
        match self {
            Self::International => "https://qoder.com",
            Self::China => "https://qoder.com.cn",
        }
    }

    fn usage_url(self) -> &'static str {
        match self {
            Self::International => "https://qoder.com/api/v2/me/usages/big_model_credits",
            Self::China => "https://qoder.com.cn/api/v2/me/usages/big_model_credits",
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

#[derive(Debug, Clone, Copy)]
struct QuotaNumbers {
    used: f64,
    total: f64,
    remaining: f64,
    used_percent: f64,
}

fn quota_numbers(summary: &Value) -> Result<QuotaNumbers, String> {
    let used = number(
        summary
            .get("usedValue")
            .or_else(|| summary.get("used_value")),
    )
    .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let total = number(
        summary
            .get("limitValue")
            .or_else(|| summary.get("limit_value")),
    )
    .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let remaining = number(
        summary
            .get("remainingValue")
            .or_else(|| summary.get("remaining_value")),
    )
    .unwrap_or_else(|| (total - used).max(0.0));
    if used < 0.0
        || total < 0.0
        || remaining < 0.0
        || (total == 0.0 && (used != 0.0 || remaining != 0.0))
    {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    let provided = number(
        summary
            .get("usagePercentage")
            .or_else(|| summary.get("usage_percentage")),
    );
    let used_percent = provided.unwrap_or_else(|| {
        if total > 0.0 {
            used / total * 100.0
        } else {
            0.0
        }
    });
    Ok(QuotaNumbers {
        used,
        total,
        remaining,
        used_percent: used_percent.clamp(0.0, 100.0),
    })
}

/// Normalizes Qoder's account usage response, including the shared quota pool.
pub fn quota_from_usage_response(
    payload: &Value,
    provider_id: &str,
) -> Result<QuotaReport, String> {
    let total_container = payload
        .get("totalQuota")
        .or_else(|| payload.get("total_quota"))
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let total_summary = total_container
        .get("quotaSummary")
        .or_else(|| total_container.get("quota_summary"))
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let base = quota_numbers(total_summary)?;
    let shared = payload
        .get("sharedQuota")
        .or_else(|| payload.get("shared_quota"))
        .and_then(|container| {
            container
                .get("quotaSummary")
                .or_else(|| container.get("quota_summary"))
        })
        .map(quota_numbers)
        .transpose()?;
    let used = base.used + shared.map(|value| value.used).unwrap_or(0.0);
    let total = base.total + shared.map(|value| value.total).unwrap_or(0.0);
    let remaining = base.remaining + shared.map(|value| value.remaining).unwrap_or(0.0);
    let used_percent = if shared.is_some() && total > 0.0 {
        used / total * 100.0
    } else {
        base.used_percent
    };
    let mut window = QuotaWindow::from_percent(
        "credits",
        "Credits",
        QuotaWindowScope::Monthly,
        QuotaKind::Credits,
        used_percent.clamp(0.0, 100.0),
        reset_at(
            payload
                .get("nextResetAt")
                .or_else(|| payload.get("next_reset_at")),
        ),
        Confidence::High,
    );
    // QuotaWindow's domain model stores integer counts. Preserve Qoder's real
    // credit totals when the provider returns whole-number credits.
    if total > 0.0 && used >= 0.0 && remaining >= 0.0 {
        let total_u64 = total.round() as u64;
        let used_u64 = used.round().min(total.round()).max(0.0) as u64;
        if let Some(limit) = std::num::NonZeroU64::new(total_u64) {
            window = QuotaWindow::with_limit(
                "credits",
                "Credits",
                QuotaWindowScope::Monthly,
                QuotaKind::Credits,
                used_u64,
                limit,
                window.reset_at,
                Confidence::High,
            );
        }
    }
    Ok(QuotaReport::new(
        provider_id,
        "provider_api",
        vec![window],
        DEFAULT_FRESHNESS,
    ))
}

fn token_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.rfind(marker)? + marker.len();
    let tail = &text[start..];
    let end = tail
        .find(|ch: char| ch == ',' || ch.is_whitespace())
        .unwrap_or(tail.len());
    let token = tail[..end].trim();
    (!token.is_empty()).then_some(token)
}

fn number_after(text: &str, marker: &str) -> Option<f64> {
    token_after(text, marker)?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Parses the latest quota snapshot Qoder writes into `renderer.log`.
pub fn quota_from_renderer_log(text: &str, provider_id: &str) -> Option<QuotaReport> {
    let mut latest = None;
    for offset in text.match_indices("userType=").map(|(offset, _)| offset) {
        let section = &text[offset..];
        let Some(plan) = token_after(section, "userType=") else {
            continue;
        };
        let Some(quota_exceeded) =
            token_after(section, "isQuotaExceeded=").map(|value| value == "true")
        else {
            continue;
        };
        let Some(quota_start) = section.find("userQuota") else {
            continue;
        };
        let quota = &section[quota_start..];
        let (Some(used), Some(total), Some(remaining), Some(reported)) = (
            number_after(quota, "used="),
            number_after(quota, "total="),
            number_after(quota, "remaining="),
            number_after(quota, "percentage="),
        ) else {
            continue;
        };
        let unit = token_after(quota, "unit=").unwrap_or("credits");
        if used < 0.0 || total < 0.0 || remaining < 0.0 {
            continue;
        }
        let used_percent = if total == 0.0 {
            0.0
        } else if quota_exceeded {
            100.0
        } else {
            reported.clamp(0.0, 100.0)
        };
        let window = if let Some(limit) = std::num::NonZeroU64::new(total.round() as u64) {
            QuotaWindow::with_limit(
                "credits",
                if unit == "calls" { "Calls" } else { "Credits" },
                QuotaWindowScope::Monthly,
                if unit == "calls" {
                    QuotaKind::Requests
                } else {
                    QuotaKind::Credits
                },
                used.round().max(0.0).min(total.round()) as u64,
                limit,
                None,
                Confidence::High,
            )
        } else {
            QuotaWindow::from_percent(
                "credits",
                "Credits",
                QuotaWindowScope::Monthly,
                QuotaKind::Credits,
                used_percent,
                None,
                Confidence::High,
            )
        };
        let mut report =
            QuotaReport::new(provider_id, "local_log", vec![window], DEFAULT_FRESHNESS);
        report.plan = Some(plan.to_string());
        latest = Some(report);
    }
    latest
}

fn recent_renderer_logs(root: &Path) -> Vec<PathBuf> {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut files = Vec::new();
    while let Some((dir, depth)) = stack.pop() {
        if files.len() >= 2000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && depth < 4 {
                stack.push((path, depth + 1));
            } else if path.file_name().and_then(|name| name.to_str()) == Some("renderer.log") {
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok());
                files.push((path, modified));
            }
        }
    }
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files
        .into_iter()
        .take(MAX_RENDERER_LOGS)
        .map(|(path, _)| path)
        .collect()
}

fn read_tail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let read_len = length.min(MAX_LOG_TAIL);
    file.seek(SeekFrom::Start(length.saturating_sub(read_len)))
        .ok()?;
    let mut buffer = Vec::with_capacity(read_len as usize);
    file.take(read_len).read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

struct QoderCore {
    site: Site,
    home: PathBuf,
    appdata: PathBuf,
    timeout: Duration,
}

impl QoderCore {
    fn new(site: Site) -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let appdata = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Roaming"));
        Self {
            site,
            home,
            appdata,
            timeout: Duration::from_secs(10),
        }
    }

    #[cfg(test)]
    fn with_roots(site: Site, home: PathBuf, appdata: PathBuf) -> Self {
        Self {
            site,
            home,
            appdata,
            timeout: Duration::from_secs(1),
        }
    }

    fn log_root(&self) -> PathBuf {
        let prefix = self.site.env_prefix();
        if let Ok(value) = std::env::var(format!("{prefix}_LOG_ROOT")) {
            let value = value.trim();
            if !value.is_empty() {
                return PathBuf::from(value);
            }
        }
        if let Ok(value) = std::env::var(format!("{prefix}_HOME")) {
            let value = value.trim();
            if !value.is_empty() {
                return PathBuf::from(value).join("logs");
            }
        }
        if cfg!(target_os = "macos") {
            return self
                .home
                .join("Library")
                .join("Application Support")
                .join(self.site.app_dir())
                .join("logs");
        }
        if cfg!(target_os = "linux") {
            return self
                .home
                .join(".config")
                .join(self.site.app_dir())
                .join("logs");
        }
        self.appdata.join(self.site.app_dir()).join("logs")
    }

    fn cookie(&self) -> Option<String> {
        std::env::var(format!("{}_COOKIE", self.site.env_prefix()))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn log_quota(&self) -> Option<QuotaReport> {
        for path in recent_renderer_logs(&self.log_root()) {
            let Some(text) = read_tail(&path) else {
                continue;
            };
            if let Some(report) = quota_from_renderer_log(&text, self.site.provider_id()) {
                return Some(report);
            }
        }
        None
    }

    fn api_quota(&self, cookie: &str) -> Result<QuotaReport, String> {
        let origin = self.site.origin();
        let referer = format!("{origin}/account/usage");
        let headers = [
            ("accept", "application/json, text/plain, */*"),
            ("accept-language", "en-US,en;q=0.9"),
            ("origin", origin),
            ("referer", referer.as_str()),
            ("user-agent", USER_AGENT),
            ("x-requested-with", "XMLHttpRequest"),
            ("bx-v", "2.5.35"),
        ];
        let response = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(self.site.usage_url())
            }
            .browser_cookie(cookie)
            .with_headers(&headers),
        )?;
        quota_from_usage_response(&response.body, self.site.provider_id())
    }

    fn fetch(&self) -> Result<Option<QuotaReport>, String> {
        // Match TokenTracker's availability behavior: local provider evidence is
        // preferred because it needs no browser secret or network call.
        if let Some(report) = self.log_quota() {
            return Ok(Some(report));
        }
        let Some(cookie) = self.cookie() else {
            return Ok(None);
        };
        self.api_quota(&cookie).map(Some)
    }

    fn detection(&self) -> DetectionResult {
        let log_exists = self.log_root().is_dir();
        let cookie = self.cookie().is_some();
        let detected = log_exists || cookie;
        DetectionResult {
            provider_id: self.site.provider_id().to_string(),
            display_name: self.site.display_name().to_string(),
            enabled: true,
            detected,
            detection_method: if log_exists {
                "local_log".to_string()
            } else {
                "environment_cookie".to_string()
            },
            source_type: if log_exists {
                "local_log".to_string()
            } else {
                "remote_api".to_string()
            },
            source_exists: detected,
            permission_state: if detected {
                "read_ok".to_string()
            } else {
                "not_found".to_string()
            },
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: if detected {
                String::new()
            } else {
                "NOT_CONFIGURED".to_string()
            },
        }
    }

    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: self.site.provider_id(),
            display_name: self.site.display_name(),
            vendor: "Alibaba",
            source_kind: SourceKind::LocalLog,
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn health(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.detected {
            return AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: format!("{} data not found", self.site.display_name()),
            };
        }
        match self.fetch() {
            Ok(Some(_)) => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: format!("{} quota available", self.site.display_name()),
            },
            Ok(None) => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: format!("{} quota not found", self.site.display_name()),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("{} quota failed ({code})", self.site.display_name()),
            },
        }
    }
}

pub struct QoderAdapter(QoderCore);
pub struct QoderCnAdapter(QoderCore);

impl Default for QoderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl QoderAdapter {
    pub fn new() -> Self {
        Self(QoderCore::new(Site::International))
    }
}

impl Default for QoderCnAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl QoderCnAdapter {
    pub fn new() -> Self {
        Self(QoderCore::new(Site::China))
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

impl_adapter!(QoderAdapter);
impl_adapter!(QoderCnAdapter);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_payload_combines_base_and_shared_quota() {
        let payload = serde_json::json!({
            "totalQuota": {"quotaSummary": {"usedValue": 20, "limitValue": 100, "remainingValue": 80}},
            "sharedQuota": {"quotaSummary": {"usedValue": 10, "limitValue": 50, "remainingValue": 40}},
            "nextResetAt": 1_800_000_000
        });
        let report = quota_from_usage_response(&payload, "qoder").expect("quota");
        assert_eq!(report.windows.len(), 1);
        assert_eq!(report.windows[0].used, 30);
        assert_eq!(report.windows[0].limit, Some(150));
        assert_eq!(report.windows[0].remaining, Some(120));
    }

    #[test]
    fn renderer_log_yields_latest_quota_snapshot() {
        let text = "prefix userType=PRO isQuotaExceeded=false userQuota used=5, total=100, remaining=95, percentage=5, unit=credits\n\
                    later userType=ULTIMATE isQuotaExceeded=true userQuota used=100, total=100, remaining=0, percentage=99, unit=calls";
        let report = quota_from_renderer_log(text, "qoder").expect("quota");
        assert_eq!(report.plan.as_deref(), Some("ULTIMATE"));
        assert_eq!(report.windows[0].used_percent, Some(100.0));
    }

    #[test]
    fn local_log_presence_drives_detection_without_network() {
        let root = tempfile::tempdir().expect("temp");
        let appdata = root.path().join("AppData");
        std::fs::create_dir_all(appdata.join("Qoder").join("logs")).expect("logs");
        let adapter =
            QoderCore::with_roots(Site::International, root.path().to_path_buf(), appdata);
        assert!(adapter.detection().detected);
    }
}
