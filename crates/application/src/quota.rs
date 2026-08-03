use chrono::{DateTime, Utc};
use lnwdeck_domain::{QuotaReport, QuotaStatus, QuotaWindow};
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

/// Builds the quota dashboard from the latest stored reports, ordered by the
/// canonical provider registry order.
pub struct QueryQuotaDashboard;

impl QueryQuotaDashboard {
    pub fn execute(conn: &Connection) -> Result<QuotaDashboard, rusqlite::Error> {
        let reports = QuotaRepository::new(conn).latest_all()?;
        let mut cards: Vec<ProviderQuotaCard> = reports.into_iter().map(card_from_report).collect();
        cards.sort_by_key(|card| registry_rank(&card.provider_id));
        Ok(QuotaDashboard {
            generated_at: Utc::now(),
            providers: cards,
        })
    }
}

fn card_from_report(report: QuotaReport) -> ProviderQuotaCard {
    let provider_id = report.provider_id.clone();
    ProviderQuotaCard {
        provider_id: provider_id.clone(),
        display_name: display_name_for(&provider_id),
        status: report.status,
        plan: report.plan,
        source: report.source,
        collected_at: report.collected_at,
        stale_at: report.stale_at,
        error_code: report.error_code,
        windows: report.windows,
    }
}

fn registry_rank(provider_id: &str) -> usize {
    REGISTRY_ORDER
        .iter()
        .position(|id| *id == provider_id)
        .unwrap_or(usize::MAX)
}

fn display_name_for(provider_id: &str) -> String {
    REGISTRY_NAMES
        .iter()
        .find(|(id, _)| *id == provider_id)
        .map(|(_, name)| *name)
        .unwrap_or(provider_id)
        .to_string()
}

const REGISTRY_ORDER: &[&str] = &[
    "anthropic_claude",
    "openai_codex",
    "google_gemini",
    "kiro_ai",
    "opencode",
    "github_copilot",
    "cursor_ide",
    "xai_grok",
    "openrouter_api",
    "ollama_local",
];

const REGISTRY_NAMES: &[(&str, &str)] = &[
    ("anthropic_claude", "Claude"),
    ("openai_codex", "Codex"),
    ("google_gemini", "Gemini"),
    ("kiro_ai", "Kiro"),
    ("opencode", "OpenCode"),
    ("github_copilot", "Copilot"),
    ("cursor_ide", "Cursor"),
    ("xai_grok", "Grok"),
    ("openrouter_api", "OpenRouter"),
    ("ollama_local", "Ollama"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn window(key: &str, used: u64, limit: u64) -> QuotaWindow {
        QuotaWindow::new(
            key,
            key,
            QuotaWindowScope::Weekly,
            QuotaKind::Tokens,
            used,
            limit,
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

    fn provider_id_from_registry_name(name: &str) -> String {
        REGISTRY_NAMES
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(id, _)| id.to_string())
            .expect("known provider")
    }

    #[test]
    fn dashboard_returns_cards_in_registry_order() {
        let storage = open_test_db();
        let repo = QuotaRepository::new(&storage.conn);
        let claude = QuotaReport::new(
            "anthropic_claude",
            "cli_api",
            vec![window("5h", 40, 100)],
            chrono::Duration::hours(1),
        );
        let codex = QuotaReport::new(
            "openai_codex",
            "cli_api",
            vec![window("7d", 10, 50)],
            chrono::Duration::hours(1),
        );
        repo.upsert_report(&claude).expect("claude");
        repo.upsert_report(&codex).expect("codex");

        let dashboard = QueryQuotaDashboard::execute(&storage.conn).expect("dashboard");
        let names: Vec<&str> = dashboard
            .providers
            .iter()
            .map(|c| c.provider_id.as_str())
            .collect();
        assert_eq!(names, vec!["anthropic_claude", "openai_codex"]);
        assert_eq!(dashboard.providers[0].display_name, "Claude");
        assert_eq!(dashboard.providers[0].windows[0].remaining, 60);
        assert!(dashboard.generated_at <= Utc::now());
    }

    #[test]
    fn dashboard_resolves_registry_display_names() {
        let storage = open_test_db();
        let repo = QuotaRepository::new(&storage.conn);
        let report = QuotaReport::new(
            "opencode",
            "cli_api",
            vec![window("monthly", 5, 10)],
            chrono::Duration::hours(1),
        );
        repo.upsert_report(&report).expect("report");

        let dashboard = QueryQuotaDashboard::execute(&storage.conn).expect("dashboard");
        assert_eq!(dashboard.providers.len(), 1);
        assert_eq!(dashboard.providers[0].provider_id, "opencode");
        assert_eq!(dashboard.providers[0].display_name, "OpenCode");
    }

    #[test]
    fn unknown_provider_keeps_raw_id_as_display_name() {
        let names: std::collections::HashMap<&str, &str> = REGISTRY_NAMES.iter().copied().collect();
        assert_eq!(display_name_for("mystery"), "mystery");
        assert_eq!(display_name_for("anthropic_claude"), "Claude");
        assert_eq!(
            names.get("openai_codex").copied().unwrap(),
            "Codex",
            "registry table stays in sync"
        );
    }

    #[test]
    fn registry_order_covers_all_display_names() {
        assert_eq!(REGISTRY_ORDER.len(), REGISTRY_NAMES.len());
        for (id, _) in REGISTRY_NAMES {
            assert!(REGISTRY_ORDER.contains(id), "{id} must be ordered");
        }
    }

    #[test]
    fn provider_id_from_registry_name_roundtrip() {
        let id = provider_id_from_registry_name("Gemini");
        assert_eq!(id, "google_gemini");
    }
}
