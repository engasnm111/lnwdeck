//! Cost read model.
//!
//! Costs are computed from recorded events and the pricing catalog. A model
//! without a catalog entry is reported as unpriced with its token totals; it is
//! never charged at another model's rate and never silently counted as zero.

use crate::usage_history::HistoryWindow;
use chrono::{DateTime, Utc};
use lnwdeck_pricing::{calculator::calculate_cost_with_provider, catalog::PriceResolver};
use rusqlite::Connection;
use serde::Serialize;

/// Cost of one provider/model pair over the window.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelCostRow {
    pub provider_id: String,
    pub model: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    /// Cost as a decimal string. Always present: unknown models get a
    /// labeled estimate, never a blank cell.
    pub cost: String,
    /// How the cost was produced: `priced`, `estimated` or `no catalog entry`.
    pub pricing_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CostBreakdown {
    pub window: HistoryWindow,
    pub generated_at: DateTime<Utc>,
    pub rows: Vec<ModelCostRow>,
    /// Sum over priced rows only.
    pub priced_total: String,
    pub priced_rows: usize,
    /// Rows charged at the generic estimate rate.
    pub estimated_rows: usize,
    /// Rows with no pricing information at all.
    pub unpriced_rows: usize,
    /// Tokens that could not be priced at all.
    pub unpriced_tokens: i64,
    /// Providers with recorded events in the window, for the filter dropdown.
    /// Always the full window set, regardless of the active filter.
    pub providers: Vec<String>,
}

pub struct QueryCosts;

impl QueryCosts {
    pub fn execute(
        conn: &Connection,
        window: HistoryWindow,
        resolver: &PriceResolver,
    ) -> Result<CostBreakdown, rusqlite::Error> {
        Self::execute_with_provider(conn, window, None, resolver)
    }

    /// Reads costs for a window, optionally narrowed to one provider. The
    /// available-provider list always covers the whole window so the filter
    /// dropdown does not shrink to the currently selected provider.
    pub fn execute_with_provider(
        conn: &Connection,
        window: HistoryWindow,
        provider_id: Option<&str>,
        resolver: &PriceResolver,
    ) -> Result<CostBreakdown, rusqlite::Error> {
        let now = Utc::now();
        let since = window
            .since(now)
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "0000-01-01T00:00:00+00:00".to_string());
        let filter = provider_id.unwrap_or("").to_string();
        // An empty provider filter matches every provider.
        let params = rusqlite::params![since, filter];

