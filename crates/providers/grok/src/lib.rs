//! Grok (xAI) quota collector.
//!
//! Grok Build users get their subscription credit pool from the billing API
//! authenticated by the CLI's existing OIDC session in `~/.grok/auth.json`.
//! API-key users retain the xAI rate-limit fallback from `GET /v1/api-key`.
//! Both paths are read-only, bounded by request timeouts, and return only
//! normalized quota metadata.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, post_form, HttpResponse, JsonRequest};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use lnwdeck_windows_integration::CredentialStore;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::time::Duration;

const PROVIDER_ID: &str = "xai_grok";
const ADAPTER_VERSION: &str = "0.3.0";
const DEFAULT_ENDPOINT: &str = "https://api.x.ai/v1/api-key";
const GROK_BUILD_BILLING_BASE: &str = "https://cli-chat-proxy.grok.com";
const GROK_OAUTH_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
/// Rate-limit headers xAI publishes on API responses.
const RATE_HEADERS: &[&str] = &[
    "x-ratelimit-limit-requests",
    "x-ratelimit-remaining-requests",
    "x-ratelimit-reset-requests",
];

#[derive(Debug, Clone)]
struct GrokBuildAuth {
    access_token: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn grok_build_auth(home: &Path) -> Option<GrokBuildAuth> {
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".grok").join("auth.json")).ok()?)
            .ok()?;
    let object = value.as_object()?;
    let mut fallback = None;
    for (scope, entry) in object {
        let Some(entry) = entry.as_object() else {
            continue;
        };
        let access_token = entry
            .get("key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let refresh_token = entry
            .get("refresh_token")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let client_id = entry
            .get("oidc_client_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                scope
                    .rsplit_once("::")
                    .map(|(_, id)| id.trim().to_string())
                    .filter(|id| !id.is_empty())
            });
        let expires_at = entry
            .get("expires_at")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc));
        let auth = GrokBuildAuth {
            access_token,
            refresh_token,
            client_id,
            expires_at,
        };
        if auth.access_token.is_some() {
            return Some(auth);
        }
        if auth.refresh_token.is_some() && auth.client_id.is_some() && fallback.is_none() {
            fallback = Some(auth);
        }
    }
    fallback
}

fn json_number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        serde_json::Value::Object(map) => json_number(map.get("val")),
        _ => None,
    }
    .filter(|number| number.is_finite())
}

fn grok_reset(value: Option<&serde_json::Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value?.as_str()?.trim())
        .ok()
        .map(|value| value.with_timezone(&chrono::Utc))
}

fn period_scope(period: Option<&str>) -> QuotaWindowScope {
    let period = period.unwrap_or_default().to_ascii_uppercase();
    if period.contains("DAY") && !period.contains("HOUR") {
        QuotaWindowScope::Daily
    } else if period.contains("WEEK") {
        QuotaWindowScope::Weekly
    } else if period.contains("MONTH") {
        QuotaWindowScope::Monthly
    } else {
        QuotaWindowScope::Other
    }
}

/// Normalizes Grok Build's unified billing payload and the legacy monthly
/// credit payload used by older accounts.
pub fn grok_build_windows(payload: &serde_json::Value) -> Result<Vec<QuotaWindow>, String> {
    let config = payload
        .get("config")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let current_period = config.get("currentPeriod");
    let reset_at = current_period
        .and_then(|period| grok_reset(period.get("end")))
        .or_else(|| grok_reset(config.get("billingPeriodEnd")));
    let scope = period_scope(
        current_period
            .and_then(|period| period.get("type"))
            .and_then(serde_json::Value::as_str),
    );
    let mut used_percent = json_number(config.get("creditUsagePercent"));
    if used_percent.is_none() {
        if let Some(items) = config
            .get("productUsage")
            .and_then(serde_json::Value::as_array)
        {
            let values: Vec<f64> = items
                .iter()
                .filter_map(|item| json_number(item.get("usagePercent")))
                .collect();
            if !values.is_empty() {
                used_percent = Some(values.iter().sum::<f64>());
            }
        }
    }
    if used_percent.is_none() {
        if let (Some(used), Some(limit)) = (
            json_number(config.get("used")),
            json_number(config.get("monthlyLimit")),
        ) {
            if limit > 0.0 {
                used_percent = Some(used / limit * 100.0);
            }
        }
    }
    if used_percent.is_none()
        && current_period.is_some()
        && config.get("creditUsagePercent").is_none()
        && config.get("productUsage").is_none()
    {
        used_percent = Some(0.0);
    }

    let mut windows = Vec::new();
    if let Some(percent) = used_percent.filter(|value| value.is_finite()) {
        windows.push(QuotaWindow::from_percent(
            "grok_build",
            "Grok Build",
            scope,
            QuotaKind::Credits,
            percent.clamp(0.0, 100.0),
            reset_at,
            Confidence::High,
        ));
    }
    if let (Some(used), Some(limit)) = (
        json_number(config.get("onDemandUsed")),
        json_number(config.get("onDemandCap")),
    ) {
        if limit > 0.0 {
            windows.push(QuotaWindow::from_percent(
                "grok_on_demand",
                "On-demand credits",
                scope,
                QuotaKind::Credits,
                (used / limit * 100.0).clamp(0.0, 100.0),
                reset_at,
                Confidence::High,
            ));
        }
    }
    if windows.is_empty() {
        Err("SOURCE_SCHEMA_MISMATCH".to_string())
    } else {
        Ok(windows)
    }
}

