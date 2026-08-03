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
    pub tokens_output: i64,
    pub confidence: String,
    pub cost: String,
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
                where_clauses.push("confidence = ?");
                sql_params.push(Value::Text(c));
            }
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT id, timestamp, provider_id, model, tokens_input, tokens_output, confidence, cost
             FROM usage_events
             {}
             ORDER BY timestamp DESC LIMIT 500",
            where_str
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(sql_params), |row| {
                Ok(AnalyticsRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    provider_id: row.get(2)?,
                    model: row.get(3)?,
                    tokens_input: row.get(4)?,
                    tokens_output: row.get(5)?,
                    confidence: row.get(6)?,
                    cost: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
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
