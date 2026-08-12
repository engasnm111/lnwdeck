use lnwdeck_provider_runtime::{
    CollectionOutcome, CollectionResult, DetectionResult, ProviderAdapter, QuotaCollectionOutcome,
    QuotaCollectionResult,
};
use lnwdeck_security::{IdentifierHasher, PrivacyGuard};
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
        Self::execute_with_hash_key(conn, adapters, &[])
    }

    /// Runs a full refresh using the installation-local key to isolate
    /// provider accounts. The key itself is never serialized or exposed.
    pub fn execute_with_hash_key(
        conn: &Connection,
        adapters: &[&dyn ProviderAdapter],
        hash_key: &[u8],
    ) -> RefreshCycleOutcome {
        Self::execute_with_progress_and_hash_key(conn, adapters, hash_key, |_, _, _| true)
    }

    /// Runs a full refresh and calls `on_provider` after each adapter finishes.
    /// Returning `false` stops before the next adapter, which lets the desktop
    /// cancel a background refresh between provider jobs without discarding
    /// data already persisted by completed providers.
    pub fn execute_with_progress<F>(
        conn: &Connection,
        adapters: &[&dyn ProviderAdapter],
        on_provider: F,
    ) -> RefreshCycleOutcome
    where
        F: FnMut(&str, usize, usize) -> bool,
    {
        Self::execute_with_progress_and_hash_key(conn, adapters, &[], on_provider)
    }

    /// Runs a full refresh while deriving a stable, installation-local
    /// account fingerprint for every adapter that exposes an account
    /// identity.
    pub fn execute_with_progress_and_hash_key<F>(
        conn: &Connection,
        adapters: &[&dyn ProviderAdapter],
        hash_key: &[u8],
        mut on_provider: F,
    ) -> RefreshCycleOutcome
    where
        F: FnMut(&str, usize, usize) -> bool,
    {
        // Cursor reads are cheap and must happen before collection starts.
        let cursor_repo = SyncCursorRepository::new(conn);
        let cursors: Vec<Result<Option<String>, rusqlite::Error>> = adapters
            .iter()
            .map(|adapter| cursor_repo.get_cursor(adapter.id()))
            .collect();

        let collected = collect_all(adapters, hash_key, &cursors);

        let mut cycle = RefreshCycleOutcome {
            usage: Vec::new(),
            quota: Vec::new(),
        };
        let total = adapters.len();
        for (index, (adapter, collection)) in adapters.iter().zip(collected).enumerate() {
            let single = match collection {
                Some(collection) => Self::persist_provider(conn, *adapter, collection),
                None => {
                    // Cursor read failed: same outcome the old loop produced.
                    let outcome = Self::storage_failure_outcome(adapter.id());
                    let _ = Self::record_run(conn, &outcome);
                    RefreshCycleOutcome {
                        usage: vec![outcome],
                        quota: Vec::new(),
                    }
                }
            };
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
        Self::refresh_provider_with_hash_key(conn, adapter, &[])
    }

    /// Refreshes one adapter using the installation-local account key.
    pub fn refresh_provider_with_hash_key(
        conn: &Connection,
        adapter: &dyn ProviderAdapter,
        hash_key: &[u8],
    ) -> RefreshCycleOutcome {
        let fingerprint = account_fingerprint(adapter, hash_key);
        let cursor = SyncCursorRepository::new(conn)
            .get_cursor(adapter.id())
            .ok()
            .flatten();
        let collection = collect_one(adapter, cursor.as_deref(), fingerprint);
        Self::persist_provider(conn, adapter, collection)
    }

    /// Persists one adapter's collected quota report and records the run.
    fn persist_quota(
        conn: &Connection,
        adapter: &dyn ProviderAdapter,
        collection: &ProviderCollection,
    ) -> QuotaCollectionOutcome {
        let mut result = collection.quota.clone();
        if let Some(mut report) = result.report.take() {
            report.account_fingerprint = collection.fingerprint.clone();
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

    /// Persists one adapter's collected detection, usage batch and run record.
    fn persist_usage(
        conn: &Connection,
        adapter: &dyn ProviderAdapter,
        collection: &ProviderCollection,
    ) -> CollectionOutcome {
        let diag = DiagnosticsRepository::new(conn);
        let cursor_repo = SyncCursorRepository::new(conn);

        match &collection.detection {
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
                        detection_error_code: code.clone(),
                    })
                    .is_err()
                {
                    return Self::storage_failure_outcome(adapter.id());
                }
            }
        }

        let mut result = collection.usage.clone();
        let mut outcome = result.outcome.clone();

        if let Some(mut batch) = result.batch.take() {
            normalize_usage_batch(&mut batch, collection.fingerprint.as_deref());
            match PrivacyGuard::validate_usage_batch(&batch) {
                Err(_) => {
                    outcome.events_rejected = batch.events.len() as u64;
                    outcome.error_code = "PRIVACY_VIOLATION".to_string();
                }
                Ok(()) => {
                    let repo = UsageRepository::new(conn);
                    let ingestion = if collection.full_scan {
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

    /// Persists one adapter's collected evidence and returns its outcomes.
    fn persist_provider(
        conn: &Connection,
        adapter: &dyn ProviderAdapter,
        collection: ProviderCollection,
    ) -> RefreshCycleOutcome {
        RefreshCycleOutcome {
            usage: vec![Self::persist_usage(conn, adapter, &collection)],
            quota: vec![Self::persist_quota(conn, adapter, &collection)],
        }
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

/// How many adapters collect at once. Collection is network/disk I/O bound;
/// persistence is serialized on one connection afterward.
const COLLECTION_WORKERS: usize = 4;

/// Everything collected for one adapter before any database write.
struct ProviderCollection {
    fingerprint: Option<String>,
    detection: Result<DetectionResult, String>,
    usage: CollectionResult,
    quota: QuotaCollectionResult,
    /// Full-scan providers ignore the persisted cursor (cumulative snapshots).
    full_scan: bool,
}

/// Collects one adapter's detect/usage/quota channels without touching the
/// database. This is where all the network and transcript-read time goes.
fn collect_one(
    adapter: &dyn ProviderAdapter,
    cursor: Option<&str>,
    fingerprint: Option<String>,
) -> ProviderCollection {
    let full_scan = matches!(adapter.id(), "openai_codex" | "opencode");
    let usage_cursor = if full_scan { None } else { cursor };
    ProviderCollection {
        fingerprint,
        detection: adapter.detect(),
        usage: adapter.collect_usage_with_cursor(usage_cursor),
        quota: adapter.collect_quota_report(),
        full_scan,
    }
}

/// Collects every adapter concurrently with a bounded worker pool.
/// Returns one slot per adapter in registry order; a `None` slot means the
/// cursor could not be read and the adapter is skipped (recorded as a
/// storage failure in the persist phase, matching the previous behavior).
fn collect_all(
    adapters: &[&dyn ProviderAdapter],
    hash_key: &[u8],
    cursors: &[Result<Option<String>, rusqlite::Error>],
) -> Vec<Option<ProviderCollection>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let next = AtomicUsize::new(0);
    let slots = Mutex::new((0..adapters.len()).map(|_| None).collect::<Vec<_>>());
    let workers = COLLECTION_WORKERS.min(adapters.len());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= adapters.len() {
                        break;
                    }
                    let Ok(cursor) = cursors[index].as_ref() else {
                        continue; // slot stays None; persist records the failure
                    };
                    let adapter = adapters[index];
                    let collected = collect_one(
                        adapter,
                        cursor.as_deref(),
                        account_fingerprint(adapter, hash_key),
                    );
                    slots.lock().expect("slots lock")[index] = Some(collected);
                }
            });
        }
    });
    slots.into_inner().expect("slots")
}

