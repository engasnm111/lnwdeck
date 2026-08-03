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
}

pub const STANDARD_PROVIDERS: &[StandardProviderMeta] = &[
    StandardProviderMeta {
        id: "opencode",
        display_name: "OpenCode",
        source_type: "Local CLI / JSON",
        default_health: "Detected",
        cost_support: "Exact",
    },
    StandardProviderMeta {
        id: "openai_codex",
        display_name: "Codex (OpenAI)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
    },
    StandardProviderMeta {
        id: "google_gemini",
        display_name: "Gemini (Google)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
    },
    StandardProviderMeta {
        id: "kiro_ai",
        display_name: "Kimi",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Estimated",
    },
    StandardProviderMeta {
        id: "anthropic_claude",
        display_name: "Claude (Anthropic)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
    },
    StandardProviderMeta {
        id: "copilot",
        display_name: "GitHub Copilot",
        source_type: "IDE Extension",
        default_health: "Not configured",
        cost_support: "Unavailable",
    },
    StandardProviderMeta {
        id: "cursor",
        display_name: "Cursor",
        source_type: "Local Log / SQLite",
        default_health: "Not configured",
        cost_support: "Estimated",
    },
    StandardProviderMeta {
        id: "grok",
        display_name: "Grok (xAI)",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
    },
    StandardProviderMeta {
        id: "ollama",
        display_name: "Ollama",
        source_type: "Local HTTP API",
        default_health: "Not configured",
        cost_support: "Free / Local",
    },
    StandardProviderMeta {
        id: "openrouter",
        display_name: "OpenRouter",
        source_type: "API / Credential",
        default_health: "Not configured",
        cost_support: "Exact",
    },
];

impl ScanProviders {
    pub fn execute(conn: &Connection) -> Result<Vec<DetailedProviderInfo>, rusqlite::Error> {
        let diag = DiagnosticsRepository::new(conn);
        let states = diag.provider_states().unwrap_or_default();
        let runs = diag.latest_runs().unwrap_or_default();

        let mut results: Vec<DetailedProviderInfo> = Vec::new();

        for std_prov in STANDARD_PROVIDERS {
            let state_opt = states.iter().find(|s| s.provider_id == std_prov.id);
            let run_opt = runs.iter().find(|r| r.provider_id == std_prov.id);

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

            let quota_summary = if event_count > 0 {
                format!("{event_count} events recorded")
            } else if detected {
                "No records yet".to_string()
            } else {
                "Not configured".to_string()
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
                reset_at: None,
                confidence: "High".to_string(),
                cost_support: std_prov.cost_support.to_string(),
            });
        }

        Ok(results)
    }
}
