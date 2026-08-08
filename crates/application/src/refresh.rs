use lnwdeck_provider_runtime::{CollectionOutcome, ProviderAdapter, QuotaCollectionOutcome};
use lnwdeck_security::PrivacyGuard;
use lnwdeck_storage::repositories::{
    CollectorRunRow, DiagnosticsRepository, ProviderStateRow, QuotaRepository,
    SyncCursorRepository, UsageRepository,
};
use rusqlite::Connection;
use serde::Serialize;

/// Result of a full refresh cycle. Usage and quota are independent channels:
/// each adapter contributes one usage outcome and one quota outcome, and a
/// failure in one channel never erases data from the other.
#[derive(Debug, Clone, Serialize)]
pub struct RefreshCycleOutcome {
    pub usage: Vec<CollectionOutcome>,
    pub quota: Vec<QuotaCollectionOutcome>,
}

/// Orchestrates detection, usage collection, and quota collection for every
/// registered adapter, persists sanitized evidence, and ingests normalized
/// batches and quota reports.
pub struct RefreshAll;

impl RefreshAll {
    /// Runs a full refresh cycle. Each adapter contributes exactly one usage
    /// outcome and one quota outcome; failures are isolated and recorded,
    /// never fatal.
    pub fn execute(conn: &Connection, adapters: &[&dyn ProviderAdapter]) -> RefreshCycleOutcome {
        Self::execute_with_progress(conn, adapters, |_, _, _| true)
    }

    /// Runs a full refresh and calls `on_provider` after each adapter finishes.
    /// Returning `false` stops before the next adapter, which lets the desktop
    /// cancel a background refresh between provider jobs without discarding
    /// data already persisted by completed providers.
    pub fn execute_with_progress<F>(
        conn: &Connection,
        adapters: &[&dyn ProviderAdapter],
        mut on_provider: F,
    ) -> RefreshCycleOutcome
    where
        F: FnMut(&str, usize, usize) -> bool,
    {
        let mut cycle = RefreshCycleOutcome {
            usage: Vec::new(),
            quota: Vec::new(),
        };
        let total = adapters.len();
        for (index, adapter) in adapters.iter().enumerate() {
            let single = Self::refresh_provider(conn, *adapter);
            cycle.usage.extend(single.usage);
            cycle.quota.extend(single.quota);
            if !on_provider(adapter.id(), index + 1, total) {
                break;
            }
        }
        cycle
    }

    /// Refreshes exactly one adapter: detection, usage, and quota channels.
    pub fn refresh_provider(
        conn: &Connection,
        adapter: &dyn ProviderAdapter,
    ) -> RefreshCycleOutcome {
        RefreshCycleOutcome {
            usage: vec![Self::refresh_adapter(conn, adapter)],
            quota: vec![Self::refresh_quota(conn, adapter)],
        }
    }

    /// Runs the quota channel for one adapter: collects the normalized
    /// report, validates it, persists it, and records a quota run.
    fn refresh_quota(conn: &Connection, adapter: &dyn ProviderAdapter) -> QuotaCollectionOutcome {
        let result = adapter.collect_quota_report();

        if let Some(report) = result.report {
            if PrivacyGuard::validate_quota_report(&report).is_err() {
                let outcome = QuotaCollectionOutcome::failure(
                    adapter.id(),
                    chrono::Utc::now(),
                    "PRIVACY_VIOLATION",
                );
                let _ = Self::record_quota_run(conn, &outcome);
                return outcome;
            }
            match QuotaRepository::new(conn).upsert_report(&report) {
                Ok(_) => {}
                Err(_) => {
                    let outcome = QuotaCollectionOutcome::failure(
                        adapter.id(),
                        chrono::Utc::now(),
                        "STORAGE_FAILURE",
                    );
                    let _ = Self::record_quota_run(conn, &outcome);
                    return outcome;
                }
            }
        }

        let _ = Self::record_quota_run(conn, &result.outcome);
        result.outcome
    }

