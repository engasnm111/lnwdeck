use lnwdeck_domain::{QuotaReport, QuotaStatus};
use lnwdeck_storage::repositories::DiagnosticsRepository;
use rusqlite::Connection;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DetailedProviderInfo {
    pub provider_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub detected: bool,
    pub source_type: String,
    pub health_status: String,
    pub event_count: i64,
    pub total_tokens: i64,
    pub last_sync: Option<String>,
    pub quota_summary: String,
    pub reset_at: Option<String>,
    pub confidence: String,
    pub cost_support: String,
}

pub struct ScanProviders;

pub struct StandardProviderMeta {
    pub id: &'static str,
    pub display_name: &'static str,
    pub source_type: &'static str,
    pub default_health: &'static str,
    pub cost_support: &'static str,
    /// Adapter provider ids that may produce quota reports for this card.
    pub quota_ids: &'static [&'static str],
}

pub const STANDARD_PROVIDERS: &[StandardProviderMeta] = &[
    StandardProviderMeta {
        id: "opencode",
        display_name: "OpenCode",
        source_type: "Local CLI / JSON",
        default_health: "Detected",
        cost_support: "Exact",
        quota_ids: &["opencode"],
    },
    StandardProviderMeta {
        id: "openai_codex",
        display_name: "Codex (OpenAI)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
        quota_ids: &["openai_codex"],
    },
    StandardProviderMeta {
        id: "google_gemini",
        display_name: "Gemini (Google)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
        quota_ids: &["google_gemini"],
    },
    StandardProviderMeta {
        id: "kiro_ai",
        display_name: "Kiro",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Estimated",
        quota_ids: &["kiro_ai"],
    },
    StandardProviderMeta {
        id: "anthropic_claude",
        display_name: "Claude (Anthropic)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
        quota_ids: &["anthropic_claude"],
    },
    StandardProviderMeta {
        id: "copilot",
        display_name: "GitHub Copilot",
        source_type: "IDE Extension",
        default_health: "Not configured",
        cost_support: "Unavailable",
        quota_ids: &["github_copilot"],
    },
    StandardProviderMeta {
        id: "cursor",
        display_name: "Cursor",
        source_type: "Local Log / SQLite",
        default_health: "Not configured",
        cost_support: "Estimated",
        quota_ids: &["cursor_ide"],
    },
    StandardProviderMeta {
        id: "grok",
        display_name: "Grok (xAI)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
        quota_ids: &["xai_grok"],
    },
    StandardProviderMeta {
        id: "ollama",
        display_name: "Ollama",
        source_type: "Local HTTP API",
        default_health: "Not configured",
        cost_support: "Free / Local",
        quota_ids: &["ollama_local"],
    },
    StandardProviderMeta {
        id: "openrouter",
        display_name: "OpenRouter",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
        quota_ids: &["openrouter_api"],
    },
];

impl ScanProviders {
    pub fn execute(conn: &Connection) -> Result<Vec<DetailedProviderInfo>, rusqlite::Error> {
        let diag = DiagnosticsRepository::new(conn);
        let states = diag.provider_states().unwrap_or_default();
        let runs = diag.latest_runs().unwrap_or_default();
        let reports = lnwdeck_storage::repositories::QuotaRepository::new(conn)
            .latest_all()
            .unwrap_or_default();

        let mut results: Vec<DetailedProviderInfo> = Vec::new();

        for std_prov in STANDARD_PROVIDERS {
            let state_opt = states.iter().find(|s| s.provider_id == std_prov.id);
            let run_opt = runs.iter().find(|r| r.provider_id == std_prov.id);
            let report = reports
                .iter()
                .find(|r| std_prov.quota_ids.contains(&r.provider_id.as_str()));

            let like_pat = format!("%{}%", std_prov.id);
            let (event_count, total_tokens, last_ts): (i64, i64, Option<String>) = conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(tokens_input + tokens_output), 0), MAX(timestamp)
                     FROM usage_events WHERE provider_id = ? OR provider_id LIKE ?",
                    [std_prov.id, &like_pat],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap_or((0, 0, None));

            let detected = state_opt
                .map(|s| s.detected)
                .unwrap_or(std_prov.id == "opencode");
            let enabled = state_opt.map(|s| s.enabled).unwrap_or(true);

            let health = if let Some(r) = run_opt {
                if !r.error_code.is_empty() {
                    format!("Error ({})", r.error_code)
                } else {
                    "Healthy".to_string()
                }
            } else if detected && event_count > 0 {
                "Healthy".to_string()
            } else if detected {
                "Detected".to_string()
            } else {
                std_prov.default_health.to_string()
            };

            let (quota_summary, reset_at, confidence) = match report {
                Some(report) => quota_card_fields(report),
                None => ("No quota data".to_string(), None, "n/a".to_string()),
            };

            results.push(DetailedProviderInfo {
                provider_id: std_prov.id.to_string(),
                display_name: std_prov.display_name.to_string(),
                enabled,
                detected,
                source_type: std_prov.source_type.to_string(),
                health_status: health,
                event_count,
                total_tokens,
                last_sync: last_ts.or_else(|| run_opt.map(|r| r.finished_at.clone())),
                quota_summary,
                reset_at,
                confidence,
                cost_support: std_prov.cost_support.to_string(),
            });
        }

        Ok(results)
    }
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
    } else if let Some(window) = report.windows.iter().find(|w| w.limit > 0) {
        let pct = window.remaining_percent.round() as u64;
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

    fn window(key: &str, used: u64, limit: u64, reset: Option<&str>) -> QuotaWindow {
        let reset_at = reset.map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .expect("rfc3339")
                .with_timezone(&chrono::Utc)
        });
        QuotaWindow::new(
            key,
            key,
            QuotaWindowScope::Weekly,
            QuotaKind::Tokens,
            used,
            limit,
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
            vec![window("5h", 775, 0, None)],
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
    fn scan_providers_joins_quota_report_for_matching_adapter_id() {
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

        let providers = ScanProviders::execute(&storage.conn).expect("scan");
        let opencode = providers
            .iter()
            .find(|p| p.provider_id == "opencode")
            .expect("opencode card");
        assert_eq!(opencode.quota_summary, "60% left");
        assert_eq!(opencode.confidence, "High");
    }

    #[test]
    fn scan_providers_uses_quota_ids_for_adapter_mismatched_cards() {
        let storage = open_test_db();
        let report = QuotaReport::new(
            "ollama_local",
            "local_api",
            vec![QuotaWindow::unlimited(
                QuotaWindowScope::Other,
                QuotaKind::Requests,
            )],
            chrono::Duration::hours(1),
        );
        lnwdeck_storage::repositories::QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("upsert");

        let providers = ScanProviders::execute(&storage.conn).expect("scan");
        let ollama = providers
            .iter()
            .find(|p| p.provider_id == "ollama")
            .expect("ollama card");
        assert_eq!(ollama.quota_summary, "Local / Unlimited");
    }
}
