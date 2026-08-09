//! Provider read model.
//!
//! One card per registered adapter. Everything on a card comes from real
//! state: the adapter descriptor (identity and declared capabilities), the
//! detection row written by the last refresh, the last collector run, the
//! stored quota report, and the ingested usage events. Nothing is defaulted to
//! a healthy or configured value, and the provider list is not restated here -
//! it is read from the registry, so ids can never drift apart again.

use lnwdeck_domain::{QuotaReport, QuotaStatus};
use lnwdeck_provider_runtime::{AdapterRegistry, NOT_SUPPORTED};
use lnwdeck_storage::repositories::DiagnosticsRepository;
use rusqlite::Connection;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct DetailedProviderInfo {
    pub provider_id: String,
    pub display_name: String,
    pub vendor: String,
    pub enabled: bool,
    pub detected: bool,
    pub source_type: String,
    /// Declared usage-history support: "supported", "local estimate" or
    /// "not supported".
    pub usage_support: String,
    /// Declared remaining-quota support, same vocabulary.
    pub quota_support: String,
    /// What the adapter needs: "none", "local files" or "API key".
    pub auth_requirement: String,
    pub health_status: String,
    pub event_count: i64,
    pub total_tokens: i64,
    pub last_sync: Option<String>,
    pub last_error_code: String,
    pub quota_summary: String,
    pub reset_at: Option<String>,
    pub confidence: String,
    /// Cost coverage measured from the stored events, never assumed.
    pub cost_support: String,
}

pub struct ScanProviders;

impl ScanProviders {
    /// Builds one card per registered adapter, in registry order.
    pub fn execute(
        conn: &Connection,
        registry: &AdapterRegistry,
    ) -> Result<Vec<DetailedProviderInfo>, rusqlite::Error> {
        let diag = DiagnosticsRepository::new(conn);
        let states = diag.provider_states()?;
        let runs = diag.latest_runs()?;
        let reports = lnwdeck_storage::repositories::QuotaRepository::new(conn)
            .latest_all()?
            .into_iter()
            .map(crate::quota::sanitize_legacy_opencode_report)
            .collect::<Vec<_>>();

        let mut results: Vec<DetailedProviderInfo> = Vec::new();
        for descriptor in registry.descriptors() {
            let id = descriptor.id;
            let state = states.iter().find(|s| s.provider_id == id);
            // Prefer the usage run for the freshness column; fall back to any
            // run for this provider.
            let run = runs
                .iter()
                .find(|r| r.provider_id == id && r.collector_mode != "quota_collect")
                .or_else(|| runs.iter().find(|r| r.provider_id == id));
            let quota_run = runs
                .iter()
                .find(|r| r.provider_id == id && r.collector_mode == "quota_collect");
            let (event_count, total_tokens, priced_events, last_ts) = usage_totals(conn, id)?;

            let detected = state.map(|s| s.detected).unwrap_or(false);
            let source_exists = state.map(|s| s.source_exists).unwrap_or(false);
            let enabled = state.map(|s| s.enabled).unwrap_or(true);
            let raw_last_error_code = run
                .map(|r| r.error_code.as_str())
                .filter(|code| !code.is_empty())
                .or_else(|| quota_run.map(|r| r.error_code.as_str()))
                .unwrap_or("");
            let last_error_code = if is_expected_provider_absence(raw_last_error_code) {
                String::new()
            } else {
                raw_last_error_code.to_string()
            };
            // Do not keep displaying a previously valid quota after the latest
            // scan proved that this machine has no connection for the provider.
            let report = if !detected
                && !source_exists
                && quota_run
                    .map(|r| is_expected_provider_absence(&r.error_code))
                    .unwrap_or(false)
            {
                None
            } else {
                reports
                    .iter()
                    .filter(|r| r.provider_id == id)
                    .max_by_key(|r| r.collected_at)
            };

            let health_status = health_label(
                &descriptor,
                detected,
                raw_last_error_code,
                source_exists,
                state.map(|s| s.detection_error_code.as_str()).unwrap_or(""),
            );

            let (quota_summary, reset_at, confidence) = match report {
                Some(report) => quota_card_fields(report),
                None if !descriptor.quota_support.is_supported() => {
                    ("Not supported".to_string(), None, "n/a".to_string())
                }
                None => ("No quota data".to_string(), None, "n/a".to_string()),
            };

            results.push(DetailedProviderInfo {
                provider_id: id.to_string(),
                display_name: descriptor.display_name.to_string(),
                vendor: descriptor.vendor.to_string(),
                enabled,
                detected,
                source_type: descriptor.source_kind.label().to_string(),
                usage_support: descriptor.usage_support.label().to_string(),
                quota_support: descriptor.quota_support.label().to_string(),
                auth_requirement: auth_label(descriptor.auth).to_string(),
                health_status,
                event_count,
                total_tokens,
                last_sync: last_ts.or_else(|| run.map(|r| r.finished_at.clone())),
                last_error_code,
                quota_summary,
                reset_at,
                confidence,
                cost_support: cost_label(event_count, priced_events),
            });
        }

        Ok(results)
    }
}