/// Derives the only account identifier that is allowed to cross the storage
/// boundary. A blank key is used by the legacy public refresh helpers in unit
/// tests and deliberately leaves the default account bucket unchanged.
fn account_fingerprint(adapter: &dyn ProviderAdapter, hash_key: &[u8]) -> Option<String> {
    if hash_key.is_empty() {
        return None;
    }
    let identity = adapter.account_identity()?;
    let identity = identity.trim();
    if identity.is_empty() {
        return None;
    }
    let input = format!("account|{}|{identity}", adapter.id());
    Some(IdentifierHasher::new(hash_key).hash(input.as_bytes()))
}

/// Namespaces event ids by account before ingestion. Several provider
/// runtimes use the same local event id shape, so without this step a second
/// account could be incorrectly treated as a duplicate of the first one.
fn normalize_usage_batch(batch: &mut lnwdeck_domain::UsageBatch, fingerprint: Option<&str>) {
    let Some(fingerprint) = fingerprint else {
        return;
    };
    let event_hasher = IdentifierHasher::new(fingerprint.as_bytes());
    for event in &mut batch.events {
        let original_id = std::mem::take(&mut event.id);
        let input = format!("event|{}|{original_id}", event.provider_id);
        event.id = event_hasher.hash(input.as_bytes());
        event.account_fingerprint = Some(fingerprint.to_string());
    }
}
