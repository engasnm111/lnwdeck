use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
            "SOURCE_UNAVAILABLE" | "NOT_INSTALLED" | "UNSUPPORTED" => Self::Unavailable,
            _ => Self::Error,
        }
    }
}

/// One quota window: used/limit/remaining for a single reset period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuotaWindow {
    pub window_key: String,
    pub label: String,
    pub scope: QuotaWindowScope,
    pub kind: QuotaKind,
    pub used: u64,
    pub limit: u64,
    pub remaining: u64,
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub reset_at: Option<DateTime<Utc>>,
    pub is_unlimited: bool,
    pub confidence: Confidence,
}

impl QuotaWindow {
    /// Builds a window from raw used/limit values and derives remaining and
    /// percentages. A `limit` of zero means the limit is unknown; such
    /// windows are never rendered as 0% remaining.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        window_key: impl Into<String>,
        label: impl Into<String>,
        scope: QuotaWindowScope,
        kind: QuotaKind,
        used: u64,
        limit: u64,
        reset_at: Option<DateTime<Utc>>,
        confidence: Confidence,
    ) -> Self {
        let remaining = limit.saturating_sub(used);
        let used_percent = if limit > 0 {
            (used as f64 / limit as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        Self {
            window_key: window_key.into(),
            label: label.into(),
            scope,
            kind,
            used,
            limit,
            remaining,
            used_percent,
            remaining_percent: 100.0 - used_percent,
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
            limit: 0,
            remaining: 0,
            used_percent: 0.0,
            remaining_percent: 0.0,
            reset_at: None,
            is_unlimited: true,
            confidence: Confidence::High,
        }
    }

    /// Placeholder window when no data is available at all.
    pub fn unknown(scope: QuotaWindowScope, kind: QuotaKind) -> Self {
        Self {
            window_key: "unknown".to_string(),
            label: "Unknown".to_string(),
            scope,
            kind,
            used: 0,
            limit: 0,
            remaining: 0,
            used_percent: 0.0,
            remaining_percent: 0.0,
            reset_at: None,
            is_unlimited: false,
            confidence: Confidence::Low,
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

    #[test]
    fn window_derives_remaining_and_percentages() {
        let window = QuotaWindow::new(
            "7d",
            "7-day",
            QuotaWindowScope::Weekly,
            QuotaKind::Tokens,
            3000,
            5000,
            Some(dt(1_800_000_000)),
            Confidence::High,
        );
        assert_eq!(window.remaining, 2000);
        assert_eq!(window.used_percent, 60.0);
        assert_eq!(window.remaining_percent, 40.0);
        assert!(!window.is_unlimited);
    }

    #[test]
    fn window_clamps_used_percent_at_100() {
        let window = QuotaWindow::new(
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Requests,
            120,
            100,
            None,
            Confidence::Medium,
        );
        assert_eq!(window.remaining, 0);
        assert_eq!(window.used_percent, 100.0);
        assert_eq!(window.remaining_percent, 0.0);
    }

    #[test]
    fn unknown_limit_never_renders_zero_percent() {
        let window = QuotaWindow::new(
            "monthly",
            "Monthly",
            QuotaWindowScope::Monthly,
            QuotaKind::Credits,
            50,
            0,
            None,
            Confidence::Low,
        );
        assert_eq!(window.remaining, 0);
        assert_eq!(window.used_percent, 0.0);
        assert_eq!(window.remaining_percent, 100.0);
    }

    #[test]
    fn unlimited_window_has_no_limit_and_no_bar() {
        let window = QuotaWindow::unlimited(QuotaWindowScope::Other, QuotaKind::Requests);
        assert!(window.is_unlimited);
        assert_eq!(window.limit, 0);
        assert_eq!(window.used, 0);
        assert_eq!(window.used_percent, 0.0);
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