/// Event count, token total, number of events that carry a computed cost, and
/// the newest event timestamp for one provider.
fn usage_totals(
    conn: &Connection,
    provider_id: &str,
) -> Result<(i64, i64, i64, Option<String>), rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(tokens_input + tokens_cached + tokens_cache_write + tokens_output), 0),
                COALESCE(SUM(CASE WHEN cost IS NOT NULL AND cost <> '' THEN 1 ELSE 0 END), 0),
                MAX(timestamp)
         FROM usage_events WHERE provider_id = ?1",
        [provider_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
}

fn auth_label(auth: lnwdeck_provider_runtime::AuthKind) -> &'static str {
    match auth {
        lnwdeck_provider_runtime::AuthKind::None => "none",
        lnwdeck_provider_runtime::AuthKind::LocalFiles => "local files",
        lnwdeck_provider_runtime::AuthKind::ApiKey => "API key",
        lnwdeck_provider_runtime::AuthKind::BrowserCookie => "browser cookie",
    }
}

/// Cost coverage derived from stored events. With no events there is nothing
/// to price, and that is stated rather than guessed.
fn cost_label(event_count: i64, priced_events: i64) -> String {
    if event_count == 0 {
        return "No data".to_string();
    }
    if priced_events == 0 {
        return "Missing pricing".to_string();
    }
    if priced_events == event_count {
        "Priced".to_string()
    } else {
        format!("Partially priced ({priced_events}/{event_count})")
    }
}

/// Health label for a provider card.
///
/// An adapter that declares no channel is "Not supported"; one that needs a
/// key the user has not entered is "Not configured"; the rest reflect the last
/// real detection and collection result.
fn health_label(
    descriptor: &lnwdeck_provider_runtime::AdapterDescriptor,
    detected: bool,
    last_error_code: &str,
    source_exists: bool,
    detection_error_code: &str,
) -> String {
    if descriptor.is_inert() {
        return "Not supported".to_string();
    }
    if detection_error_code == "NOT_CONFIGURED" {
        return "Not configured".to_string();
    }
    if last_error_code == NOT_SUPPORTED {
        return "Not supported".to_string();
    }
    if !last_error_code.is_empty() && !is_expected_provider_absence(last_error_code) {
        return format!("Error ({last_error_code})");
    }
    if detected {
        return "Healthy".to_string();
    }
    if !source_exists
        && (detection_error_code.is_empty()
            || is_expected_provider_absence(detection_error_code)
            || is_expected_provider_absence(last_error_code))
    {
        "Not connected".to_string()
    } else if detection_error_code.is_empty() {
        "Source not found".to_string()
    } else {
        format!("Source not found ({detection_error_code})")
    }
}

/// Missing local integrations are expected when the same installation is
/// opened on another machine. They should remain local to the provider card
/// instead of being surfaced as a global refresh failure.
fn is_expected_provider_absence(code: &str) -> bool {
    matches!(
        code,
        "SOURCE_UNAVAILABLE" | "NOT_INSTALLED" | "NOT_CONFIGURED" | "UNSUPPORTED" | "NOT_SUPPORTED"
    )
}

