use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

use crate::Confidence;

/// Default freshness window for a quota report: one hour.
pub const DEFAULT_FRESHNESS: Duration = Duration::hours(1);

/// How the quota window period is defined by the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuotaWindowScope {
    Rolling,
    Daily,
    Weekly,
    Monthly,
    Session,
    Other,
}

/// What a quota window measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuotaKind {
    Requests,
    Tokens,
    Credits,
    Parallel,
}

/// Overall status of a quota report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    Fresh,
    Stale,
    Unavailable,
    AuthExpired,
    RateLimited,
    Error,
}

impl QuotaStatus {
    /// Statuses that still describe the provider's remaining quota.
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Fresh | Self::Stale)
    }

    /// Statuses that indicate the report carries an error, not quota data.
    pub fn is_error(self) -> bool {
        matches!(
            self,
            Self::Unavailable | Self::AuthExpired | Self::RateLimited | Self::Error
        )
    }

    /// Maps a sanitized collector error code to a specific status.
    pub fn from_error_code(code: &str) -> Self {
        match code {
            "AUTH_EXPIRED" | "AUTH_FAILED" | "TOKEN_EXPIRED" => Self::AuthExpired,
            "RATE_LIMITED" | "RATE_LIMIT" => Self::RateLimited,
            "SOURCE_UNAVAILABLE" | "NOT_INSTALLED" | "UNSUPPORTED" | "NOT_SUPPORTED"
            | "NOT_CONFIGURED" => Self::Unavailable,
            _ => Self::Error,
        }
    }
}

/// One quota window.
///
/// `limit`, `remaining`, `used_percent` and `remaining_percent` are `None`
/// whenever the provider does not report a real limit. They are never
/// defaulted to zero or to one hundred percent, so a consumer cannot render
/// a progress bar for a window whose limit is unknown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuotaWindow {
    pub window_key: String,
    pub label: String,
    pub scope: QuotaWindowScope,
    pub kind: QuotaKind,
    pub used: u64,
    pub limit: Option<u64>,
    pub remaining: Option<u64>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub reset_at: Option<DateTime<Utc>>,
    pub is_unlimited: bool,
    pub confidence: Confidence,
}

impl QuotaWindow {
    /// Window with a real, provider-reported limit. Remaining and both
    /// percentages are derived from `used` and `limit`.
    ///
    /// `limit` is a `NonZeroU64` on purpose: a caller that only has a
    /// possibly-unknown limit must decide explicitly between this
    /// constructor and [`QuotaWindow::usage_only`], instead of passing zero
    /// and silently getting a fabricated full bar.
    #[allow(clippy::too_many_arguments)]
    pub fn with_limit(
        window_key: impl Into<String>,
        label: impl Into<String>,
        scope: QuotaWindowScope,
        kind: QuotaKind,
        used: u64,
        limit: NonZeroU64,
        reset_at: Option<DateTime<Utc>>,
        confidence: Confidence,
    ) -> Self {
        let limit_value = limit.get();
        let used_percent = (used as f64 / limit_value as f64 * 100.0).clamp(0.0, 100.0);
        Self {
            window_key: window_key.into(),
            label: label.into(),
            scope,
            kind,
            used,
            limit: Some(limit_value),
            remaining: Some(limit_value.saturating_sub(used)),
            used_percent: Some(used_percent),
            remaining_percent: Some(100.0 - used_percent),
            reset_at,
            is_unlimited: false,
            confidence,
        }
    }

    /// Window that records real usage over a period while the provider's
    /// limit is unknown. Consumers must render the used amount only; there
    /// is no remaining value and no percentage to show.
    pub fn usage_only(
        window_key: impl Into<String>,
        label: impl Into<String>,
        scope: QuotaWindowScope,
        kind: QuotaKind,
        used: u64,
        reset_at: Option<DateTime<Utc>>,
        confidence: Confidence,
    ) -> Self {
        Self {
            window_key: window_key.into(),
            label: label.into(),
            scope,
            kind,
            used,
            limit: None,
            remaining: None,
            used_percent: None,
            remaining_percent: None,
            reset_at,
            is_unlimited: false,
            confidence,
        }
    }

