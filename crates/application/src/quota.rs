use chrono::{DateTime, Utc};
use lnwdeck_domain::{QuotaReport, QuotaStatus, QuotaWindow};
use lnwdeck_provider_runtime::{AdapterDescriptor, AdapterRegistry, ChannelSupport};
use lnwdeck_storage::repositories::{
    CollectorRunRow, DiagnosticsRepository, ProviderStateRow, QuotaRepository,
};
use rusqlite::Connection;
use serde::Serialize;

/// Dashboard read model for quota: one card per registered provider.
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

/// Converts the pre-dashboard OpenCode local estimate into an explicit
/// unavailable state at read time. Old installations may still have a
/// hard-coded local percentage stored from an earlier adapter version; it
/// must never remain visible while the provider now requires its workspace
/// dashboard credential. The historical window rows stay in storage.
pub(crate) fn sanitize_legacy_opencode_report(report: QuotaReport) -> QuotaReport {
    if report.provider_id == "opencode" && report.source == "local_estimate" {
        QuotaReport::failed("opencode", "provider_api", "NOT_CONFIGURED")
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
        let reports = QuotaRepository::new(conn)
            .latest_all()?
            .into_iter()
            .map(sanitize_legacy_opencode_report)
            .collect::<Vec<_>>();
        let diagnostics = DiagnosticsRepository::new(conn);
        let states = diagnostics.provider_states()?;
        let runs = diagnostics.latest_runs()?;
        let descriptors = registry.descriptors();
        let mut cards = Vec::with_capacity(descriptors.len() + reports.len());

        for descriptor in &descriptors {
            let report = reports.iter().find(|r| r.provider_id == descriptor.id);
            let state = states.iter().find(|s| s.provider_id == descriptor.id);
            let quota_run = runs
                .iter()
                .find(|r| r.provider_id == descriptor.id && r.collector_mode == "quota_collect");
            let connection_state = connection_state_for(descriptor, state, quota_run, report);
            cards.push(card_from_descriptor(descriptor, report, connection_state));
        }

        // Unknown ids (for example a provider removed in a later build) sort
        // last instead of being dropped, so stored data stays visible.
        for report in reports {
            if registry.rank(&report.provider_id).is_none() {
                cards.push(card_from_report(report, registry));
            }
        }
        // Unknown ids (for example a provider removed in a later build) sort
        // last instead of being dropped, so stored data stays visible.
        cards.sort_by_key(|card| registry.rank(&card.provider_id).unwrap_or(usize::MAX));
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

fn card_from_descriptor(
    descriptor: &AdapterDescriptor,
    report: Option<&QuotaReport>,
    connection_state: ProviderConnectionState,
) -> ProviderQuotaCard {
    let now = Utc::now();
    let effective_report =
        report.filter(|_| connection_state == ProviderConnectionState::Connected);
    match effective_report {
        Some(report) => ProviderQuotaCard {
            provider_id: descriptor.id.to_string(),
            display_name: descriptor.display_name.to_string(),
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
            connection_state,
            quota_support: descriptor.quota_support.label().to_string(),
            status: QuotaStatus::Unavailable,
            plan: None,
            source: descriptor.source_kind.label().to_string(),
            collected_at: now,
            stale_at: now,
            error_code: state_error_code(connection_state),
            windows: Vec::new(),
        },
    }
}

fn card_from_report(report: QuotaReport, registry: &AdapterRegistry) -> ProviderQuotaCard {
    let provider_id = report.provider_id.clone();
    let display_name = registry
        .display_name(&provider_id)
        .unwrap_or(provider_id.as_str())
        .to_string();
    ProviderQuotaCard {
        provider_id,
        display_name,
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
