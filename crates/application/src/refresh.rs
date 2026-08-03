use lnwdeck_provider_runtime::{CollectionOutcome, ProviderAdapter};
use lnwdeck_security::PrivacyGuard;
use lnwdeck_storage::repositories::{
    CollectorRunRow, DiagnosticsRepository, ProviderStateRow, SyncCursorRepository, UsageRepository,
};
use rusqlite::Connection;

/// Orchestrates detection and collection for every registered adapter,
/// persists sanitized evidence, and ingests normalized batches.
pub struct RefreshAll;

impl RefreshAll {
    /// Runs a full refresh cycle. Each adapter contributes exactly one
    /// outcome; failures are isolated and recorded, never fatal.
    pub fn execute(conn: &Connection, adapters: &[&dyn ProviderAdapter]) -> Vec<CollectionOutcome> {
        let mut outcomes = Vec::new();
        for adapter in adapters {
            outcomes.push(Self::refresh_adapter(conn, *adapter));
        }
        outcomes
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
                        adapter_version: "0.1.0".to_string(),
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

        let mut result = adapter.collect_usage_with_cursor(cursor.as_deref());
        let mut outcome = result.outcome.clone();

        if let Some(batch) = result.batch.take() {
            match PrivacyGuard::validate_usage_batch(&batch) {
                Err(_) => {
                    outcome.events_rejected = batch.events.len() as u64;
                    outcome.error_code = "PRIVACY_VIOLATION".to_string();
                }
                Ok(()) => {
                    let repo = UsageRepository::new(conn);
                    match repo.ingest_batch_with_counts(&batch) {
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
}