pub struct GrokAdapter {
    endpoint: String,
    home: PathBuf,
    timeout: Duration,
}

impl Default for GrokAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            endpoint: std::env::var("LNWDECK_XAI_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string()),
            home,
            timeout: Duration::from_secs(10),
        }
    }

    /// Adapter pinned to an explicit endpoint (used by tests).
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            home: PathBuf::new(),
            timeout: Duration::from_secs(5),
        }
    }

    fn api_key(&self) -> Result<String, String> {
        match CredentialStore::get(PROVIDER_ID) {
            Ok(key) if !key.trim().is_empty() => Ok(key),
            Ok(_) => Err("NOT_CONFIGURED".to_string()),
            Err(lnwdeck_windows_integration::CredentialError::NotFound) => {
                Err("NOT_CONFIGURED".to_string())
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn build_access_token(&self, force_refresh: bool) -> Result<Option<String>, String> {
        let Some(auth) = grok_build_auth(&self.home) else {
            return Ok(None);
        };
        let expired = auth
            .expires_at
            .is_some_and(|expiry| expiry <= chrono::Utc::now() + chrono::Duration::seconds(60));
        if !force_refresh && !expired {
            if let Some(token) = auth.access_token {
                return Ok(Some(token));
            }
        }
        let refresh_token = auth
            .refresh_token
            .as_deref()
            .ok_or_else(|| "AUTH_EXPIRED".to_string())?;
        let client_id = auth
            .client_id
            .as_deref()
            .ok_or_else(|| "AUTH_EXPIRED".to_string())?;
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ];
        let response = post_form(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(GROK_OAUTH_TOKEN_URL)
            },
            &form,
        )?;
        response
            .body
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .map(Some)
            .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())
    }

    fn fetch_build_with_token(&self, token: &str) -> Result<QuotaReport, String> {
        let credits = format!("{GROK_BUILD_BILLING_BASE}/v1/billing?format=credits");
        let response = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(&credits)
            }
            .bearer(token),
        )
        .or_else(|error| {
            if error == "AUTH_EXPIRED" {
                return Err(error);
            }
            let legacy = format!("{GROK_BUILD_BILLING_BASE}/v1/billing");
            get_json(
                JsonRequest {
                    timeout: self.timeout,
                    ..JsonRequest::new(&legacy)
                }
                .bearer(token),
            )
        })?;
        let windows = grok_build_windows(&response.body)?;
        Ok(QuotaReport::new(
            PROVIDER_ID,
            "provider_api",
            windows,
            DEFAULT_FRESHNESS,
        ))
    }

    fn fetch_build(&self) -> Result<Option<QuotaReport>, String> {
        let Some(token) = self.build_access_token(false)? else {
            return Ok(None);
        };
        match self.fetch_build_with_token(&token) {
            Ok(report) => Ok(Some(report)),
            Err(code) if code == "AUTH_EXPIRED" => {
                let Some(refreshed) = self.build_access_token(true)? else {
                    return Err(code);
                };
                self.fetch_build_with_token(&refreshed).map(Some)
            }
            Err(code) => Err(code),
        }
    }

    /// Builds a requests window from rate-limit headers.
    ///
    /// Returns `None` when the headers are absent or unusable; the caller then
    /// reports that no quota was published rather than inventing one.
    pub fn window_from_headers(response: &HttpResponse) -> Option<QuotaWindow> {
        let limit = NonZeroU64::new(response.header_u64("x-ratelimit-limit-requests")?)?;
        let remaining = response
            .header_u64("x-ratelimit-remaining-requests")
            .unwrap_or(0)
            .min(limit.get());
        let used = limit.get() - remaining;
        let reset_at = response
            .header_u64("x-ratelimit-reset-requests")
            .and_then(|seconds| {
                chrono::Utc::now().checked_add_signed(chrono::Duration::seconds(seconds as i64))
            });
        Some(QuotaWindow::with_limit(
            "requests",
            "Requests",
            QuotaWindowScope::Rolling,
            QuotaKind::Requests,
            used,
            limit,
            reset_at,
            Confidence::High,
        ))
    }

    fn fetch(&self) -> Result<QuotaReport, String> {
        let key = self.api_key()?;
        let response = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(&self.endpoint)
            }
            .bearer(&key)
            .capture(RATE_HEADERS),
        )?;

        let window = Self::window_from_headers(&response)
            .ok_or_else(|| "QUOTA_NOT_PUBLISHED".to_string())?;
        Ok(QuotaReport::new(
            PROVIDER_ID,
            "provider_api",
            vec![window],
            DEFAULT_FRESHNESS,
        ))
    }
}