        let mut stmt = conn.prepare(
            "SELECT provider_id, model, COUNT(*),
                    COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0)
             FROM usage_events
             WHERE timestamp >= ?1 AND (?2 = '' OR provider_id = ?2)
             GROUP BY provider_id, model
             ORDER BY SUM(tokens_input + tokens_output) DESC, model",
        )?;

        let raw = stmt
            .query_map(params, |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut rows = Vec::with_capacity(raw.len());
        let mut priced_total = 0.0f64;
        let mut priced_rows = 0usize;
        let mut estimated_rows = 0usize;
        let mut unpriced_rows = 0usize;
        let mut unpriced_tokens = 0i64;

        for (provider_id, model, request_count, tokens_input, tokens_output) in raw {
            let calculated = calculate_cost_with_provider(
                &provider_id,
                &model,
                tokens_input.max(0) as u64,
                tokens_output.max(0) as u64,
                resolver,
            );
            let (cost, pricing_status) = match calculated {
                Ok(estimate) => match estimate.status {
                    lnwdeck_pricing::PricingStatus::Priced => {
                        priced_rows += 1;
                        priced_total += estimate.cost.parse::<f64>().unwrap_or(0.0);
                        (estimate.cost, "priced".to_string())
                    }
                    lnwdeck_pricing::PricingStatus::Estimated => {
                        estimated_rows += 1;
                        (estimate.cost, "estimated".to_string())
                    }
                },
                Err(_) => {
                    unpriced_rows += 1;
                    unpriced_tokens += tokens_input + tokens_output;
                    (String::new(), "no catalog entry".to_string())
                }
            };
            rows.push(ModelCostRow {
                provider_id,
                model,
                request_count,
                tokens_input,
                tokens_output,
                cost,
                pricing_status,
            });
        }

        let mut provider_stmt = conn.prepare(
            "SELECT DISTINCT provider_id FROM usage_events
                          WHERE timestamp >= ?1 ORDER BY provider_id",
        )?;
        let providers = provider_stmt
            .query_map([since], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(CostBreakdown {
            window,
            generated_at: now,
            rows,
            priced_total: format!("{priced_total:.6}"),
            priced_rows,
            estimated_rows,
            unpriced_rows,
            unpriced_tokens,
            providers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use serde_json::json;
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    fn insert_event(storage: &Storage, id: &str, provider: &str, model: &str, tokens: i64) {
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES (?1, 'b', ?2, ?3, ?4, ?5, 0, 'Medium', 'local', '')",
                rusqlite::params![id, Utc::now().to_rfc3339(), provider, model, tokens],
            )
            .expect("insert");
    }

    fn resolver() -> PriceResolver {
        PriceResolver::new_with_overrides(&json!([
            {
                "provider": "anthropic",
                "model": "claude-test",
                "input_per_1k": "0.003",
                "output_per_1k": "0.015"
            }
        ]))
    }

    #[test]
    fn empty_database_produces_an_empty_breakdown() {
        let storage = open_db();
        let breakdown =
            QueryCosts::execute(&storage.conn, HistoryWindow::Last30d, &resolver()).expect("costs");
        assert!(breakdown.rows.is_empty());
        assert_eq!(breakdown.priced_rows, 0);
        assert_eq!(breakdown.estimated_rows, 0);
        assert_eq!(breakdown.unpriced_rows, 0);
        assert_eq!(breakdown.priced_total, "0.000000");
    }

    #[test]
    fn priced_models_are_calculated_from_the_catalog() {
        let storage = open_db();
        insert_event(&storage, "e1", "anthropic_claude", "claude-test", 1000);

        let breakdown =
            QueryCosts::execute(&storage.conn, HistoryWindow::Last30d, &resolver()).expect("costs");
        assert_eq!(breakdown.rows.len(), 1);
        assert_eq!(breakdown.rows[0].cost, "0.003000");
        assert_eq!(breakdown.rows[0].pricing_status, "priced");
        assert_eq!(breakdown.priced_rows, 1);
        assert_eq!(breakdown.estimated_rows, 0);
        assert_eq!(breakdown.unpriced_rows, 0);
        assert_eq!(breakdown.priced_total, "0.003000");
    }

    #[test]
    fn unknown_models_get_a_labeled_estimate_not_a_blank_cell() {
        let storage = open_db();
        insert_event(&storage, "e1", "mystery_provider", "mystery-model", 5000);

        let breakdown =
            QueryCosts::execute(&storage.conn, HistoryWindow::Last30d, &resolver()).expect("costs");
        assert_eq!(breakdown.rows.len(), 1);
        assert_eq!(
            breakdown.rows[0].pricing_status, "estimated",
            "unknown models are estimated, never silently zero or blank"
        );
        assert_eq!(
            breakdown.rows[0].cost, "0.012500",
            "generic estimate applies: 2.50 per 1M input tokens"
        );
        assert_eq!(breakdown.estimated_rows, 1);
        assert_eq!(breakdown.unpriced_rows, 0);
        assert_eq!(breakdown.priced_total, "0.000000");
    }

    #[test]
    fn mixed_coverage_is_reported_separately() {
        let storage = open_db();
        insert_event(&storage, "e1", "anthropic_claude", "claude-test", 2000);
        insert_event(&storage, "e2", "mystery_provider", "mystery-model", 100);

        let breakdown =
            QueryCosts::execute(&storage.conn, HistoryWindow::Last30d, &resolver()).expect("costs");
        assert_eq!(breakdown.rows.len(), 2);
        assert_eq!(breakdown.priced_rows, 1);
        assert_eq!(breakdown.estimated_rows, 1);
        assert_eq!(breakdown.unpriced_rows, 0);
        assert_eq!(breakdown.priced_total, "0.006000");
        assert_eq!(breakdown.unpriced_tokens, 0);
    }

    #[test]
    fn provider_filter_narrows_rows_but_keeps_all_providers_available() {
        let storage = open_db();
        insert_event(&storage, "e1", "anthropic_claude", "claude-test", 2000);
        insert_event(&storage, "e2", "opencode", "glm-5", 500);

        let all =
            QueryCosts::execute(&storage.conn, HistoryWindow::Last30d, &resolver()).expect("costs");
        assert_eq!(all.rows.len(), 2, "no filter returns every row");
        assert_eq!(all.providers, vec!["anthropic_claude", "opencode"]);

        let filtered = QueryCosts::execute_with_provider(
            &storage.conn,
            HistoryWindow::Last30d,
            Some("opencode"),
            &resolver(),
        )
        .expect("costs");
        assert_eq!(
            filtered.rows.len(),
            1,
            "the filter keeps only matching rows"
        );
        assert_eq!(filtered.rows[0].provider_id, "opencode");
        assert_eq!(
            filtered.providers,
            vec!["anthropic_claude", "opencode"],
            "the dropdown must not shrink to the selected provider"
        );
        assert_eq!(filtered.priced_total, "0.000500");

        let none = QueryCosts::execute_with_provider(
            &storage.conn,
            HistoryWindow::Last30d,
            Some("unknown_provider"),
            &resolver(),
        )
        .expect("costs");
        assert!(none.rows.is_empty(), "an unknown provider yields no rows");
        assert!(
            !none.providers.is_empty(),
            "available providers still describe the window"
        );
    }
}
