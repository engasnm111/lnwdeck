use chrono::{DateTime, Utc};
use lnwdeck_domain::{QuotaReport, QuotaStatus, QuotaWindow};
use lnwdeck_provider_runtime::{AdapterDescriptor, AdapterRegistry, ChannelSupport};
use lnwdeck_storage::repositories::{
    CollectorRunRow, DiagnosticsRepository, ProviderStateRow, QuotaRepository,
};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;

/// Dashboard read model for quota: one card per registered provider/account.
/// The frontend must not interpret raw provider payloads; all semantics are
/// resolved here.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuotaDashboard {
    pub generated_at: DateTime<Utc>,
    pub providers: Vec<ProviderQuotaCard>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderQuotaCard {
    pub provider_id: String,
    pub display_name: String,
    /// Present when more than one fingerprinted account exists for this
    /// provider. The raw fingerprint never crosses the native boundary.
    pub account_index: Option<u32>,
    pub connection_state: ProviderConnectionState,
    /// Descriptor capability label: `supported`, `local estimate` or
    /// `not supported`. A local estimate must never be presented as quota.
    pub quota_support: String,
    pub status: QuotaStatus,
    pub plan: Option<String>,
    pub source: String,
    pub collected_at: DateTime<Utc>,
    pub stale_at: DateTime<Utc>,
    pub error_code: Option<String>,
    pub windows: Vec<QuotaWindow>,
}

/// Secret-free connection state shared by provider pages and the widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderConnectionState {
    Connected,
    NotDetected,
    NotConfigured,
    PermissionRequired,
    AuthExpired,
    RateLimited,
    TransientError,
    Unsupported,
}

/// Hides legacy Gemini reports that were produced by the retired
/// `retrieveUserQuota` fallback. That source no longer exists: its placeholder
/// fractions (observed as a fabricated "100% remaining") and the account
/// fingerprint it was stored under would otherwise keep a dead card on the
/// dashboard. The rows stay in storage for history; only the read model hides
/// them.
pub(crate) fn sanitize_legacy_gemini_reports(
    report: QuotaReport,
    now: DateTime<Utc>,
) -> Option<QuotaReport> {
    let report = derive_read_time_status(report, now);
    if report.provider_id == "google_gemini" && report.source == "provider_api" {
        return None;
    }
    Some(report)
}

/// Converts the pre-dashboard OpenCode local estimate into an explicit
/// unavailable state at read time. Old installations may still have a
/// hard-coded local percentage stored from an earlier adapter version; it
/// must never remain visible while the provider now requires its workspace
/// dashboard credential. The historical window rows stay in storage.
pub(crate) fn sanitize_legacy_opencode_report(report: QuotaReport) -> QuotaReport {
    if report.provider_id == "opencode" && report.source == "local_estimate" {
        let account_fingerprint = report.account_fingerprint.clone();
        let mut sanitized = QuotaReport::failed("opencode", "provider_api", "NOT_CONFIGURED");
        sanitized.account_fingerprint = account_fingerprint;
        sanitized
    } else {
        report
    }
}

/// Builds the quota dashboard from the latest stored reports.
///
/// Display names and card ordering come from the adapter registry, which is
/// the single declaration of provider identity; this read model does not keep
/// its own copy of the provider list.
pub struct QueryQuotaDashboard;