    /// Window for local/unlimited providers: no limit, no remaining bar.
    pub fn unlimited(scope: QuotaWindowScope, kind: QuotaKind) -> Self {
        Self {
            window_key: "unlimited".to_string(),
            label: "Unlimited".to_string(),
            scope,
            kind,
            used: 0,
            limit: None,
            remaining: None,
            used_percent: None,
            remaining_percent: None,
            reset_at: None,
            is_unlimited: true,
            confidence: Confidence::High,
        }
    }

    /// True when the provider reported a real limit, i.e. when a remaining
    /// bar and percentage may be rendered.
    pub fn limit_known(&self) -> bool {
        self.limit.is_some()
    }

    /// Verifies the window's internal consistency. Returns the offending
    /// field name when an invariant is broken, so contract tests and the
    /// privacy guard can reject fabricated data instead of storing it.
    pub fn check_invariants(&self) -> Result<(), String> {
        match (
            self.limit,
            self.remaining,
            self.used_percent,
            self.remaining_percent,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(limit), Some(remaining), Some(used_percent), Some(remaining_percent)) => {
                if self.is_unlimited {
                    return Err("is_unlimited window must not carry a limit".to_string());
                }
                if limit == 0 {
                    return Err("limit must be non-zero when present".to_string());
                }
                if remaining != limit.saturating_sub(self.used) {
                    return Err("remaining does not match limit - used".to_string());
                }
                if !(0.0..=100.0).contains(&used_percent) {
                    return Err("used_percent out of range".to_string());
                }
                if (used_percent + remaining_percent - 100.0).abs() > f64::EPSILON * 100.0 {
                    return Err("percentages do not sum to 100".to_string());
                }
                Ok(())
            }
            _ => Err("limit, remaining and percentages must all be set or all absent".to_string()),
        }
    }
}

/// Normalized quota report for one provider, produced by a quota collector.
///
/// Carries report-level metadata (status, plan, freshness) plus one window
/// per reset period. Never contains raw credentials, paths, or account ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuotaReport {
    pub provider_id: String,
    pub account_fingerprint: Option<String>,
    pub plan: Option<String>,
    pub status: QuotaStatus,
    pub source: String,
    pub collected_at: DateTime<Utc>,
    pub stale_at: DateTime<Utc>,
    pub error_code: Option<String>,
    pub windows: Vec<QuotaWindow>,
}

impl QuotaReport {
    /// Fresh report built from the given windows. `stale_at` is derived from
    /// `collected_at` plus `freshness`.
    pub fn new(
        provider_id: impl Into<String>,
        source: impl Into<String>,
        windows: Vec<QuotaWindow>,
        freshness: Duration,
    ) -> Self {
        let collected_at = Utc::now();
        Self {
            provider_id: provider_id.into(),
            account_fingerprint: None,
            plan: None,
            status: QuotaStatus::Fresh,
            source: source.into(),
            collected_at,
            stale_at: collected_at + freshness,
            error_code: None,
            windows,
        }
    }

    /// Error report: the quota channel failed with a sanitized error code.
    /// Known codes map to specific statuses (`AUTH_EXPIRED`,
    /// `RATE_LIMITED`, `SOURCE_UNAVAILABLE`, `UNSUPPORTED`); anything else
    /// becomes `Error`.
    pub fn failed(
        provider_id: impl Into<String>,
        source: impl Into<String>,
        error_code: impl Into<String>,
    ) -> Self {
        let error_code = error_code.into();
        let status = QuotaStatus::from_error_code(&error_code);
        let collected_at = Utc::now();
        Self {
            provider_id: provider_id.into(),
            account_fingerprint: None,
            plan: None,
            status,
            source: source.into(),
            collected_at,
            stale_at: collected_at + DEFAULT_FRESHNESS,
            error_code: Some(error_code),
            windows: Vec::new(),
        }
    }

    /// True when the report carries real quota data that the UI may render.
    pub fn is_usable(&self) -> bool {
        self.status.is_usable() && !self.windows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(timestamp: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(timestamp, 0).expect("valid timestamp")
    }

    fn nz(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).expect("non-zero limit")
    }

    #[test]
    fn window_derives_remaining_and_percentages() {
        let window = QuotaWindow::with_limit(
            "7d",
            "7-day",
            QuotaWindowScope::Weekly,
            QuotaKind::Tokens,
            3000,
            nz(5000),
            Some(dt(1_800_000_000)),
            Confidence::High,
        );
        assert_eq!(window.remaining, Some(2000));
        assert_eq!(window.used_percent, Some(60.0));
        assert_eq!(window.remaining_percent, Some(40.0));
        assert!(!window.is_unlimited);
        assert!(window.limit_known());
        window.check_invariants().expect("consistent window");
    }

