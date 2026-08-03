//! Budget read model and progress evaluation.
//!
//! Progress is measured against recorded usage for the budget's period. A
//! budget with no matching usage shows zero spend, and a budget whose spend
//! cannot be priced says so instead of reporting a comfortable number.

use chrono::{Duration, Utc};
use lnwdeck_pricing::{calculator::calculate_cost_with_provider, catalog::PriceResolver};
use lnwdeck_storage::repositories::{BudgetPeriod, BudgetRepository, BudgetRow, BudgetScope};
use rusqlite::Connection;
use serde::Serialize;

/// A budget together with what has actually been consumed in its period.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetProgress {
    pub budget: BudgetRow,
    /// Start of the current period, RFC3339.
    pub period_start: String,
    pub request_count: i64,
    pub tokens_used: i64,
    /// Priced spend as a decimal string.
    pub cost_used: String,
    /// Tokens whose model has no catalog entry, so they carry no cost.
    pub unpriced_tokens: i64,
    /// Percentage of the cost limit consumed, `None` when no cost limit is set.
    pub cost_percent: Option<f64>,
    /// Percentage of the token limit consumed, `None` when no token limit is
    /// set.
    pub token_percent: Option<f64>,
    /// "under", "warning" or "exceeded", derived from the two percentages.
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetOverview {
    pub generated_at: chrono::DateTime<Utc>,
    pub budgets: Vec<BudgetProgress>,
}

pub struct QueryBudgets;

impl QueryBudgets {
    pub fn execute(
        conn: &Connection,
        resolver: &PriceResolver,
    ) -> Result<BudgetOverview, rusqlite::Error> {
        let budgets = BudgetRepository::new(conn).list()?;
        let mut progress = Vec::with_capacity(budgets.len());
        for budget in budgets {
            progress.push(evaluate(conn, budget, resolver)?);
        }
        Ok(BudgetOverview {
            generated_at: Utc::now(),
            budgets: progress,
        })
    }
}

/// Computes one budget's progress from recorded events.
pub fn evaluate(
    conn: &Connection,
    budget: BudgetRow,
    resolver: &PriceResolver,
) -> Result<BudgetProgress, rusqlite::Error> {
    let period_start = Utc::now() - Duration::days(budget.period.days());
    let period_start_text = period_start.to_rfc3339();
    let provider_filter = match &budget.scope {
        BudgetScope::Global => String::new(),
        BudgetScope::Provider(id) => id.clone(),
    };

    let mut stmt = conn.prepare(
        "SELECT provider_id, model, COUNT(*),
                COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0)
         FROM usage_events
         WHERE timestamp >= ?1 AND (?2 = '' OR provider_id = ?2)
         GROUP BY provider_id, model",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![period_start_text, provider_filter],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let mut request_count = 0i64;
    let mut tokens_used = 0i64;
    let mut cost_used = 0.0f64;
    let mut unpriced_tokens = 0i64;

    for (provider_id, model, requests, input, output) in rows {
        request_count += requests;
        tokens_used += input + output;
        match calculate_cost_with_provider(
            &provider_id,
            &model,
            input.max(0) as u64,
            output.max(0) as u64,
            resolver,
        ) {
            Ok(value) => cost_used += value.parse::<f64>().unwrap_or(0.0),
            Err(_) => unpriced_tokens += input + output,
        }
    }

    let cost_percent = budget
        .cost_limit
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|limit| *limit > 0.0)
        .map(|limit| cost_used / limit * 100.0);
    let token_percent = budget
        .token_limit
        .filter(|limit| *limit > 0)
        .map(|limit| tokens_used as f64 / limit as f64 * 100.0);

    let worst = [cost_percent, token_percent]
        .into_iter()
        .flatten()
        .fold(None::<f64>, |acc, value| {
            Some(acc.map_or(value, |current: f64| current.max(value)))
        });
    let state = match worst {
        Some(percent) if percent >= 100.0 => "exceeded",
        Some(percent) if percent >= budget.warn_percent as f64 => "warning",
        Some(_) => "under",
        // Without a usable limit there is nothing to be under or over.
        None => "unknown",
    }
    .to_string();

    Ok(BudgetProgress {
        budget,
        period_start: period_start_text,
        request_count,
        tokens_used,
        cost_used: format!("{cost_used:.6}"),
        unpriced_tokens,
        cost_percent,
        token_percent,
        state,
    })
}

