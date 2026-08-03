//! OpenRouter quota collector.
//!
//! OpenRouter publishes the credit balance and rate limit of an API key at
//! `GET /api/v1/key`. This adapter reads that endpoint with a user-supplied key
//! taken from the Windows Credential Manager and turns the response into real
//! quota windows:
//!
//! - a credits window with a known limit when the key has a credit cap, or a
//!   usage-only credits window when the account is uncapped;
//! - a requests window built from the published rate limit.
//!
//! Nothing is requested until the user has stored a key: without one the
//! adapter reports `NOT_CONFIGURED` and performs no network access. Credit
//! amounts are carried in micro-credits (1 credit = 1_000_000) so fractional
//! balances survive the integer quota model without rounding.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaStatus, QuotaWindow, QuotaWindowScope,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, JsonRequest};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use lnwdeck_windows_integration::CredentialStore;
use std::num::NonZeroU64;
use std::time::Duration;

const PROVIDER_ID: &str = "openrouter_api";
const ADAPTER_VERSION: &str = "0.2.0";
const DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/v1/key";
/// One credit expressed in the integer units stored in quota windows.
pub const MICRO_CREDITS: f64 = 1_000_000.0;

pub struct OpenRouterAdapter {
    endpoint: String,
    timeout: Duration,
}

impl Default for OpenRouterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenRouterAdapter {
    pub fn new() -> Self {
        Self {
            endpoint: std::env::var("LNWDECK_OPENROUTER_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string()),
            timeout: Duration::from_secs(10),
        }
    }

    /// Adapter pinned to an explicit endpoint (used by tests).
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout: Duration::from_secs(5),
        }
    }

    /// The stored API key, or `NOT_CONFIGURED` when the user has not entered
    /// one. No request is made without it.
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

    /// Converts the `/key` response into quota windows.
    ///
    /// Returns an error when the payload does not contain the fields this
    /// adapter needs, instead of emitting an empty or guessed report.
    pub fn windows_from_payload(payload: &serde_json::Value) -> Result<Vec<QuotaWindow>, String> {
        let data = payload
            .get("data")
            .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        let usage = data
            .get("usage")
            .and_then(|value| value.as_f64())
            .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        if usage < 0.0 {
            return Err("SOURCE_SCHEMA_MISMATCH".to_string());
        }
        let used_micro = (usage * MICRO_CREDITS).round() as u64;

        let mut windows = Vec::new();
        match data.get("limit") {
            Some(serde_json::Value::Number(number)) => {
                let limit = number
                    .as_f64()
                    .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
                let limit_micro = (limit * MICRO_CREDITS).round() as u64;
                match NonZeroU64::new(limit_micro) {
                    Some(limit_micro) => windows.push(QuotaWindow::with_limit(
                        "credits",
                        "Credits",
                        QuotaWindowScope::Other,
                        QuotaKind::Credits,
                        used_micro,
                        limit_micro,
                        None,
                        Confidence::High,
                    )),
                    // A declared limit of zero is not a usable limit.
                    None => windows.push(QuotaWindow::usage_only(
                        "credits",
                        "Credits",
                        QuotaWindowScope::Other,
                        QuotaKind::Credits,
                        used_micro,
                        None,
                        Confidence::High,
                    )),
                }
            }
            // `limit: null` means the account has no credit cap.
            Some(serde_json::Value::Null) | None => windows.push(QuotaWindow::usage_only(
                "credits",
                "Credits",
                QuotaWindowScope::Other,
                QuotaKind::Credits,
                used_micro,
                None,
                Confidence::High,
            )),
            Some(_) => return Err("SOURCE_SCHEMA_MISMATCH".to_string()),
        }

        // Rate limit, when published, is a real requests-per-interval limit.
        if let Some(rate) = data.get("rate_limit") {
            if let Some(requests) = rate.get("requests").and_then(|value| value.as_u64()) {
                if let Some(limit) = NonZeroU64::new(requests) {
                    let interval = rate
                        .get("interval")
                        .and_then(|value| value.as_str())
                        .unwrap_or("interval");
                    windows.push(QuotaWindow::with_limit(
                        "rate",
                        format!("Requests per {interval}"),
                        QuotaWindowScope::Rolling,
                        QuotaKind::Requests,
                        0,
                        limit,
                        None,
                        Confidence::High,
                    ));
                }
            }
        }

        Ok(windows)
    }

    fn fetch(&self) -> Result<QuotaReport, String> {
        let key = self.api_key()?;
        let response = get_json(
            JsonRequest {
                timeout: self.timeout,
                ..JsonRequest::new(&self.endpoint)
            }
            .bearer(&key),
        )?;
        let windows = Self::windows_from_payload(&response.body)?;
        if windows.is_empty() {
            return Err("SOURCE_SCHEMA_MISMATCH".to_string());
        }
        let mut report = QuotaReport::new(PROVIDER_ID, "provider_api", windows, DEFAULT_FRESHNESS);
        if let Some(free_tier) = response
            .body
            .get("data")
            .and_then(|data| data.get("is_free_tier"))
            .and_then(|value| value.as_bool())
        {
            report.plan = Some(if free_tier { "Free tier" } else { "Paid" }.to_string());
        }
        report.status = QuotaStatus::Fresh;
        Ok(report)
    }
}