impl QueryQuotaDashboard {
    pub fn execute(
        conn: &Connection,
        registry: &AdapterRegistry,
    ) -> Result<QuotaDashboard, rusqlite::Error> {
        let now = Utc::now();
        let reports = QuotaRepository::new(conn)
            .latest_all()?
            .into_iter()
            .map(sanitize_legacy_opencode_report)
            .filter_map(|report| sanitize_legacy_gemini_reports(report, now))
            .collect::<Vec<_>>();
        let mut reports_by_provider: BTreeMap<String, Vec<QuotaReport>> = BTreeMap::new();
        for report in reports {
            reports_by_provider
                .entry(report.provider_id.clone())
                .or_default()
                .push(report);
        }
        let diagnostics = DiagnosticsRepository::new(conn);
        let states = diagnostics.provider_states()?;
        let runs = diagnostics.latest_runs()?;
        let descriptors = registry.descriptors();
        let report_count = reports_by_provider.values().map(Vec::len).sum::<usize>();
        let mut cards = Vec::with_capacity(descriptors.len() + report_count);

        for descriptor in &descriptors {
            let state = states.iter().find(|s| s.provider_id == descriptor.id);
            let quota_run = runs
                .iter()
                .find(|r| r.provider_id == descriptor.id && r.collector_mode == "quota_collect");
            let provider_reports = reports_by_provider
                .remove(descriptor.id)
                .unwrap_or_default();
            if provider_reports.is_empty() {
                let connection_state = connection_state_for(descriptor, state, quota_run, None);
                cards.push(card_from_descriptor(
                    descriptor,
                    None,
                    connection_state,
                    None,
                    quota_run,
                ));
            } else {
                let account_index = (provider_reports.len() > 1).then_some(0u32);
                let run_requires_ide =
                    quota_run.is_some_and(|run| run.error_code == "SOURCE_REQUIRES_IDE");
                for (index, report) in provider_reports.iter().enumerate() {
                    // The moment the Antigravity IDE closes, the cached
                    // reading becomes old data: status is forced to stale and
                    // the error code rides along so the UI can explain what
                    // to do next. Real percentages stay visible, never fresh.
                    let report = if run_requires_ide && report.is_usable() {
                        let mut aged = report.clone();
                        aged.status = QuotaStatus::Stale;
                        aged.error_code = Some("SOURCE_REQUIRES_IDE".to_string());
                        aged
                    } else {
                        report.clone()
                    };
                    let connection_state =
                        connection_state_for(descriptor, state, quota_run, Some(&report));
                    let account_index = account_index.map(|_| index as u32 + 1);
                    cards.push(card_from_descriptor(
                        descriptor,
                        Some(&report),
                        connection_state,
                        account_index,
                        quota_run,
                    ));
                }
            }
        }

        // Unknown ids (for example a provider removed in a later build) sort
        // last instead of being dropped, so stored data stays visible.
        for (provider_id, provider_reports) in reports_by_provider {
            if registry.rank(&provider_id).is_none() {
                let account_index = (provider_reports.len() > 1).then_some(0u32);
                for (index, report) in provider_reports.into_iter().enumerate() {
                    cards.push(card_from_report(
                        report,
                        registry,
                        account_index.map(|_| index as u32 + 1),
                    ));
                }
            }
        }
        cards.sort_by_key(|card| {
            (
                registry.rank(&card.provider_id).unwrap_or(usize::MAX),
                card.provider_id.clone(),
                card.account_index.unwrap_or(0),
            )
        });
        Ok(QuotaDashboard {
            generated_at: Utc::now(),
            providers: cards,
        })
    }
}

fn connection_state_for(
    descriptor: &AdapterDescriptor,
    state: Option<&ProviderStateRow>,
    quota_run: Option<&CollectorRunRow>,
    report: Option<&QuotaReport>,
) -> ProviderConnectionState {
    let report_is_absence = report.is_none_or(|value| {
        matches!(
            value.error_code.as_deref(),
            Some(
                "SOURCE_UNAVAILABLE"
                    | "NOT_INSTALLED"
                    | "NOT_CONFIGURED"
                    | "NOT_SUPPORTED"
                    | "UNSUPPORTED"
            )
        )
    });
    let source_absent = state.is_none_or(|provider| !provider.source_exists && !provider.detected)
        && report_is_absence;
    if source_absent {
        return ProviderConnectionState::NotDetected;
    }

    if descriptor.quota_support == ChannelSupport::Unsupported {
        return ProviderConnectionState::Unsupported;
    }

    let error_code = quota_run
        .and_then(|run| (!run.error_code.is_empty()).then_some(run.error_code.as_str()))
        .or_else(|| {
            state.and_then(|provider| {
                (!provider.detection_error_code.is_empty())
                    .then_some(provider.detection_error_code.as_str())
            })
        })
        .or_else(|| report.and_then(|value| value.error_code.as_deref()));

    match error_code {
        Some("NOT_CONFIGURED") => return ProviderConnectionState::NotConfigured,
        Some("PERMISSION_DENIED") | Some("ACCESS_DENIED") => {
            return ProviderConnectionState::PermissionRequired;
        }
        Some("AUTH_EXPIRED") | Some("AUTH_FAILED") | Some("TOKEN_EXPIRED") => {
            return ProviderConnectionState::AuthExpired;
        }
        Some("RATE_LIMITED") | Some("RATE_LIMIT") => {
            return ProviderConnectionState::RateLimited;
        }
        Some("SOURCE_UNAVAILABLE") | Some("NOT_INSTALLED") => {
            if state.is_none_or(|provider| !provider.source_exists && !provider.detected) {
                return ProviderConnectionState::NotDetected;
            }
            return ProviderConnectionState::TransientError;
        }
        // The source needs the Antigravity IDE running. When a usable
        // reading is cached it stays connected so the UI can present it as
        // old data instead of hiding it; without cached data the card reads
        // as a transient error with guidance.
        Some("SOURCE_REQUIRES_IDE") if report.is_some_and(QuotaReport::is_usable) => {
            return ProviderConnectionState::Connected;
        }
        Some(code) if !code.is_empty() && code != "NOT_SUPPORTED" && code != "UNSUPPORTED" => {
            return ProviderConnectionState::TransientError;
        }
        _ => {}
    }

    if state.is_some_and(|provider| provider.detected || provider.source_exists)
        || report.is_some_and(QuotaReport::is_usable)
    {
        ProviderConnectionState::Connected
    } else if descriptor.needs_credentials() {
        ProviderConnectionState::NotConfigured
    } else {
        ProviderConnectionState::NotDetected
    }
}