/// Period label used by the UI.
pub fn period_label(period: BudgetPeriod) -> &'static str {
    match period {
        BudgetPeriod::Daily => "daily",
        BudgetPeriod::Weekly => "weekly",
        BudgetPeriod::Monthly => "monthly",
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

    fn resolver() -> PriceResolver {
        PriceResolver::new_with_overrides(&json!([
            {
                "provider": "anthropic",
                "model": "claude-test",
                "input_per_1k": "1.0",
                "output_per_1k": "1.0"
            }
        ]))
    }

    fn insert_event(
        storage: &Storage,
        id: &str,
        provider: &str,
        model: &str,
        tokens: i64,
        hours_ago: i64,
    ) {
        let timestamp = (Utc::now() - Duration::hours(hours_ago)).to_rfc3339();
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES (?1, 'b', ?2, ?3, ?4, ?5, 0, 'Medium', 'local', '')",
                rusqlite::params![id, timestamp, provider, model, tokens],
            )
            .expect("insert");
    }

    fn budget(scope: BudgetScope, cost: &str, tokens: Option<u64>) -> BudgetRow {
        BudgetRow {
            id: 0,
            scope,
            period: BudgetPeriod::Monthly,
            cost_limit: cost.to_string(),
            token_limit: tokens,
            warn_percent: 80,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn no_budgets_produce_an_empty_overview() {
        let storage = open_db();
        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        assert!(
            overview.budgets.is_empty(),
            "no budget may be invented for the user"
        );
    }

    #[test]
    fn a_budget_without_usage_reports_zero_not_a_healthy_guess() {
        let storage = open_db();
        BudgetRepository::new(&storage.conn)
            .upsert(&budget(BudgetScope::Global, "10", None))
            .expect("insert");

        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        let progress = &overview.budgets[0];
        assert_eq!(progress.request_count, 0);
        assert_eq!(progress.tokens_used, 0);
        assert_eq!(progress.cost_used, "0.000000");
        assert_eq!(progress.cost_percent, Some(0.0));
        assert_eq!(progress.state, "under");
    }

    #[test]
    fn cost_progress_uses_priced_events_only() {
        let storage = open_db();
        // 2000 tokens at 1.0 per 1k = 2.00 priced.
        insert_event(&storage, "e1", "anthropic_claude", "claude-test", 2000, 1);
        // Unpriced model: counted in tokens, never given a cost.
        insert_event(&storage, "e2", "mystery", "mystery-model", 500, 1);
        BudgetRepository::new(&storage.conn)
            .upsert(&budget(BudgetScope::Global, "10", None))
            .expect("insert");

        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        let progress = &overview.budgets[0];
        assert_eq!(progress.tokens_used, 2500);
        assert_eq!(progress.cost_used, "2.000000");
        assert_eq!(progress.unpriced_tokens, 500);
        assert_eq!(progress.cost_percent, Some(20.0));
        assert_eq!(progress.state, "under");
    }

    #[test]
    fn warning_and_exceeded_states_come_from_the_threshold() {
        let storage = open_db();
        insert_event(&storage, "e1", "anthropic_claude", "claude-test", 9000, 1);
        let repo = BudgetRepository::new(&storage.conn);
        repo.upsert(&budget(BudgetScope::Global, "10", None))
            .expect("insert");

        let warning = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        assert_eq!(warning.budgets[0].cost_percent, Some(90.0));
        assert_eq!(warning.budgets[0].state, "warning");

        insert_event(&storage, "e2", "anthropic_claude", "claude-test", 2000, 1);
        let exceeded = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        assert_eq!(exceeded.budgets[0].state, "exceeded");
        assert!(exceeded.budgets[0].cost_percent.unwrap() > 100.0);
    }

    #[test]
    fn token_limits_are_evaluated_independently() {
        let storage = open_db();
        insert_event(&storage, "e1", "mystery", "mystery-model", 900, 1);
        BudgetRepository::new(&storage.conn)
            .upsert(&budget(BudgetScope::Global, "", Some(1000)))
            .expect("insert");

        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        let progress = &overview.budgets[0];
        assert_eq!(progress.token_percent, Some(90.0));
        assert_eq!(progress.cost_percent, None, "no cost limit was configured");
        assert_eq!(progress.state, "warning");
    }

    #[test]
    fn provider_budgets_only_count_their_provider() {
        let storage = open_db();
        insert_event(&storage, "e1", "anthropic_claude", "claude-test", 5000, 1);
        insert_event(&storage, "e2", "anthropic_claude", "claude-test", 3000, 2);
        insert_event(&storage, "e3", "opencode", "glm-5", 9000, 1);
        BudgetRepository::new(&storage.conn)
            .upsert(&budget(
                BudgetScope::Provider("anthropic_claude".to_string()),
                "10",
                None,
            ))
            .expect("insert");

        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        assert_eq!(
            overview.budgets[0].tokens_used, 8000,
            "another provider's usage must not count towards this budget"
        );
        assert_eq!(overview.budgets[0].cost_used, "8.000000");
        assert_eq!(overview.budgets[0].unpriced_tokens, 0);
    }

    #[test]
    fn usage_a_provider_cannot_be_priced_for_is_reported_not_charged() {
        let storage = open_db();
        // The catalog only prices claude-test for Anthropic; the same model
        // recorded under another provider must not inherit that rate.
        insert_event(&storage, "e1", "opencode", "claude-test", 5000, 1);
        BudgetRepository::new(&storage.conn)
            .upsert(&budget(
                BudgetScope::Provider("opencode".to_string()),
                "10",
                None,
            ))
            .expect("insert");

        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        assert_eq!(overview.budgets[0].tokens_used, 5000);
        assert_eq!(overview.budgets[0].cost_used, "0.000000");
        assert_eq!(overview.budgets[0].unpriced_tokens, 5000);
    }

    #[test]
    fn events_outside_the_period_are_ignored() {
        let storage = open_db();
        insert_event(
            &storage,
            "old",
            "anthropic_claude",
            "claude-test",
            9000,
            24 * 40,
        );
        BudgetRepository::new(&storage.conn)
            .upsert(&budget(BudgetScope::Global, "10", None))
            .expect("insert");

        let overview = QueryBudgets::execute(&storage.conn, &resolver()).expect("overview");
        assert_eq!(
            overview.budgets[0].tokens_used, 0,
            "a 40-day-old event is outside a monthly budget"
        );
    }
}
