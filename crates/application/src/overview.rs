use rusqlite::Connection;

pub struct QueryOverview;

#[derive(Debug)]
pub struct OverviewResult {
    pub total_events: i64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub provider_count: i64,
    pub top_providers: Vec<TopProvider>,
    pub latest_event_at: Option<String>,
    pub oldest_event_at: Option<String>,
    pub high_confidence_count: i64,
    pub confidence_coverage: f64,
}

#[derive(Debug)]
pub struct TopProvider {
    pub provider_id: String,
    pub event_count: i64,
}

impl QueryOverview {
    pub fn execute(conn: &Connection) -> Result<OverviewResult, rusqlite::Error> {
        let total_events: i64 =
            conn.query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))?;

        let total_tokens_input: i64 = conn.query_row(
            "SELECT COALESCE(SUM(tokens_input), 0) FROM usage_events",
            [],
            |row| row.get(0),
        )?;

        let total_tokens_output: i64 = conn.query_row(
            "SELECT COALESCE(SUM(tokens_output), 0) FROM usage_events",
            [],
            |row| row.get(0),
        )?;

        let provider_count: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT provider_id) FROM usage_events",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT provider_id, COUNT(*) as cnt FROM usage_events
             GROUP BY provider_id ORDER BY cnt DESC LIMIT 5",
        )?;
        let top_providers: Vec<TopProvider> = stmt
            .query_map([], |row| {
                Ok(TopProvider {
                    provider_id: row.get(0)?,
                    event_count: row.get(1)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let latest_event_at: Option<String> = conn
            .query_row("SELECT MAX(timestamp) FROM usage_events", [], |row| {
                row.get(0)
            })
            .ok();

        let oldest_event_at: Option<String> = conn
            .query_row("SELECT MIN(timestamp) FROM usage_events", [], |row| {
                row.get(0)
            })
            .ok();

        let high_confidence_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM usage_events WHERE confidence = 'High'",
            [],
            |row| row.get(0),
        )?;

        let confidence_coverage = if total_events > 0 {
            high_confidence_count as f64 / total_events as f64
        } else {
            0.0
        };

        Ok(OverviewResult {
            total_events,
            total_tokens_input,
            total_tokens_output,
            provider_count,
            top_providers,
            latest_event_at,
            oldest_event_at,
            high_confidence_count,
            confidence_coverage,
        })
    }
}
