use chrono::{DateTime, Utc};
use lnwdeck_domain::{QuotaReport, QuotaStatus, QuotaWindow};
use lnwdeck_provider_runtime::AdapterRegistry;
use lnwdeck_storage::repositories::QuotaRepository;
use rusqlite::Connection;
use serde::Serialize;

/// Dashboard read model for quota: one card per provider that has a report.
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
    pub status: QuotaStatus,
    pub plan: Option<String>,
    pub source: String,
    pub collected_at: DateTime<Utc>,
    pub stale_at: DateTime<Utc>,
    pub error_code: Option<String>,
    pub windows: Vec<QuotaWindow>,
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
        let reports = QuotaRepository::new(conn).latest_all()?;
        let mut cards: Vec<ProviderQuotaCard> = reports
            .into_iter()
            .map(|report| card_from_report(report, registry))
            .collect();
        // Unknown ids (for example a provider removed in a later build) sort
        // last instead of being dropped, so stored data stays visible.
        cards.sort_by_key(|card| registry.rank(&card.provider_id).unwrap_or(usize::MAX));
        Ok(QuotaDashboard {
            generated_at: Utc::now(),
            providers: cards,
        })
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
            ("openai_codex", "Codex"),
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
        assert_eq!(ids, vec!["anthropic_claude", "openai_codex"]);
        assert_eq!(dashboard.providers[0].display_name, "Claude");
        assert_eq!(dashboard.providers[0].windows[0].remaining, Some(60));
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
        assert_eq!(dashboard.providers.len(), 1);
        assert_eq!(dashboard.providers[0].provider_id, "opencode");
        assert_eq!(dashboard.providers[0].display_name, "OpenCode");
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
            vec!["opencode", "mystery_provider"],
            "stored data for an unknown id stays visible, ordered last"
        );
        assert_eq!(dashboard.providers[1].display_name, "mystery_provider");
    }

    #[test]
    fn usage_only_windows_reach_the_dashboard_without_percentages() {
        let storage = open_test_db();
        QuotaRepository::new(&storage.conn)
            .upsert_report(&QuotaReport::new(
                "opencode",
                "local_estimate",
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
        let window = &dashboard.providers[0].windows[0];
        assert_eq!(window.used, 775);
        assert_eq!(window.limit, None);
        assert_eq!(window.remaining_percent, None);
    }
}