impl ProviderAdapter for GrokAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Grok",
            vendor: "xAI",
            source_kind: SourceKind::RemoteApi,
            // xAI exposes no usage history endpoint.
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::ApiKey,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        match self.fetch_build()? {
            Some(report) => Ok(Some(report)),
            None => self.fetch().map(Some),
        }
    }

    fn account_identity(&self) -> Option<String> {
        grok_build_auth(&self.home)
            .and_then(|auth| auth.access_token)
            .or_else(|| self.api_key().ok())
    }

    fn health_check(&self) -> AdapterHealth {
        if grok_build_auth(&self.home).is_some() {
            return match self.fetch_build() {
                Ok(Some(_)) => AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "Grok Build billing available".to_string(),
                },
                Ok(None) => AdapterHealth {
                    status: AdapterHealthStatus::NotConfigured,
                    message: "Grok Build is not configured".to_string(),
                },
                Err(code) => AdapterHealth {
                    status: AdapterHealthStatus::Unhealthy,
                    message: format!("Grok Build request failed ({code})"),
                },
            };
        }
        match self.api_key() {
            Err(code) if code == "NOT_CONFIGURED" => AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "Grok Build login or xAI API key not found".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("xAI credential unreadable ({code})"),
            },
            Ok(_) => match self.fetch() {
                Ok(_) => AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "xAI key verified and rate limit reported".to_string(),
                },
                Err(code) if code == "QUOTA_NOT_PUBLISHED" => AdapterHealth {
                    status: AdapterHealthStatus::Degraded,
                    message: "xAI key verified but no quota is published".to_string(),
                },
                Err(code) => AdapterHealth {
                    status: AdapterHealthStatus::Unhealthy,
                    message: format!("xAI request failed ({code})"),
                },
            },
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FileSystem,
            Permission::Credential,
            Permission::Network,
        ]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        let build_configured = grok_build_auth(&self.home).is_some();
        let api_configured = self.api_key().is_ok();
        let configured = build_configured || api_configured;
        Ok(DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Grok".to_string(),
            enabled: true,
            detected: configured,
            detection_method: if build_configured {
                "local_auth".to_string()
            } else {
                "credential".to_string()
            },
            source_type: "remote_api".to_string(),
            source_exists: configured,
            permission_state: if build_configured {
                "local_auth_found".to_string()
            } else if api_configured {
                "credential_stored".to_string()
            } else {
                "credential_required".to_string()
            },
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(chrono::Utc::now().to_rfc3339()),
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
    use std::collections::HashMap;

    fn response_with(headers: &[(&str, &str)]) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: serde_json::json!({ "api_key_id": "id", "name": "key" }),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn descriptor_is_consistent_and_credential_backed() {
        let adapter = GrokAdapter::with_endpoint("https://example.invalid/v1/api-key");
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.id, "xai_grok");
        assert_eq!(descriptor.quota_support, ChannelSupport::Native);
        assert_eq!(descriptor.usage_support, ChannelSupport::Unsupported);
        assert!(descriptor.needs_credentials());
    }

    #[test]
    fn grok_build_billing_prefers_unified_usage_and_sums_product_usage() {
        let payload = serde_json::json!({
            "config": {
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "start": "2026-09-14T00:00:00Z",
                    "end": "2026-09-21T00:00:00Z"
                },
                "productUsage": [
                    {"usagePercent": 20},
                    {"usagePercent": 15}
                ],
                "onDemandCap": {"val": 100},
                "onDemandUsed": {"val": 25}
            }
        });
        let windows = grok_build_windows(&payload).expect("billing windows");
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].used_percent, Some(35.0));
        assert_eq!(windows[0].scope, QuotaWindowScope::Weekly);
        assert_eq!(windows[1].used_percent, Some(25.0));
        assert!(windows.iter().all(|window| window.reset_at.is_some()));
    }

    #[test]
    fn grok_build_billing_accepts_legacy_monthly_counters() {
        let payload = serde_json::json!({
            "config": {
                "monthlyLimit": {"val": 200},
                "used": {"val": 50},
                "billingPeriodEnd": "2026-10-01T00:00:00Z"
            }
        });
        let windows = grok_build_windows(&payload).expect("billing windows");
        assert_eq!(windows[0].used_percent, Some(25.0));
    }

    #[test]
    fn rate_limit_headers_become_a_real_requests_window() {
        let response = response_with(&[
            ("x-ratelimit-limit-requests", "480"),
            ("x-ratelimit-remaining-requests", "300"),
            ("x-ratelimit-reset-requests", "60"),
        ]);
        let window = GrokAdapter::window_from_headers(&response).expect("window");
        assert_eq!(window.kind, QuotaKind::Requests);
        assert_eq!(window.limit, Some(480));
        assert_eq!(window.used, 180);
        assert_eq!(window.remaining, Some(300));
        assert_eq!(window.remaining_percent, Some(62.5));
        assert!(window.reset_at.is_some(), "reset countdown is real");
        window.check_invariants().expect("consistent window");
    }

    #[test]
    fn remaining_above_the_limit_is_clamped_not_trusted_blindly() {
        let response = response_with(&[
            ("x-ratelimit-limit-requests", "100"),
            ("x-ratelimit-remaining-requests", "999"),
        ]);
        let window = GrokAdapter::window_from_headers(&response).expect("window");
        assert_eq!(window.remaining, Some(100));
        assert_eq!(window.used, 0);
        window.check_invariants().expect("consistent window");
    }

    #[test]
    fn missing_headers_produce_no_window() {
        assert!(
            GrokAdapter::window_from_headers(&response_with(&[])).is_none(),
            "no published limit must not become a fabricated window"
        );
        assert!(GrokAdapter::window_from_headers(&response_with(&[(
            "x-ratelimit-limit-requests",
            "0"
        )]))
        .is_none());
        assert!(GrokAdapter::window_from_headers(&response_with(&[(
            "x-ratelimit-limit-requests",
            "unknown"
        )]))
        .is_none());
    }

    #[test]
    fn without_a_stored_key_no_request_is_attempted() {
        let adapter = GrokAdapter::with_endpoint("https://127.0.0.1:1/v1/api-key");
        let _ = CredentialStore::delete(PROVIDER_ID);
        assert_eq!(adapter.collect_quota(), Err("NOT_CONFIGURED".to_string()));
        assert_eq!(
            adapter.health_check().status,
            AdapterHealthStatus::NotConfigured
        );
        assert!(!adapter.detect().expect("detect").detected);
    }

    #[test]
    fn unsupported_usage_channel_is_never_a_successful_collection() {
        let adapter = GrokAdapter::with_endpoint("https://example.invalid/v1/api-key");
        let result = adapter.collect_usage_with_cursor(None);
        assert!(result.batch.is_none());
        assert_eq!(result.outcome.error_code, "NOT_SUPPORTED");
    }
}