fn state_error_code(state: ProviderConnectionState) -> Option<String> {
    match state {
        ProviderConnectionState::NotDetected => Some("SOURCE_UNAVAILABLE".to_string()),
        ProviderConnectionState::NotConfigured => Some("NOT_CONFIGURED".to_string()),
        ProviderConnectionState::PermissionRequired => Some("PERMISSION_DENIED".to_string()),
        ProviderConnectionState::AuthExpired => Some("AUTH_EXPIRED".to_string()),
        ProviderConnectionState::RateLimited => Some("RATE_LIMITED".to_string()),
        ProviderConnectionState::TransientError => Some("SOURCE_SCHEMA_MISMATCH".to_string()),
        ProviderConnectionState::Unsupported => Some("NOT_SUPPORTED".to_string()),
        ProviderConnectionState::Connected => None,
    }
}

/// Presents a stored report with its read-time status: a fresh report whose
/// freshness window has already closed reads as stale (the last reading stays
/// visible, clearly aged, instead of masquerading as live). The stored row is
/// not rewritten by a read; the refresh loop keeps storage truthful.
pub(crate) fn derive_read_time_status(mut report: QuotaReport, now: DateTime<Utc>) -> QuotaReport {
    if report.status == QuotaStatus::Fresh && now >= report.stale_at {
        report.status = QuotaStatus::Stale;
    }
    report
}

fn card_from_descriptor(
    descriptor: &AdapterDescriptor,
    report: Option<&QuotaReport>,
    connection_state: ProviderConnectionState,
    account_index: Option<u32>,
    quota_run: Option<&CollectorRunRow>,
) -> ProviderQuotaCard {
    let now = Utc::now();
    let effective_report =
        report.filter(|_| connection_state == ProviderConnectionState::Connected);
    match effective_report {
        Some(report) => ProviderQuotaCard {
            provider_id: descriptor.id.to_string(),
            display_name: descriptor.display_name.to_string(),
            account_index,
            connection_state,
            quota_support: descriptor.quota_support.label().to_string(),
            status: report.status,
            plan: report.plan.clone(),
            source: report.source.clone(),
            collected_at: report.collected_at,
            stale_at: report.stale_at,
            error_code: report.error_code.clone(),
            windows: report.windows.clone(),
        },
        None => ProviderQuotaCard {
            provider_id: descriptor.id.to_string(),
            display_name: descriptor.display_name.to_string(),
            account_index,
            connection_state,
            quota_support: descriptor.quota_support.label().to_string(),
            status: QuotaStatus::Unavailable,
            plan: None,
            source: descriptor.source_kind.label().to_string(),
            collected_at: now,
            stale_at: now,
            error_code: quota_run
                .and_then(|run| (!run.error_code.is_empty()).then_some(run.error_code.clone()))
                .or_else(|| state_error_code(connection_state)),
            windows: Vec::new(),
        },
    }
}