    #[test]
    fn window_clamps_used_percent_at_100() {
        let window = QuotaWindow::with_limit(
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Requests,
            120,
            nz(100),
            None,
            Confidence::Medium,
        );
        assert_eq!(window.remaining, Some(0));
        assert_eq!(window.used_percent, Some(100.0));
        assert_eq!(window.remaining_percent, Some(0.0));
        window.check_invariants().expect("consistent window");
    }

    #[test]
    fn usage_only_window_reports_no_limit_and_no_percentages() {
        let window = QuotaWindow::usage_only(
            "monthly",
            "Monthly",
            QuotaWindowScope::Monthly,
            QuotaKind::Credits,
            50,
            None,
            Confidence::Low,
        );
        assert_eq!(window.used, 50);
        assert_eq!(window.limit, None, "unknown limit must stay unknown");
        assert_eq!(window.remaining, None);
        assert_eq!(
            window.used_percent, None,
            "a percentage without a limit would be fabricated"
        );
        assert_eq!(
            window.remaining_percent, None,
            "remaining percent must never default to 100"
        );
        assert!(!window.limit_known());
        window.check_invariants().expect("consistent window");
    }

    #[test]
    fn unlimited_window_has_no_limit_and_no_bar() {
        let window = QuotaWindow::unlimited(QuotaWindowScope::Other, QuotaKind::Requests);
        assert!(window.is_unlimited);
        assert_eq!(window.limit, None);
        assert_eq!(window.used, 0);
        assert_eq!(window.used_percent, None);
        assert_eq!(window.remaining_percent, None);
        window.check_invariants().expect("consistent window");
    }

    #[test]
    fn check_invariants_rejects_fabricated_windows() {
        let mut half_set = QuotaWindow::usage_only(
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Tokens,
            10,
            None,
            Confidence::Low,
        );
        half_set.remaining_percent = Some(100.0);
        assert!(
            half_set.check_invariants().is_err(),
            "a percentage without a limit must be rejected"
        );

        let mut inconsistent = QuotaWindow::with_limit(
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Tokens,
            10,
            nz(100),
            None,
            Confidence::High,
        );
        inconsistent.remaining = Some(999);
        assert!(
            inconsistent.check_invariants().is_err(),
            "remaining must match limit - used"
        );

        let mut unlimited_with_limit =
            QuotaWindow::unlimited(QuotaWindowScope::Other, QuotaKind::Requests);
        unlimited_with_limit.limit = Some(10);
        unlimited_with_limit.remaining = Some(10);
        unlimited_with_limit.used_percent = Some(0.0);
        unlimited_with_limit.remaining_percent = Some(100.0);
        assert!(
            unlimited_with_limit.check_invariants().is_err(),
            "an unlimited window must not carry a limit"
        );
    }

    #[test]
    fn report_derives_stale_at_from_freshness() {
        let report = QuotaReport::new(
            "opencode",
            "cli_api",
            vec![QuotaWindow::unlimited(
                QuotaWindowScope::Other,
                QuotaKind::Requests,
            )],
            Duration::minutes(30),
        );
        assert_eq!(report.status, QuotaStatus::Fresh);
        assert!(report.stale_at > report.collected_at);
        assert!(report.is_usable());
    }

    #[test]
    fn failed_report_has_error_status_and_no_windows() {
        let report = QuotaReport::failed("codex", "cli_api", "AUTH_EXPIRED");
        assert_eq!(report.status, QuotaStatus::AuthExpired);
        assert_eq!(report.error_code.as_deref(), Some("AUTH_EXPIRED"));
        assert!(report.windows.is_empty());
        assert!(!report.is_usable());
    }

    #[test]
    fn status_classification() {
        assert!(QuotaStatus::Fresh.is_usable());
        assert!(QuotaStatus::Stale.is_usable());
        assert!(!QuotaStatus::Error.is_usable());
        assert!(QuotaStatus::AuthExpired.is_error());
        assert!(QuotaStatus::RateLimited.is_error());
        assert!(!QuotaStatus::Fresh.is_error());
    }
}
