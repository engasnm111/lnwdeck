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
            "SELECT COALESCE(SUM(tokens_input + tokens_cached + tokens_cache_write), 0)
             FROM usage_events",
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

        // Calculate Cost Pipeline. Aggregated per (provider, model) group in
        // SQL so a large history never streams every row into Rust: recorded
        // costs are summed directly, and the tokens of events without a
        // recorded cost get one estimate for the whole group (pricing is
        // linear in tokens, so this equals the old per-event estimates).
        let price_resolver = PriceResolver::new_with_overrides(&json!([]));
        let mut total_cost: f64 = 0.0;
        let mut has_computable_cost = false;

        if total_events > 0 {
            let mut group_stmt = conn.prepare(
                "SELECT provider_id, model,
                        COALESCE(SUM(CASE WHEN CAST(cost AS REAL) > 0 THEN CAST(cost AS REAL) ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN CAST(cost AS REAL) > 0 THEN 0 ELSE tokens_input END), 0),
                        COALESCE(SUM(CASE WHEN CAST(cost AS REAL) > 0 THEN 0 ELSE tokens_output END), 0)
                 FROM usage_events
                 GROUP BY provider_id, model",
            )?;
            let groups = group_stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?;

            for group in groups {
                let (pid, model, recorded, unpriced_input, unpriced_output) = group?;
                if recorded > 0.0 {
                    total_cost += recorded;
                    has_computable_cost = true;
                }
                if unpriced_input > 0 || unpriced_output > 0 {
                    if let Ok(estimate) = calculate_cost_with_provider(
                        &pid,
                        &model,
                        unpriced_input as u64,
                        unpriced_output as u64,
                        &price_resolver,
                    ) {
                        if let Ok(val) = estimate.cost.parse::<f64>() {
                            total_cost += val;
                            has_computable_cost = true;
                        }
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
