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
}

pub struct ScanProviders;

impl ScanProviders {
    pub fn execute(conn: &Connection) -> Result<Vec<DetailedProviderInfo>, rusqlite::Error> {
        let diag = DiagnosticsRepository::new(conn);
        let states = diag.provider_states().unwrap_or_default();
        let runs = diag.latest_runs().unwrap_or_default();

        let mut results = Vec::new();

        if states.is_empty() {
            // Check usage_events directly if states repository is empty
            let mut stmt = conn.prepare(
                "SELECT provider_id, COUNT(*), COALESCE(SUM(tokens_input + tokens_output), 0), MAX(timestamp)
                 FROM usage_events GROUP BY provider_id",
            )?;
            let rows = stmt.query_map([], |row| {
                let pid: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                let tokens: i64 = row.get(2)?;
                let max_ts: Option<String> = row.get(3)?;
                Ok(DetailedProviderInfo {
                    provider_id: pid.clone(),
                    display_name: pid.clone(),
                    enabled: true,
                    detected: true,
                    source_type: "auto".to_string(),
                    health_status: "Healthy".to_string(),
                    event_count: count,
                    total_tokens: tokens,
                    last_sync: max_ts,
                    quota_summary: "Active".to_string(),
                    reset_at: None,
                    confidence: "High".to_string(),
                })
            })?;
            for info in rows.flatten() {
                results.push(info);
            }

            return Ok(results);
        }

        for st in states {
            let run = runs.iter().find(|r| r.provider_id == st.provider_id);
            let (event_count, total_tokens, last_ts): (i64, i64, Option<String>) = conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(tokens_input + tokens_output), 0), MAX(timestamp)
                     FROM usage_events WHERE provider_id = ?",
                    [&st.provider_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap_or((0, 0, None));

            let health = if let Some(r) = run {
                if !r.error_code.is_empty() {
                    format!("Error ({})", r.error_code)
                } else {
                    "Healthy".to_string()
                }
            } else if st.detected {
                "Healthy".to_string()
            } else {
                "Not detected".to_string()
            };

            results.push(DetailedProviderInfo {
                provider_id: st.provider_id.clone(),
                display_name: st.display_name.clone(),
                enabled: st.enabled,
                detected: st.detected,
                source_type: st.source_type.clone(),
                health_status: health,
                event_count,
                total_tokens,
                last_sync: last_ts.or_else(|| run.map(|r| r.finished_at.clone())),
                quota_summary: "Active".to_string(),
                reset_at: None,
                confidence: "High".to_string(),
            });
        }

        Ok(results)
    }
}
