use crate::collection::CollectionResult;
use crate::descriptor::{AdapterDescriptor, ChannelSupport};
use crate::detection::DetectionResult;
use crate::health::{AdapterHealth, AdapterHealthStatus};
use crate::permissions::Permission;
use crate::quota::QuotaCollectionResult;
use lnwdeck_domain::{QuotaReport, UsageBatch};

/// Error code recorded when a channel the adapter does not implement is
/// requested. It is never reported as a successful collection.
pub const NOT_SUPPORTED: &str = "NOT_SUPPORTED";

/// A provider integration.
///
/// The descriptor is the contract: the runtime only calls a channel the
/// descriptor declares as supported, and the default channel implementations
/// refuse to invent data. An adapter that declares support must return real
/// data or a sanitized error code.
pub trait ProviderAdapter: Send + Sync {
    /// Identity and capabilities of this adapter.
    fn descriptor(&self) -> AdapterDescriptor;

    /// Canonical provider id, taken from the descriptor.
    fn id(&self) -> &'static str {
        self.descriptor().id
    }

    /// Display name, taken from the descriptor.
    fn name(&self) -> &'static str {
        self.descriptor().display_name
    }

    /// Collects usage events. Adapters that declare usage support must
    /// override this; the default refuses instead of returning an empty
    /// batch that would look like a successful collection.
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        Err(NOT_SUPPORTED.to_string())
    }

    /// Collects the provider quota report. `Ok(None)` means the source was
    /// not available on this attempt and is recorded as such.
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        Ok(None)
    }

    /// Health of the integration. The default is derived from the
    /// descriptor, so an adapter that collects nothing can never report
    /// itself as healthy.
    fn health_check(&self) -> AdapterHealth {
        let descriptor = self.descriptor();
        if descriptor.is_inert() {
            return AdapterHealth {
                status: AdapterHealthStatus::Unsupported,
                message: format!("{} integration is not implemented", descriptor.display_name),
            };
        }
        if descriptor.needs_credentials() {
            return AdapterHealth {
                status: AdapterHealthStatus::NotConfigured,
                message: format!("{} requires an API key", descriptor.display_name),
            };
        }
        AdapterHealth {
            status: AdapterHealthStatus::Degraded,
            message: format!(
                "{} source has not been checked yet",
                descriptor.display_name
            ),
        }
    }

    /// Permissions the adapter needs before it may read its source.
    fn required_permissions(&self) -> Vec<Permission> {
        Vec::new()
    }

    /// Runs sanitized detection. Adapters without a detectable local source
    /// report `detected = false` with method `unsupported`.
    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(DetectionResult::from_descriptor(&self.descriptor()))
    }

    /// Collects usage with an optional persisted cursor.
    ///
    /// When the descriptor declares no usage support the channel is not
    /// invoked at all and the attempt is recorded as `NOT_SUPPORTED`.
    fn collect_usage_with_cursor(&self, cursor: Option<&str>) -> CollectionResult {
        let started_at = chrono::Utc::now();
        let descriptor = self.descriptor();
        if !descriptor.usage_support.is_supported() {
            return CollectionResult::not_supported(descriptor.id, started_at, cursor);
        }
        let mode = collector_mode(descriptor.usage_support);
        CollectionResult::from_basic(
            descriptor.id,
            mode,
            started_at,
            self.collect_usage(),
            cursor,
        )
    }

    /// Collects the quota report and wraps the result with evidence.
    ///
    /// Unsupported quota channels are recorded as `NOT_SUPPORTED` without
    /// calling the adapter; `Ok(None)` from a supported channel means the
    /// source was unavailable.
    fn collect_quota_report(&self) -> QuotaCollectionResult {
        let started_at = chrono::Utc::now();
        let descriptor = self.descriptor();
        if !descriptor.quota_support.is_supported() {
            return QuotaCollectionResult::not_supported(descriptor.id, started_at);
        }
        match self.collect_quota() {
            Ok(Some(report)) => QuotaCollectionResult::from_report(report, started_at),
            Ok(None) => QuotaCollectionResult::source_unavailable(descriptor.id, started_at),
            Err(code) => QuotaCollectionResult::failed(descriptor.id, started_at, code),
        }
    }
}

/// Collector mode label recorded in diagnostics for a supported channel.
fn collector_mode(support: ChannelSupport) -> &'static str {
    match support {
        ChannelSupport::Native => "provider_api",
        ChannelSupport::LocalEstimate => "local_scan",
        ChannelSupport::Unsupported => "unsupported",
    }
}
