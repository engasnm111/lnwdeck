use lnwdeck_pricing::{calculator::calculate_cost_with_provider, catalog::PriceResolver};
use rusqlite::Connection;
use serde_json::json;

pub struct QueryOverview;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OverviewResult {
    pub total_events: i64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub total_cost: f64,
    pub cost_formatted: String,
    pub cost_status: String,
    pub provider_count: i64,
    pub top_providers: Vec<TopProvider>,
    pub latest_event_at: Option<String>,
    pub oldest_event_at: Option<String>,
    pub high_confidence_count: i64,
    pub confidence_coverage: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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

        // Calculate Cost Pipeline
        let price_resolver = PriceResolver::new_with_overrides(&json!([]));
        let mut total_cost: f64 = 0.0;
        let mut has_computable_cost = false;

        if total_events > 0 {
            let mut event_stmt = conn.prepare(
                "SELECT provider_id, model, tokens_input, tokens_output, cost FROM usage_events",
            )?;
            let rows = event_stmt.query_map([], |row| {
                let pid: String = row.get(0)?;
                let model: String = row.get(1)?;
                let tin: i64 = row.get(2)?;
                let tout: i64 = row.get(3)?;
                let raw_cost: String = row.get(4)?;
                Ok((pid, model, tin, tout, raw_cost))
            })?;

            for r in rows.flatten() {
                let (pid, model, tin, tout, raw_cost) = r;
                let parsed_cost = raw_cost.parse::<f64>().unwrap_or(0.0);
                if parsed_cost > 0.0 {
                    total_cost += parsed_cost;
                    has_computable_cost = true;
                } else if let Ok(estimate) = calculate_cost_with_provider(
                    &pid,
                    &model,
                    tin as u64,
                    tout as u64,
                    &price_resolver,
                ) {
                    if let Ok(val) = estimate.cost.parse::<f64>() {
                        total_cost += val;
                        has_computable_cost = true;
                    }
                }
            }
        }

        let (cost_formatted, cost_status) = if total_events == 0 {
            ("$0.00".to_string(), "no_data".to_string())
        } else if has_computable_cost && total_cost > 0.0 {
            (format!("${:.4}", total_cost), "estimated".to_string())
        } else {
            ("Unavailable".to_string(), "missing_pricing".to_string())
        };

        Ok(OverviewResult {
            total_events,
            total_tokens_input,
            total_tokens_output,
            total_cost,
            cost_formatted,
            cost_status,
            provider_count,
            top_providers,
            latest_event_at,
            oldest_event_at,
            high_confidence_count,
            confidence_coverage,
        })
    }
}