impl ProviderAdapter for OpenRouterAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "OpenRouter",
            vendor: "OpenRouter",
            source_kind: SourceKind::RemoteApi,
            // OpenRouter reports account credits, not a per-request history.
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::ApiKey,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.fetch().map(Some)
    }

    fn health_check(&self) -> AdapterHealth {
        match self.api_key() {
            Err(code) if code == "NOT_CONFIGURED" => AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "OpenRouter API key not stored".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: format!("OpenRouter credential unreadable ({code})"),
            },
            Ok(_) => match self.fetch() {
                Ok(_) => AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "OpenRouter key verified".to_string(),
                },
                Err(code) => AdapterHealth {
                    status: AdapterHealthStatus::Unhealthy,
                    message: format!("OpenRouter request failed ({code})"),
                },
            },
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::Credential, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        let configured = self.api_key().is_ok();
        Ok(DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "OpenRouter".to_string(),
            enabled: true,
            detected: configured,
            detection_method: "credential".to_string(),
            source_type: "remote_api".to_string(),
            source_exists: configured,
            permission_state: if configured {
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

    #[test]
    fn descriptor_declares_a_credential_backed_native_quota_channel() {
        let adapter = OpenRouterAdapter::with_endpoint("https://example.invalid/key");
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.id, "openrouter_api");
        assert_eq!(descriptor.quota_support, ChannelSupport::Native);
        assert_eq!(
            descriptor.usage_support,
            ChannelSupport::Unsupported,
            "OpenRouter does not expose a per-request history"
        );
        assert!(descriptor.needs_credentials());
    }

    #[test]
    fn capped_account_produces_a_real_remaining_bar() {
        let payload = serde_json::json!({
            "data": {
                "label": "key",
                "usage": 2.5,
                "limit": 10.0,
                "is_free_tier": false,
                "rate_limit": { "requests": 200, "interval": "10s" }
            }
        });
        let windows = OpenRouterAdapter::windows_from_payload(&payload).expect("windows");
        assert_eq!(windows.len(), 2);

        let credits = &windows[0];
        assert_eq!(credits.kind, QuotaKind::Credits);
        assert_eq!(credits.used, 2_500_000);
        assert_eq!(credits.limit, Some(10_000_000));
        assert_eq!(credits.remaining, Some(7_500_000));
        assert_eq!(credits.remaining_percent, Some(75.0));
        credits.check_invariants().expect("consistent");

        let rate = &windows[1];
        assert_eq!(rate.kind, QuotaKind::Requests);
        assert_eq!(rate.limit, Some(200));
        assert_eq!(rate.label, "Requests per 10s");
    }

    #[test]
    fn uncapped_account_reports_usage_without_a_percentage() {
        let payload = serde_json::json!({
            "data": { "usage": 1.25, "limit": null }
        });
        let windows = OpenRouterAdapter::windows_from_payload(&payload).expect("windows");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used, 1_250_000);
        assert_eq!(windows[0].limit, None);
        assert_eq!(
            windows[0].remaining_percent, None,
            "an uncapped account has no remaining percentage"
        );
    }

    #[test]
    fn zero_limit_is_not_treated_as_a_usable_limit() {
        let payload = serde_json::json!({ "data": { "usage": 0.0, "limit": 0 } });
        let windows = OpenRouterAdapter::windows_from_payload(&payload).expect("windows");
        assert_eq!(windows[0].limit, None);
        assert_eq!(windows[0].remaining_percent, None);
    }

    #[test]
    fn unexpected_payloads_are_rejected_instead_of_guessed() {
        for payload in [
            serde_json::json!({}),
            serde_json::json!({ "data": {} }),
            serde_json::json!({ "data": { "usage": "lots" } }),
            serde_json::json!({ "data": { "usage": -1.0 } }),
            serde_json::json!({ "data": { "usage": 1.0, "limit": "ten" } }),
        ] {
            assert_eq!(
                OpenRouterAdapter::windows_from_payload(&payload),
                Err("SOURCE_SCHEMA_MISMATCH".to_string()),
                "payload must be rejected: {payload}"
            );
        }
    }

    #[test]
    fn without_a_stored_key_no_request_is_attempted() {
        // The endpoint is unroutable on purpose: reaching it would mean the
        // adapter tried a request before a credential existed.
        let adapter = OpenRouterAdapter::with_endpoint("https://127.0.0.1:1/key");
        let _ = CredentialStore::delete(PROVIDER_ID);
        assert_eq!(
            adapter.collect_quota(),
            Err("NOT_CONFIGURED".to_string()),
            "a missing key must be reported without any network access"
        );
        assert_eq!(
            adapter.health_check().status,
            AdapterHealthStatus::NotConfigured
        );
        let detection = adapter.detect().expect("detect");
        assert!(!detection.detected);
        assert_eq!(detection.detection_error_code, "NOT_CONFIGURED");
    }

    #[test]
    fn unsupported_usage_channel_is_never_a_successful_collection() {
        let adapter = OpenRouterAdapter::with_endpoint("https://example.invalid/key");
        let result = adapter.collect_usage_with_cursor(None);
        assert!(result.batch.is_none());
        assert_eq!(result.outcome.error_code, "NOT_SUPPORTED");
    }
}
