use chrono::{DateTime, Utc};
use lnwdeck_domain::UsageBatch;
use serde::Serialize;

/// Collector evidence for a single refresh attempt. Never contains paths,
/// file names, credentials, or any forbidden content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionOutcome {
    pub provider_id: String,
    pub collector_mode: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
    pub source_records_seen: u64,
    pub records_parsed: u64,
    pub events_normalized: u64,
    pub events_rejected: u64,
    pub duplicates_skipped: u64,
    pub events_inserted: u64,
    pub quota_snapshots_inserted: u64,
    pub warning_codes: Vec<String>,
    pub error_code: String,
    pub next_retry_at: Option<String>,
}

/// Adapter collection result: an optional normalized batch plus evidence.
/// Errors are encoded in `outcome.error_code` so every attempt produces a
/// recordable, serializable outcome.
#[derive(Debug, Clone)]
pub struct CollectionResult {
    pub batch: Option<UsageBatch>,
    pub outcome: CollectionOutcome,
    pub next_cursor: Option<String>,
}

fn elapsed_ms(since: DateTime<Utc>) -> u64 {
    let now = Utc::now();
    (now - since).num_milliseconds().max(0) as u64
}

impl CollectionOutcome {
    fn base(provider_id: &str, collector_mode: &str, started_at: DateTime<Utc>) -> Self {
        let started = started_at.to_rfc3339();
        let finished_at = Utc::now();
        Self {
            provider_id: provider_id.to_string(),
            collector_mode: collector_mode.to_string(),
            duration_ms: elapsed_ms(started_at),
            started_at: started,
            finished_at: finished_at.to_rfc3339(),
            source_records_seen: 0,
            records_parsed: 0,
            events_normalized: 0,
            events_rejected: 0,
            duplicates_skipped: 0,
            events_inserted: 0,
            quota_snapshots_inserted: 0,
            warning_codes: Vec::new(),
            error_code: String::new(),
            next_retry_at: None,
        }
    }

    /// Success outcome for adapters that only implement the basic
    /// `collect_usage` contract without incremental cursor support.
    pub fn success(
        provider_id: &str,
        collector_mode: &str,
        started_at: DateTime<Utc>,
        events_normalized: u64,
    ) -> Self {
        let mut outcome = Self::base(provider_id, collector_mode, started_at);
        outcome.events_normalized = events_normalized;
        outcome
    }

    /// Failure outcome carrying a sanitized error code.
    pub fn failure(
        provider_id: &str,
        collector_mode: &str,
        started_at: DateTime<Utc>,
        error_code: impl Into<String>,
    ) -> Self {
        let mut outcome = Self::base(provider_id, collector_mode, started_at);
        outcome.error_code = error_code.into();
        outcome
    }

    /// Records a collection warning code (e.g. `ROW_SKIPPED`).
    pub fn with_warning(mut self, code: impl Into<String>) -> Self {
        self.warning_codes.push(code.into());
        self
    }
}

impl CollectionResult {
    /// Result for adapters that only implement the basic `collect_usage`
    /// contract. Successful batches are wrapped; failures are encoded in the
    /// outcome. The cursor is preserved unchanged.
    pub fn from_basic(
        provider_id: &str,
        collector_mode: &str,
        started_at: DateTime<Utc>,
        result: Result<UsageBatch, String>,
        cursor: Option<&str>,
    ) -> Self {
        match result {
            Ok(batch) => {
                let events_normalized = batch.events.len() as u64;
                CollectionResult {
                    batch: Some(batch),
                    outcome: CollectionOutcome::success(
                        provider_id,
                        collector_mode,
                        started_at,
                        events_normalized,
                    ),
                    next_cursor: cursor.map(str::to_string),
                }
            }
            Err(code) => CollectionResult {
                batch: None,
                outcome: CollectionOutcome::failure(provider_id, collector_mode, started_at, code),
                next_cursor: cursor.map(str::to_string),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_outcome_records_normalized_count() {
        let started = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let outcome = CollectionOutcome::success("p", "scan", started, 3);
        assert_eq!(outcome.events_normalized, 3);
        assert!(outcome.error_code.is_empty());
        assert!(outcome.started_at.starts_with("2023-11"));
    }

    #[test]
    fn failure_outcome_encodes_sanitized_error_code() {
        let started = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let outcome = CollectionOutcome::failure("p", "scan", started, "SOURCE_UNAVAILABLE");
        assert_eq!(outcome.error_code, "SOURCE_UNAVAILABLE");
        assert_eq!(outcome.events_normalized, 0);
    }

    #[test]
    fn from_basic_maps_success_and_failure() {
        let started = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let ok = CollectionResult::from_basic(
            "p",
            "scan",
            started,
            Ok(UsageBatch {
                batch_id: "b".to_string(),
                events: vec![],
            }),
            Some("c1"),
        );
        assert!(ok.batch.is_some());
        assert_eq!(ok.next_cursor.as_deref(), Some("c1"));

        let err = CollectionResult::from_basic(
            "p",
            "scan",
            started,
            Err("LOCKED".to_string()),
            Some("c1"),
        );
        assert!(err.batch.is_none());
        assert_eq!(err.outcome.error_code, "LOCKED");
    }

    #[test]
    fn duration_is_measured_from_started_at() {
        let started = Utc::now() - chrono::Duration::milliseconds(25);
        let outcome = CollectionOutcome::success("p", "scan", started, 0);
        assert!(
            outcome.duration_ms >= 20,
            "duration was {}",
            outcome.duration_ms
        );
    }
}