fn card_from_report(
    report: QuotaReport,
    registry: &AdapterRegistry,
    account_index: Option<u32>,
) -> ProviderQuotaCard {
    let provider_id = report.provider_id.clone();
    let display_name = registry
        .display_name(&provider_id)
        .unwrap_or(provider_id.as_str())
        .to_string();
    ProviderQuotaCard {
        provider_id,
        display_name,
        account_index,
        connection_state: if report.is_usable() {
            ProviderConnectionState::Connected
        } else {
            ProviderConnectionState::TransientError
        },
        quota_support: if report.windows.iter().any(|window| window.limit.is_some()) {
            ChannelSupport::Native.label().to_string()
        } else {
            ChannelSupport::LocalEstimate.label().to_string()
        },
        status: report.status,
        plan: report.plan,
        source: report.source,
        collected_at: report.collected_at,
        stale_at: report.stale_at,
        error_code: report.error_code,
        windows: report.windows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn window(key: &str, used: u64, limit: u64) -> QuotaWindow {
        QuotaWindow::with_limit(
            key,
            key,
            QuotaWindowScope::Weekly,
            QuotaKind::Tokens,
            used,
            std::num::NonZeroU64::new(limit).expect("fixture limit is non-zero"),
            None,
            Confidence::High,
        )
    }

    fn open_test_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let db_path = dir.path().join("test.db");
        let storage = Storage::open(&db_path).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    struct Fake(lnwdeck_provider_runtime::AdapterDescriptor);

    impl lnwdeck_provider_runtime::ProviderAdapter for Fake {
        fn descriptor(&self) -> lnwdeck_provider_runtime::AdapterDescriptor {
            self.0
        }
    }

    /// Registry mirroring the shipped order for the providers used below.
    fn test_registry() -> AdapterRegistry {
        use lnwdeck_provider_runtime::{AdapterDescriptor, AuthKind, ChannelSupport, SourceKind};
        let mut registry = AdapterRegistry::new();
        for (id, name) in [
            ("anthropic_claude", "Claude"),
            ("openai_codex", "OpenAI Codex"),
            ("opencode", "OpenCode"),
        ] {
            registry
                .register(Box::new(Fake(AdapterDescriptor {
                    id,
                    display_name: name,
                    vendor: "Vendor",
                    source_kind: SourceKind::LocalJsonl,
                    usage_support: ChannelSupport::LocalEstimate,
                    quota_support: ChannelSupport::LocalEstimate,
                    auth: AuthKind::LocalFiles,
                    adapter_version: "0.2.0",
                })))
                .expect("register");
        }
        registry
    }

    #[test]
    fn dashboard_returns_cards_in_registry_order() {
        let storage = open_test_db();
        let repo = QuotaRepository::new(&storage.conn);
        // Stored in the opposite order to the registry on purpose.
        repo.upsert_report(&QuotaReport::new(
            "openai_codex",
            "cli_api",
            vec![window("7d", 10, 50)],
            chrono::Duration::hours(1),
        ))
        .expect("codex");
        repo.upsert_report(&QuotaReport::new(
            "anthropic_claude",
            "cli_api",
            vec![window("5h", 40, 100)],
            chrono::Duration::hours(1),
        ))
        .expect("claude");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let ids: Vec<&str> = dashboard
            .providers
            .iter()
            .map(|c| c.provider_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["anthropic_claude", "openai_codex", "opencode"],
            "registered providers remain visible even when no report exists"
        );
        assert_eq!(dashboard.providers[0].display_name, "Claude");
        assert_eq!(dashboard.providers[0].windows[0].remaining, Some(60));
        assert!(dashboard.providers[2].windows.is_empty());
        assert!(dashboard.generated_at <= Utc::now());
    }

    #[test]
    fn dashboard_resolves_display_names_from_the_registry() {
        let storage = open_test_db();
        QuotaRepository::new(&storage.conn)
            .upsert_report(&QuotaReport::new(
                "opencode",
                "cli_api",
                vec![window("monthly", 5, 10)],
                chrono::Duration::hours(1),
            ))
            .expect("report");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let card = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "opencode")
            .expect("opencode card");
        assert_eq!(card.display_name, "OpenCode");
        assert_eq!(card.connection_state, ProviderConnectionState::Connected);
    }

    #[test]
    fn dashboard_keeps_different_account_fingerprints_as_separate_cards() {
        let storage = open_test_db();
        let repo = QuotaRepository::new(&storage.conn);
        for (fingerprint, used) in [("account-a", 10), ("account-b", 70)] {
            let mut report = QuotaReport::new(
                "anthropic_claude",
                "provider_api",
                vec![window("5h", used, 100)],
                chrono::Duration::hours(1),
            );
            report.account_fingerprint = Some(fingerprint.to_string());
            repo.upsert_report(&report).expect("account report");
        }

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let accounts: Vec<&ProviderQuotaCard> = dashboard
            .providers
            .iter()
            .filter(|card| card.provider_id == "anthropic_claude")
            .collect();
        assert_eq!(accounts.len(), 2);
        assert_eq!(
            accounts
                .iter()
                .map(|card| card.account_index)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
        assert_eq!(accounts[0].windows[0].used, 10);
        assert_eq!(accounts[1].windows[0].used, 70);
    }

    #[test]
    fn expired_fresh_reports_read_as_stale_but_stay_visible() {
        let storage = open_test_db();
        let mut report = QuotaReport::new(
            "anthropic_claude",
            "provider_api",
            vec![window("5h", 40, 100)],
            chrono::Duration::hours(1),
        );
        // Simulate a reading collected before the freshness window closed:
        // the stored status is still fresh because no newer collection ran.
        let now = Utc::now();
        report.collected_at = now - chrono::Duration::hours(2);
        report.stale_at = now - chrono::Duration::hours(1);
        QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("report");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let card = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "anthropic_claude")
            .expect("claude card");
        assert_eq!(
            card.status,
            QuotaStatus::Stale,
            "a reading older than its freshness window must never present as live"
        );
        assert_eq!(card.connection_state, ProviderConnectionState::Connected);
        assert_eq!(card.windows.len(), 1, "the last reading stays visible");
        assert_eq!(card.collected_at, now - chrono::Duration::hours(2));
    }

    #[test]
    fn legacy_gemini_fallback_reports_never_reach_the_dashboard() {
        let storage = open_test_db();
        let repo = QuotaRepository::new(&storage.conn);
        // The retired retrieveUserQuota fallback stored placeholder windows
        // (0% used / 100% remaining) under the Gemini CLI account fingerprint.
        let mut legacy = QuotaReport::new(
            "google_gemini",
            "provider_api",
            vec![window("pro", 0, 1)],
            chrono::Duration::hours(1),
        );
        legacy.account_fingerprint = Some("old-cli-account".to_string());
        repo.upsert_report(&legacy).expect("legacy report");
        // The current Language Server source is the only one that may show.
        let mut current = QuotaReport::new(
            "google_gemini",
            "antigravity_ls",
            vec![window("pro", 40, 100)],
            chrono::Duration::hours(1),
        );
        current.account_fingerprint = Some("keyring-account".to_string());
        repo.upsert_report(&current).expect("current report");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let gemini: Vec<&ProviderQuotaCard> = dashboard
            .providers
            .iter()
            .filter(|card| card.provider_id == "google_gemini")
            .collect();
        assert_eq!(
            gemini.len(),
            1,
            "the retired fallback account must not keep a card on the dashboard"
        );
        assert_eq!(gemini[0].source, "antigravity_ls");
        assert_eq!(gemini[0].windows[0].used, 40);
    }

    #[test]
    fn an_ide_required_run_keeps_the_cached_reading_as_stale() {
        let storage = open_test_db();
        // A usable report from an earlier successful collection.
        let mut report = QuotaReport::new(
            "anthropic_claude",
            "provider_api",
            vec![window("5h", 40, 100)],
            chrono::Duration::hours(1),
        );
        report.collected_at = Utc::now() - chrono::Duration::minutes(5);
        report.stale_at = Utc::now() + chrono::Duration::minutes(55);
        QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("report");
        // The latest quota run needs the Antigravity IDE running: the cached
        // reading stays visible but must read as stale, not live.
        let diag = DiagnosticsRepository::new(&storage.conn);
        diag.insert_collector_run(&CollectorRunRow {
            id: 0,
            provider_id: "anthropic_claude".to_string(),
            collector_mode: "quota_collect".to_string(),
            started_at: (Utc::now() - chrono::Duration::seconds(5)).to_rfc3339(),
            finished_at: Utc::now().to_rfc3339(),
            duration_ms: 500,
            source_records_seen: 0,
            records_parsed: 0,
            events_normalized: 0,
            events_rejected: 0,
            duplicates_skipped: 0,
            events_inserted: 0,
            quota_snapshots_inserted: 0,
            warning_codes: Vec::new(),
            error_code: "SOURCE_REQUIRES_IDE".to_string(),
            next_retry_at: None,
        })
        .expect("run");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let card = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "anthropic_claude")
            .expect("claude card");
        assert_eq!(card.connection_state, ProviderConnectionState::Connected);
        assert_eq!(
            card.status,
            QuotaStatus::Stale,
            "the last reading is old data the moment the IDE closes, not live"
        );
        assert_eq!(card.error_code.as_deref(), Some("SOURCE_REQUIRES_IDE"));
        assert_eq!(card.windows.len(), 1, "the cached reading stays visible");
        assert_eq!(card.windows[0].used, 40);
    }

    #[test]
    fn a_failed_run_without_cached_data_hides_windows_and_carries_the_real_code() {
        let storage = open_test_db();
        // A usable report from an earlier successful collection.
        let mut report = QuotaReport::new(
            "anthropic_claude",
            "provider_api",
            vec![window("5h", 40, 100)],
            chrono::Duration::hours(1),
        );
        report.collected_at = Utc::now() - chrono::Duration::minutes(5);
        report.stale_at = Utc::now() + chrono::Duration::minutes(55);
        QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("report");
        // The latest quota run failed for a reason that invalidates the data.
        let diag = DiagnosticsRepository::new(&storage.conn);
        diag.insert_collector_run(&CollectorRunRow {
            id: 0,
            provider_id: "anthropic_claude".to_string(),
            collector_mode: "quota_collect".to_string(),
            started_at: (Utc::now() - chrono::Duration::seconds(5)).to_rfc3339(),
            finished_at: Utc::now().to_rfc3339(),
            duration_ms: 500,
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

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let card = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "anthropic_claude")
            .expect("claude card");
        assert_eq!(card.connection_state, ProviderConnectionState::AuthExpired);
        assert_eq!(card.status, QuotaStatus::Unavailable);
        assert_eq!(card.error_code.as_deref(), Some("AUTH_EXPIRED"));
        assert!(
            card.windows.is_empty(),
            "a failed run must not keep stale percentages on screen"
        );
    }

    #[test]
    fn an_ide_required_run_without_cached_data_shows_only_the_guidance_card() {
        let storage = open_test_db();
        let diag = DiagnosticsRepository::new(&storage.conn);
        diag.upsert_provider_state(&ProviderStateRow {
            provider_id: "anthropic_claude".to_string(),
            display_name: "Claude".to_string(),
            enabled: true,
            detected: true,
            detection_method: "local_scan".to_string(),
            source_type: "local_files".to_string(),
            source_exists: true,
            permission_state: "read_ok".to_string(),
            adapter_version: "0.2.0".to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        })
        .expect("state");
        diag.insert_collector_run(&CollectorRunRow {
            id: 0,
            provider_id: "anthropic_claude".to_string(),
            collector_mode: "quota_collect".to_string(),
            started_at: (Utc::now() - chrono::Duration::seconds(5)).to_rfc3339(),
            finished_at: Utc::now().to_rfc3339(),
            duration_ms: 500,
            source_records_seen: 0,
            records_parsed: 0,
            events_normalized: 0,
            events_rejected: 0,
            duplicates_skipped: 0,
            events_inserted: 0,
            quota_snapshots_inserted: 0,
            warning_codes: Vec::new(),
            error_code: "SOURCE_REQUIRES_IDE".to_string(),
            next_retry_at: None,
        })
        .expect("run");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let card = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "anthropic_claude")
            .expect("claude card");
        assert_eq!(
            card.connection_state,
            ProviderConnectionState::TransientError
        );
        assert_eq!(card.status, QuotaStatus::Unavailable);
        assert_eq!(card.error_code.as_deref(), Some("SOURCE_REQUIRES_IDE"));
        assert!(card.windows.is_empty());
    }

    #[test]
    fn dashboard_includes_registered_providers_without_reports_as_disconnected() {
        let storage = open_test_db();
        QuotaRepository::new(&storage.conn)
            .upsert_report(&QuotaReport::new(
                "anthropic_claude",
                "cli_api",
                vec![window("5h", 40, 100)],
                chrono::Duration::hours(1),
            ))
            .expect("report");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        assert_eq!(dashboard.providers.len(), 3);
        let missing = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "opencode")
            .expect("registered provider without a report");
        let json = serde_json::to_value(missing).expect("serialize card");
        assert_eq!(json["connection_state"], "not_detected");
        assert_eq!(json["windows"], serde_json::json!([]));
    }

    #[test]
    fn unsupported_provider_without_local_source_is_not_connected() {
        use lnwdeck_provider_runtime::{AuthKind, SourceKind};

        let descriptor = AdapterDescriptor {
            id: "local_only",
            display_name: "Local only",
            vendor: "Vendor",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Unsupported,
            auth: AuthKind::LocalFiles,
            adapter_version: "test",
        };

        assert_eq!(
            connection_state_for(&descriptor, None, None, None),
            ProviderConnectionState::NotDetected
        );

        let missing_report = QuotaReport::failed("local_only", "provider_api", "NOT_CONFIGURED");
        assert_eq!(
            connection_state_for(&descriptor, None, None, Some(&missing_report)),
            ProviderConnectionState::NotDetected
        );
    }

    #[test]
    fn a_provider_missing_from_the_registry_keeps_its_id_and_sorts_last() {
        let storage = open_test_db();
        let repo = QuotaRepository::new(&storage.conn);
        repo.upsert_report(&QuotaReport::new(
            "mystery_provider",
            "cli_api",
            vec![window("5h", 1, 10)],
            chrono::Duration::hours(1),
        ))
        .expect("mystery");
        repo.upsert_report(&QuotaReport::new(
            "opencode",
            "cli_api",
            vec![window("5h", 1, 10)],
            chrono::Duration::hours(1),
        ))
        .expect("opencode");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let ids: Vec<&str> = dashboard
            .providers
            .iter()
            .map(|c| c.provider_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "anthropic_claude",
                "openai_codex",
                "opencode",
                "mystery_provider"
            ],
            "registered cards stay ordered and stored data for an unknown id stays visible last"
        );
        assert_eq!(dashboard.providers[3].display_name, "mystery_provider");
    }

    #[test]
    fn usage_only_windows_reach_the_dashboard_without_percentages() {
        let storage = open_test_db();
        QuotaRepository::new(&storage.conn)
            .upsert_report(&QuotaReport::new(
                "opencode",
                "provider_api",
                vec![QuotaWindow::usage_only(
                    "5h",
                    "5-hour",
                    QuotaWindowScope::Rolling,
                    QuotaKind::Tokens,
                    775,
                    None,
                    Confidence::Medium,
                )],
                chrono::Duration::hours(1),
            ))
            .expect("report");

        let dashboard =
            QueryQuotaDashboard::execute(&storage.conn, &test_registry()).expect("dashboard");
        let card = dashboard
            .providers
            .iter()
            .find(|card| card.provider_id == "opencode")
            .expect("opencode card");
        let window = &card.windows[0];
        assert_eq!(window.used, 775);
        assert_eq!(window.limit, None);
        assert_eq!(window.remaining_percent, None);
    }

    #[test]
    fn legacy_opencode_estimate_is_hidden_without_deleting_history() {
        let report = QuotaReport::new(
            "opencode",
            "local_estimate",
            vec![window("5h", 80, 100)],
            chrono::Duration::hours(1),
        );
        let sanitized = sanitize_legacy_opencode_report(report);
        assert_eq!(sanitized.status, QuotaStatus::Unavailable);
        assert_eq!(sanitized.error_code.as_deref(), Some("NOT_CONFIGURED"));
        assert!(sanitized.windows.is_empty());
    }
}
