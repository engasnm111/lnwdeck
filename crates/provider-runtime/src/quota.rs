use chrono::{DateTime, Utc};
use lnwdeck_domain::{QuotaReport, QuotaStatus};
use serde::Serialize;

/// Evidence for a single quota collection attempt. Never contains paths,
/// file names, credentials, or any forbidden content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuotaCollectionOutcome {
    pub provider_id: String,
    pub collector_mode: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
    pub windows_collected: u64,
    pub status: QuotaStatus,
    pub error_code: String,
}

/// Quota collection result: an optional normalized report plus evidence.
/// Errors are encoded in `outcome.error_code` and mapped to a status.
#[derive(Debug, Clone)]
pub struct QuotaCollectionResult {
    pub report: Option<QuotaReport>,
    pub outcome: QuotaCollectionOutcome,
}

fn elapsed_ms(since: DateTime<Utc>) -> u64 {
    let now = Utc::now();
    (now - since).num_milliseconds().max(0) as u64
}

impl QuotaCollectionOutcome {
    fn base(provider_id: &str, started_at: DateTime<Utc>) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            collector_mode: "quota_collect".to_string(),
            started_at: started_at.to_rfc3339(),
            finished_at: Utc::now().to_rfc3339(),
            duration_ms: elapsed_ms(started_at),
            windows_collected: 0,
            status: QuotaStatus::Error,
            error_code: String::new(),
        }
    }

    /// Success outcome derived from a normalized report.
    pub fn success(provider_id: &str, started_at: DateTime<Utc>, report: &QuotaReport) -> Self {
        let mut outcome = Self::base(provider_id, started_at);
        outcome.status = report.status;
        outcome.windows_collected = report.windows.len() as u64;
        outcome.error_code = report.error_code.clone().unwrap_or_default();
        outcome
    }

    /// Failure outcome carrying a sanitized error code. The status is
    /// derived from the code (`AUTH_EXPIRED`, `RATE_LIMITED`, ...).
    pub fn failure(
        provider_id: &str,
        started_at: DateTime<Utc>,
        error_code: impl Into<String>,
    ) -> Self {
        let error_code = error_code.into();
        let mut outcome = Self::base(provider_id, started_at);
        outcome.status = QuotaStatus::from_error_code(&error_code);
        outcome.error_code = error_code;
        outcome
    }
}

impl QuotaCollectionResult {
    /// Success result for adapters that returned a normalized report.
    pub fn from_report(report: QuotaReport, started_at: DateTime<Utc>) -> Self {
        let outcome = QuotaCollectionOutcome::success(&report.provider_id, started_at, &report);
        Self {
            report: Some(report),
            outcome,
        }
    }

    /// Result for a quota channel the adapter does not implement. The
    /// descriptor decides this, so the adapter is never called.
    pub fn not_supported(provider_id: &str, started_at: DateTime<Utc>) -> Self {
        Self {
            report: None,
            outcome: QuotaCollectionOutcome::failure(
                provider_id,
                started_at,
                crate::adapter::NOT_SUPPORTED,
            ),
        }
    }

    /// Result for a supported channel whose source was not present on this
    /// attempt (the adapter returned `Ok(None)`). Distinct from
    /// `not_supported`: the integration exists, the data does not.
    pub fn source_unavailable(provider_id: &str, started_at: DateTime<Utc>) -> Self {
        Self {
            report: None,
            outcome: QuotaCollectionOutcome::failure(provider_id, started_at, "SOURCE_UNAVAILABLE"),
        }
    }

    /// Failure result for adapters whose quota collection returned an error.
    pub fn failed(
        provider_id: &str,
        started_at: DateTime<Utc>,
        error_code: impl Into<String>,
    ) -> Self {
        Self {
            report: None,
            outcome: QuotaCollectionOutcome::failure(provider_id, started_at, error_code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};

    fn sample_report() -> QuotaReport {
        let window = QuotaWindow::with_limit(
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Requests,
            40,
            std::num::NonZeroU64::new(100).expect("fixture limit is non-zero"),
            None,
            Confidence::High,
        );
        QuotaReport::new(
            "claude",
            "cli_api",
            vec![window],
            chrono::Duration::hours(1),
        )
    }

    #[test]
    fn success_outcome_counts_windows() {
        let started = Utc::now() - chrono::Duration::milliseconds(25);
        let report = sample_report();
        let result = QuotaCollectionResult::from_report(report.clone(), started);
        assert!(result.report.is_some());
        assert_eq!(result.outcome.windows_collected, 1);
        assert_eq!(result.outcome.status, QuotaStatus::Fresh);
        assert!(result.outcome.error_code.is_empty());
        assert!(
            result.outcome.duration_ms >= 20,
            "duration was {}",
            result.outcome.duration_ms
        );
    }

    #[test]
    fn not_supported_and_source_unavailable_are_distinct() {
        let not_supported = QuotaCollectionResult::not_supported("cursor_ide", Utc::now());
        assert!(not_supported.report.is_none());
        assert_eq!(not_supported.outcome.status, QuotaStatus::Unavailable);
        assert_eq!(
            not_supported.outcome.error_code,
            crate::adapter::NOT_SUPPORTED
        );
        assert_eq!(not_supported.outcome.windows_collected, 0);

        let unavailable = QuotaCollectionResult::source_unavailable("codex", Utc::now());
        assert!(unavailable.report.is_none());
        assert_eq!(unavailable.outcome.status, QuotaStatus::Unavailable);
        assert_eq!(
            unavailable.outcome.error_code, "SOURCE_UNAVAILABLE",
            "a supported channel with a missing source must not read as unimplemented"
        );
    }

    #[test]
    fn failure_maps_error_code_to_status() {
        let result = QuotaCollectionResult::failed("gemini", Utc::now(), "AUTH_EXPIRED");
        assert!(result.report.is_none());
        assert_eq!(result.outcome.status, QuotaStatus::AuthExpired);
        assert_eq!(result.outcome.error_code, "AUTH_EXPIRED");

        let rate = QuotaCollectionResult::failed("grok", Utc::now(), "RATE_LIMITED");
        assert_eq!(rate.outcome.status, QuotaStatus::RateLimited);
    }
}