    fn refresh_adapter(conn: &Connection, adapter: &dyn ProviderAdapter) -> CollectionOutcome {
        let diag = DiagnosticsRepository::new(conn);
        let cursor_repo = SyncCursorRepository::new(conn);

        match adapter.detect() {
            Ok(detection) => {
                if diag
                    .upsert_provider_state(&ProviderStateRow {
                        provider_id: detection.provider_id.clone(),
                        display_name: detection.display_name.clone(),
                        enabled: detection.enabled,
                        detected: detection.detected,
                        detection_method: detection.detection_method.clone(),
                        source_type: detection.source_type.clone(),
                        source_exists: detection.source_exists,
                        permission_state: detection.permission_state.clone(),
                        adapter_version: detection.adapter_version.clone(),
                        last_detection_at: detection.last_detection_at.clone(),
                        detection_error_code: detection.detection_error_code.clone(),
                    })
                    .is_err()
                {
                    return Self::storage_failure_outcome(adapter.id());
                }
            }
            Err(code) => {
                if diag
                    .upsert_provider_state(&ProviderStateRow {
                        provider_id: adapter.id().to_string(),
                        display_name: adapter.name().to_string(),
                        enabled: true,
                        detected: false,
                        detection_method: "unsupported".to_string(),
                        source_type: String::new(),
                        source_exists: false,
                        permission_state: "n/a".to_string(),
                        adapter_version: "0.2.0".to_string(),
                        last_detection_at: None,
                        detection_error_code: code,
                    })
                    .is_err()
                {
                    return Self::storage_failure_outcome(adapter.id());
                }
            }
        }

        let cursor = match cursor_repo.get_cursor(adapter.id()) {
            Ok(cursor) => cursor,
            Err(_) => {
                let outcome = Self::storage_failure_outcome(adapter.id());
                let _ = Self::record_run(conn, &outcome);
                return outcome;
            }
        };

        let full_scan = matches!(adapter.id(), "openai_codex" | "opencode");
        let mut result =
            adapter.collect_usage_with_cursor(if full_scan { None } else { cursor.as_deref() });
        let mut outcome = result.outcome.clone();

        if let Some(batch) = result.batch.take() {
            match PrivacyGuard::validate_usage_batch(&batch) {
                Err(_) => {
                    outcome.events_rejected = batch.events.len() as u64;
                    outcome.error_code = "PRIVACY_VIOLATION".to_string();
                }
                Ok(()) => {
                    let repo = UsageRepository::new(conn);
                    let ingestion = if full_scan {
                        let (current_source, legacy_source) = if adapter.id() == "openai_codex" {
                            ("local_jsonl_v2", "local_jsonl")
                        } else {
                            ("opencode_db", "opencode_db")
                        };
                        repo.replace_provider_batch(
                            &batch,
                            adapter.id(),
                            current_source,
                            legacy_source,
                        )
                    } else {
                        repo.ingest_batch_with_counts(&batch)
                    };
                    match ingestion {
                        Ok((inserted, duplicates)) => {
                            outcome.events_inserted = inserted;
                            outcome.duplicates_skipped = duplicates;
                        }
                        Err(_) => {
                            outcome.error_code = "STORAGE_FAILURE".to_string();
                        }
                    }
                }
            }
        }

        if let Some(next_cursor) = result.next_cursor {
            let _ = cursor_repo.upsert_cursor(adapter.id(), &next_cursor);
        }

        let _ = Self::record_run(conn, &outcome);
        outcome
    }

    fn storage_failure_outcome(provider_id: &str) -> CollectionOutcome {
        let started_at = chrono::Utc::now();
        CollectionOutcome::failure(provider_id, "passive_scan", started_at, "STORAGE_FAILURE")
    }

    fn record_run(conn: &Connection, outcome: &CollectionOutcome) -> Result<(), rusqlite::Error> {
        let diag = DiagnosticsRepository::new(conn);
        diag.insert_collector_run(&CollectorRunRow {
            id: 0,
            provider_id: outcome.provider_id.clone(),
            collector_mode: outcome.collector_mode.clone(),
            started_at: outcome.started_at.clone(),
            finished_at: outcome.finished_at.clone(),
            duration_ms: outcome.duration_ms,
            source_records_seen: outcome.source_records_seen,
            records_parsed: outcome.records_parsed,
            events_normalized: outcome.events_normalized,
            events_rejected: outcome.events_rejected,
            duplicates_skipped: outcome.duplicates_skipped,
            events_inserted: outcome.events_inserted,
            quota_snapshots_inserted: outcome.quota_snapshots_inserted,
            warning_codes: outcome.warning_codes.clone(),
            error_code: outcome.error_code.clone(),
            next_retry_at: outcome.next_retry_at.clone(),
        })
    }

    fn record_quota_run(
        conn: &Connection,
        outcome: &QuotaCollectionOutcome,
    ) -> Result<(), rusqlite::Error> {
        let diag = DiagnosticsRepository::new(conn);
        diag.insert_collector_run(&CollectorRunRow {
            id: 0,
            provider_id: outcome.provider_id.clone(),
            collector_mode: outcome.collector_mode.clone(),
            started_at: outcome.started_at.clone(),
            finished_at: outcome.finished_at.clone(),
            duration_ms: outcome.duration_ms,
            source_records_seen: 0,
            records_parsed: 0,
            events_normalized: 0,
            events_rejected: 0,
            duplicates_skipped: 0,
            events_inserted: 0,
            quota_snapshots_inserted: outcome.windows_collected,
            warning_codes: Vec::new(),
            error_code: outcome.error_code.clone(),
            next_retry_at: None,
        })
    }
}
