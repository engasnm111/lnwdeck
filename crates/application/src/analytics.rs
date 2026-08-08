use lnwdeck_pricing::{calculator::calculate_cost_with_provider, catalog::PriceResolver};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

pub struct QueryAnalytics;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AnalyticsRow {
    pub id: String,
    pub timestamp: String,
    pub provider_id: String,
    pub model: String,
    pub tokens_input: i64,
    pub tokens_cached: i64,
    pub tokens_cache_write: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub confidence: String,
    pub cost: String,
    pub pricing_status: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AnalyticsResult {
    pub rows: Vec<AnalyticsRow>,
    pub available_providers: Vec<String>,
    pub available_models: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct AnalyticsFilter {
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub confidence: Option<String>,
}

impl QueryAnalytics {
    pub fn execute(
        conn: &Connection,
        filter: AnalyticsFilter,
        resolver: &lnwdeck_pricing::catalog::PriceResolver,
    ) -> Result<AnalyticsResult, rusqlite::Error> {
        let mut where_clauses = Vec::new();
        let mut sql_params: Vec<Value> = Vec::new();

        if let Some(p) = filter.provider_id {
            if !p.is_empty() {
                where_clauses.push("provider_id = ?");
                sql_params.push(Value::Text(p));
            }
        }
        if let Some(m) = filter.model {
            if !m.is_empty() {
                where_clauses.push("model = ?");
                sql_params.push(Value::Text(m));
            }
        }
        if let Some(c) = filter.confidence {
            if !c.is_empty() {
                where_clauses.push("LOWER(confidence) = LOWER(?)");
                sql_params.push(Value::Text(c));
            }
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT id, timestamp, provider_id, model, tokens_input, tokens_cached,
                    tokens_cache_write, tokens_output, tokens_reasoning, confidence, cost
             FROM usage_events
             {}
             ORDER BY timestamp DESC LIMIT 500",
            where_str
        );

        let mut stmt = conn.prepare(&sql)?;
        let raw_rows = stmt
            .query_map(params_from_iter(sql_params), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let rows = raw_rows
            .into_iter()
            .map(
                |(
                    id,
                    timestamp,
                    provider_id,
                    model,
                    tokens_input,
                    tokens_cached,
                    tokens_cache_write,
                    tokens_output,
                    tokens_reasoning,
                    confidence,
                    raw_cost,
                )| {
                    let (cost, pricing_status) = recorded_or_calculated_cost(
                        &provider_id,
                        &model,
                        tokens_input,
                        tokens_output,
                        &raw_cost,
                        resolver,
                    );
                    AnalyticsRow {
                        id,
                        timestamp,
                        provider_id,
                        model,
                        tokens_input,
                        tokens_cached,
                        tokens_cache_write,
                        tokens_output,
                        tokens_reasoning,
                        confidence: normalize_confidence(&confidence),
                        cost,
                        pricing_status,
                    }
                },
            )
            .collect();

        let mut prov_stmt =
            conn.prepare("SELECT DISTINCT provider_id FROM usage_events ORDER BY provider_id")?;
        let available_providers: Vec<String> = prov_stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut model_stmt =
            conn.prepare("SELECT DISTINCT model FROM usage_events ORDER BY model")?;
        let available_models: Vec<String> = model_stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(AnalyticsResult {
            rows,
            available_providers,
            available_models,
        })
    }
}

fn normalize_confidence(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" => "High".to_string(),
        "medium" => "Medium".to_string(),
        "low" => "Low".to_string(),
        _ => "Unknown".to_string(),
    }
}

fn recorded_or_calculated_cost(
    provider_id: &str,
    model: &str,
    tokens_input: i64,
    tokens_output: i64,
    raw_cost: &str,
    resolver: &PriceResolver,
) -> (String, String) {
    if raw_cost
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .is_some()
    {
        return (raw_cost.trim().to_string(), "recorded".to_string());
    }

    match calculate_cost_with_provider(
        provider_id,
        model,
        tokens_input.max(0) as u64,
        tokens_output.max(0) as u64,
        resolver,
    ) {
        Ok(estimate) => {
            let status = match estimate.status {
                lnwdeck_pricing::calculator::PricingStatus::Priced => "priced",
                lnwdeck_pricing::calculator::PricingStatus::Estimated => "estimated",
            };
            (estimate.cost, status.to_string())
        }
        Err(_) => ("".to_string(), "unpriced".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use lnwdeck_pricing::catalog::PriceResolver;
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use serde_json::json;
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("analytics.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    #[test]
    fn analytics_calculates_missing_cost_and_normalizes_confidence() {
        let storage = open_db();
        storage
            .conn
            .execute(
                "INSERT INTO usage_events
                    (id, batch_id, timestamp, provider_id, model, tokens_input,
                     tokens_cached, tokens_cache_write, tokens_output, tokens_reasoning,
                     confidence, data_source, cost)
                 VALUES ('event-1', 'batch-1', ?1, 'openai_codex', 'gpt-5.3-codex',
                         1000, 200, 0, 1000, 200, 'high', 'fixture', '0')",
                [Utc::now().to_rfc3339()],
            )
            .expect("insert analytics fixture");

        let resolver = PriceResolver::new_with_overrides(&json!([]));
        let result = QueryAnalytics::execute(&storage.conn, AnalyticsFilter::default(), &resolver)
            .expect("analytics");

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].cost, "0.015750");
        assert_eq!(result.rows[0].pricing_status, "priced");
        assert_eq!(result.rows[0].confidence, "High");
    }
}