/// Resolves the quota fields of a provider card from a stored report.
/// Never falls back to fake percentages: limit-less windows are shown as
/// usage estimates and error reports are shown as their status.
fn quota_card_fields(report: &QuotaReport) -> (String, Option<String>, String) {
    if !report.is_usable() {
        let status = match report.status {
            QuotaStatus::Fresh | QuotaStatus::Stale => "stale".to_string(),
            QuotaStatus::Unavailable => "unavailable".to_string(),
            QuotaStatus::AuthExpired => "auth expired".to_string(),
            QuotaStatus::RateLimited => "rate limited".to_string(),
            QuotaStatus::Error => "error".to_string(),
        };
        return (
            format!(
                "{status}{}",
                report
                    .error_code
                    .as_ref()
                    .map(|c| format!(" ({c})"))
                    .unwrap_or_default()
            ),
            None,
            "n/a".to_string(),
        );
    }

    let reset_at = report
        .windows
        .iter()
        .filter_map(|w| w.reset_at)
        .min()
        .map(|d| d.to_rfc3339());

    let confidence = {
        let mut best = lnwdeck_domain::Confidence::Low;
        for window in &report.windows {
            match window.confidence {
                lnwdeck_domain::Confidence::High => {
                    best = lnwdeck_domain::Confidence::High;
                }
                lnwdeck_domain::Confidence::Medium if best == lnwdeck_domain::Confidence::Low => {
                    best = lnwdeck_domain::Confidence::Medium;
                }
                _ => {}
            }
        }
        match best {
            lnwdeck_domain::Confidence::High => "High".to_string(),
            lnwdeck_domain::Confidence::Medium => "Medium".to_string(),
            lnwdeck_domain::Confidence::Low => "Low".to_string(),
        }
    };

    let summary = if report.windows.iter().any(|w| w.is_unlimited) {
        "Local / Unlimited".to_string()
    } else if let Some(remaining_percent) = report
        .windows
        .iter()
        .find_map(|window| window.remaining_percent)
    {
        let pct = remaining_percent.round() as u64;
        if let Some(reset) = reset_at.as_ref() {
            format!("{pct}% left · resets {reset}")
        } else {
            format!("{pct}% left")
        }
    } else if let Some(window) = report.windows.first() {
        format!(
            "used {} {} (estimate)",
            window.used,
            match window.kind {
                lnwdeck_domain::QuotaKind::Requests => "requests",
                lnwdeck_domain::QuotaKind::Tokens => "tokens",
                lnwdeck_domain::QuotaKind::Credits => "credits",
                lnwdeck_domain::QuotaKind::Parallel => "parallel",
            }
        )
    } else {
        "No quota data".to_string()
    };

    (summary, reset_at, confidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_domain::{Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope};
    use lnwdeck_provider_runtime::{
        AdapterDescriptor, AuthKind, ChannelSupport, ProviderAdapter, SourceKind,
    };
    use lnwdeck_storage::repositories::{CollectorRunRow, ProviderStateRow};
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn open_test_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let db_path = dir.path().join("test.db");
        let storage = Storage::open(&db_path).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    struct Fake(AdapterDescriptor);

    impl ProviderAdapter for Fake {
        fn descriptor(&self) -> AdapterDescriptor {
            self.0
        }
    }

    fn local_descriptor(id: &'static str, name: &'static str) -> AdapterDescriptor {
        AdapterDescriptor {
            id,
            display_name: name,
            vendor: "Vendor",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: "0.2.0",
        }
    }

    fn api_descriptor(id: &'static str, name: &'static str) -> AdapterDescriptor {
        AdapterDescriptor {
            source_kind: SourceKind::RemoteApi,
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::ApiKey,
            ..local_descriptor(id, name)
        }
    }

    fn test_registry() -> AdapterRegistry {
        let mut registry = AdapterRegistry::new();
        registry
            .register(Box::new(Fake(local_descriptor("opencode", "OpenCode"))))
            .expect("opencode");
        registry
            .register(Box::new(Fake(api_descriptor(
                "openrouter_api",
                "OpenRouter",
            ))))
            .expect("openrouter");
        registry
    }

    fn window(key: &str, used: u64, limit: u64, reset: Option<&str>) -> QuotaWindow {
        let reset_at = reset.map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .expect("rfc3339")
                .with_timezone(&chrono::Utc)
        });
        QuotaWindow::with_limit(
            key,
            key,
            QuotaWindowScope::Weekly,
            QuotaKind::Tokens,
            used,
            std::num::NonZeroU64::new(limit).expect("fixture limit is non-zero"),
            reset_at,
            Confidence::High,
        )
    }

    #[test]
    fn quota_summary_uses_remaining_percent_when_limit_known() {
        let report = QuotaReport::new(
            "opencode",
            "cli_api",
            vec![window("5h", 40, 100, Some("2026-08-05T00:00:00Z"))],
            chrono::Duration::hours(1),
        );
        let (summary, reset_at, confidence) = quota_card_fields(&report);
        assert_eq!(summary, "60% left · resets 2026-08-05T00:00:00+00:00");
        assert_eq!(reset_at.as_deref(), Some("2026-08-05T00:00:00+00:00"));
        assert_eq!(confidence, "High");
    }

    #[test]
    fn quota_summary_marks_usage_only_estimate() {
        let report = QuotaReport::new(
            "opencode",
            "local_estimate",
            vec![QuotaWindow::usage_only(
                "5h",
                "5-hour",
                QuotaWindowScope::Rolling,
                QuotaKind::Tokens,
                775,
                None,
                Confidence::High,
            )],
            chrono::Duration::hours(1),
        );
        let (summary, reset_at, confidence) = quota_card_fields(&report);
        assert_eq!(summary, "used 775 tokens (estimate)");
        assert!(reset_at.is_none());
        assert_eq!(confidence, "High");
    }

    #[test]
    fn quota_summary_reports_unlimited_for_local_providers() {
        let report = QuotaReport::new(
            "ollama_local",
            "local_api",
            vec![QuotaWindow::unlimited(
                QuotaWindowScope::Other,
                QuotaKind::Requests,
            )],
            chrono::Duration::hours(1),
        );
        let (summary, _, _) = quota_card_fields(&report);
        assert_eq!(summary, "Local / Unlimited");
    }

    #[test]
    fn quota_summary_shows_error_status_for_failed_report() {
        let report = QuotaReport::failed("codex", "cli_api", "AUTH_EXPIRED");
        let (summary, reset_at, confidence) = quota_card_fields(&report);
        assert_eq!(summary, "auth expired (AUTH_EXPIRED)");
        assert!(reset_at.is_none());
        assert_eq!(confidence, "n/a");
    }

    #[test]
    fn cards_follow_the_registry_and_never_default_to_detected() {
        let storage = open_test_db();
        let cards = ScanProviders::execute(&storage.conn, &test_registry()).expect("scan");

        assert_eq!(cards.len(), 2, "one card per registered adapter");
        assert_eq!(cards[0].provider_id, "opencode");
        assert_eq!(cards[1].provider_id, "openrouter_api");
        for card in &cards {
            assert!(
                !card.detected,
                "{} must not be reported as detected before a refresh",
                card.provider_id
            );
            assert_eq!(card.event_count, 0);
            assert_eq!(card.cost_support, "No data");
        }
        assert_eq!(cards[0].health_status, "Not connected");
        assert_eq!(cards[0].usage_support, "local estimate");
        assert_eq!(cards[0].auth_requirement, "local files");
        assert_eq!(cards[1].usage_support, "not supported");
        assert_eq!(cards[1].auth_requirement, "API key");
    }

    #[test]
    fn detection_and_run_state_drive_the_health_label() {
        let storage = open_test_db();
        let diag = DiagnosticsRepository::new(&storage.conn);
        diag.upsert_provider_state(&ProviderStateRow {
            provider_id: "opencode".to_string(),
            display_name: "OpenCode".to_string(),
            enabled: true,
            detected: true,
            detection_method: "local_sqlite".to_string(),
            source_type: "local_sqlite".to_string(),
            source_exists: true,
            permission_state: "read_ok".to_string(),
            adapter_version: "0.2.0".to_string(),
            last_detection_at: Some("2026-08-04T00:00:00+00:00".to_string()),
            detection_error_code: String::new(),
        })
        .expect("state");
        diag.upsert_provider_state(&ProviderStateRow {
            provider_id: "openrouter_api".to_string(),
            display_name: "OpenRouter".to_string(),
            enabled: true,
            detected: false,
            detection_method: "credential".to_string(),
            source_type: "remote_api".to_string(),
            source_exists: false,
            permission_state: "credential_required".to_string(),
            adapter_version: "0.2.0".to_string(),
            last_detection_at: Some("2026-08-04T00:00:00+00:00".to_string()),
            detection_error_code: "NOT_CONFIGURED".to_string(),
        })
        .expect("state");

        let cards = ScanProviders::execute(&storage.conn, &test_registry()).expect("scan");
        assert_eq!(cards[0].health_status, "Healthy");
        assert_eq!(
            cards[1].health_status, "Not configured",
            "a provider waiting for an API key is not an error"
        );
        assert_eq!(cards[1].quota_summary, "No quota data");
    }

    #[test]
    fn real_collector_errors_surface_on_the_card() {
        let storage = open_test_db();
        let diag = DiagnosticsRepository::new(&storage.conn);
        diag.insert_collector_run(&CollectorRunRow {
            id: 0,
            provider_id: "opencode".to_string(),
            collector_mode: "local_scan".to_string(),
            started_at: "2026-08-04T00:00:00+00:00".to_string(),
            finished_at: "2026-08-04T00:00:01+00:00".to_string(),
            duration_ms: 1000,
            source_records_seen: 0,
            records_parsed: 0,
            events_normalized: 0,
            events_rejected: 0,
            duplicates_skipped: 0,
            events_inserted: 0,
            quota_snapshots_inserted: 0,
            warning_codes: Vec::new(),
            error_code: "AUTH_EXPIRED".to_string(),
            next_retry_at: None,
        })
        .expect("run");

        let cards = ScanProviders::execute(&storage.conn, &test_registry()).expect("scan");
        assert_eq!(cards[0].health_status, "Error (AUTH_EXPIRED)");
        assert_eq!(cards[0].last_error_code, "AUTH_EXPIRED");
    }

    #[test]
    fn expected_provider_absence_is_not_a_refresh_error_or_stale_quota() {
        let storage = open_test_db();
        lnwdeck_storage::repositories::QuotaRepository::new(&storage.conn)
            .upsert_report(&QuotaReport::new(
                "opencode",
                "cli_api",
                vec![window("5h", 40, 100, None)],
                chrono::Duration::hours(1),
            ))
            .expect("report");
        DiagnosticsRepository::new(&storage.conn)
            .insert_collector_run(&CollectorRunRow {
                id: 0,
                provider_id: "opencode".to_string(),
                collector_mode: "quota_collect".to_string(),
                started_at: "2026-08-04T00:00:00+00:00".to_string(),
                finished_at: "2026-08-04T00:00:01+00:00".to_string(),
                duration_ms: 1000,
                source_records_seen: 0,
                records_parsed: 0,
                events_normalized: 0,
                events_rejected: 0,
                duplicates_skipped: 0,
                events_inserted: 0,
                quota_snapshots_inserted: 0,
                warning_codes: Vec::new(),
                error_code: "SOURCE_UNAVAILABLE".to_string(),
                next_retry_at: None,
            })
            .expect("run");

        let card = ScanProviders::execute(&storage.conn, &test_registry())
            .expect("scan")
            .into_iter()
            .find(|card| card.provider_id == "opencode")
            .expect("opencode card");
        assert_eq!(card.health_status, "Not connected");
        assert!(card.last_error_code.is_empty());
        assert_eq!(card.quota_summary, "No quota data");
    }

    #[test]
    fn stored_quota_report_is_joined_by_canonical_provider_id() {
        let storage = open_test_db();
        let report = QuotaReport::new(
            "opencode",
            "cli_api",
            vec![window("5h", 40, 100, None)],
            chrono::Duration::hours(1),
        );
        lnwdeck_storage::repositories::QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("upsert");

        let cards = ScanProviders::execute(&storage.conn, &test_registry()).expect("scan");
        let opencode = cards
            .iter()
            .find(|p| p.provider_id == "opencode")
            .expect("opencode card");
        assert_eq!(opencode.quota_summary, "60% left");
        assert_eq!(opencode.confidence, "High");
    }

    #[test]
    fn unsupported_quota_channel_is_labelled_not_supported() {
        let storage = open_test_db();
        let mut registry = AdapterRegistry::new();
        registry
            .register(Box::new(Fake(AdapterDescriptor {
                quota_support: ChannelSupport::Unsupported,
                ..local_descriptor("usage_only_provider", "Usage Only")
            })))
            .expect("register");

        let cards = ScanProviders::execute(&storage.conn, &registry).expect("scan");
        assert_eq!(cards[0].quota_support, "not supported");
        assert_eq!(cards[0].quota_summary, "Not supported");
    }

    #[test]
    fn cost_coverage_is_measured_from_stored_events() {
        let storage = open_test_db();
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES ('e1', 'b1', '2026-08-04T00:00:00+00:00', 'opencode', 'm', 10, 5, 'Medium', 'local_sqlite', '0.01')",
                [],
            )
            .expect("priced event");
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES ('e2', 'b1', '2026-08-04T00:01:00+00:00', 'opencode', 'm', 20, 5, 'Medium', 'local_sqlite', '')",
                [],
            )
            .expect("unpriced event");

        let cards = ScanProviders::execute(&storage.conn, &test_registry()).expect("scan");
        assert_eq!(cards[0].event_count, 2);
        assert_eq!(cards[0].total_tokens, 40);
        assert_eq!(cards[0].cost_support, "Partially priced (1/2)");
        assert_eq!(
            cards[0].last_sync.as_deref(),
            Some("2026-08-04T00:01:00+00:00")
        );
    }
}
