//! Grok (xAI) quota collector.
//!
//! xAI does not publish a subscription balance endpoint. What it does expose is
//! `GET /v1/api-key`, which validates the key, and rate-limit headers on the
//! same response. This adapter therefore reports exactly what the provider
//! gives:
//!
//! - a requests window with a real limit and remaining count when the
//!   rate-limit headers are present;
//! - `QUOTA_NOT_PUBLISHED` when the key is valid but xAI returned no limit
//!   information, so the UI states that no quota is available instead of
//!   showing an invented bar;
//! - `NOT_CONFIGURED` when the user has not stored a key, in which case no
//!   request is made at all.

use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
};
use lnwdeck_provider_http::{get_json, HttpResponse, JsonRequest};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use lnwdeck_windows_integration::CredentialStore;
use std::num::NonZeroU64;
use std::time::Duration;

const PROVIDER_ID: &str = "xai_grok";
const ADAPTER_VERSION: &str = "0.2.0";
const DEFAULT_ENDPOINT: &str = "https://api.x.ai/v1/api-key";
/// Rate-limit headers xAI publishes on API responses.
const RATE_HEADERS: &[&str] = &[
    "x-ratelimit-limit-requests",
    "x-ratelimit-remaining-requests",
    "x-ratelimit-reset-requests",
];

pub struct GrokAdapter {
    endpoint: String,
    timeout: Duration,
}

impl Default for GrokAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokAdapter {
    pub fn new() -> Self {
        Self {
            endpoint: std::env::var("LNWDECK_XAI_ENDPOINT")
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
        self.fetch().map(Some)
    }

    fn account_identity(&self) -> Option<String> {
        self.api_key().ok()
    }

    fn health_check(&self) -> AdapterHealth {
        match self.api_key() {
            Err(code) if code == "NOT_CONFIGURED" => AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: "xAI API key not stored".to_string(),
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
        vec![Permission::Credential, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        let configured = self.api_key().is_ok();
        Ok(DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Grok".to_string(),
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
