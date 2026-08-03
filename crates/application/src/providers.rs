use rusqlite::Connection;

pub struct ScanProviders;

#[derive(Debug)]
pub struct ProviderSummary {
    pub provider_id: String,
    pub event_count: i64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub latest_event_at: Option<String>,
}

impl ScanProviders {
    pub fn execute(conn: &Connection) -> Result<Vec<ProviderSummary>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT provider_id,
                    COUNT(*) as cnt,
                    COALESCE(SUM(tokens_input), 0),
                    COALESCE(SUM(tokens_output), 0),
                    MAX(timestamp)
             FROM usage_events
             GROUP BY provider_id
             ORDER BY cnt DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProviderSummary {
                provider_id: row.get(0)?,
                event_count: row.get(1)?,
                total_tokens_input: row.get(2)?,
                total_tokens_output: row.get(3)?,
                latest_event_at: row.get(4)?,
            })
        })?;

        let mut providers = Vec::new();
        for row in rows {
            providers.push(row?);
        }
        Ok(providers)
    }
}
