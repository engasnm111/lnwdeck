//! Usage-history read models.
//!
//! Usage history and remaining quota are two separate channels and stay
//! separate here: everything in this module is derived from recorded
//! `usage_events` only. It never reads a quota report, and the quota read model
//! never derives history from these rows.

use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use serde::Serialize;

/// Rolling window a history query can be scoped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryWindow {
    Last24h,
    Last7d,
    Last30d,
    All,
}

impl HistoryWindow {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "last_24h" | "24h" => Some(Self::Last24h),
            "last_7d" | "7d" => Some(Self::Last7d),
            "last_30d" | "30d" => Some(Self::Last30d),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    /// Inclusive lower bound for the window, or `None` for the full history.
    pub fn since(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Last24h => Some(now - Duration::hours(24)),
            Self::Last7d => Some(now - Duration::days(7)),
            Self::Last30d => Some(now - Duration::days(30)),
            Self::All => None,
        }
    }
}

/// One model's recorded usage.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelUsageRow {
    pub model: String,
    pub provider_id: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    /// Share of the window's total tokens, 0..=100. `None` when the window
    /// recorded no tokens at all, so a share is not invented.
    pub token_share_percent: Option<f64>,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
}

/// One day of recorded usage, used for the trend chart.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DailyUsageRow {
    pub day: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
}

/// Complete usage-history read model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageHistory {
    pub window: HistoryWindow,
    pub generated_at: DateTime<Utc>,
    /// Lower bound applied to the query, `None` for the full history.
    pub since: Option<DateTime<Utc>>,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub models: Vec<ModelUsageRow>,
    pub daily: Vec<DailyUsageRow>,
    pub providers: Vec<String>,
}

pub struct QueryUsageHistory;

impl QueryUsageHistory {
    /// Reads recorded usage for a window, optionally narrowed to one provider.
    pub fn execute(
        conn: &Connection,
        window: HistoryWindow,
        provider_id: Option<&str>,
    ) -> Result<UsageHistory, rusqlite::Error> {
        let now = Utc::now();
        let since = window.since(now);
        let since_text = since
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "0000-01-01T00:00:00+00:00".to_string());
        let provider_filter = provider_id.unwrap_or("").to_string();
        // An empty provider filter matches every provider.
        let params = rusqlite::params![since_text, provider_filter];

        let (request_count, tokens_input, tokens_output): (i64, i64, i64) = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(tokens_input), 0),
                    COALESCE(SUM(tokens_output), 0)
             FROM usage_events
             WHERE timestamp >= ?1 AND (?2 = '' OR provider_id = ?2)",
            params,
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let total_tokens = tokens_input + tokens_output;

        let mut model_stmt = conn.prepare(
            "SELECT model, provider_id, COUNT(*),
                    COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0),
                    MIN(timestamp), MAX(timestamp)
             FROM usage_events
             WHERE timestamp >= ?1 AND (?2 = '' OR provider_id = ?2)
             GROUP BY model, provider_id
             ORDER BY SUM(tokens_input + tokens_output) DESC, model",
        )?;
        let models = model_stmt
            .query_map(params, |row| {
                let input: i64 = row.get(3)?;
                let output: i64 = row.get(4)?;
                Ok(ModelUsageRow {
                    model: row.get(0)?,
                    provider_id: row.get(1)?,
                    request_count: row.get(2)?,
                    tokens_input: input,
                    tokens_output: output,
                    token_share_percent: if total_tokens > 0 {
                        Some((input + output) as f64 / total_tokens as f64 * 100.0)
                    } else {
                        None
                    },
                    first_seen_at: row.get(5)?,
                    last_seen_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut daily_stmt = conn.prepare(
            "SELECT substr(timestamp, 1, 10) AS day, COUNT(*),
                    COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0)
             FROM usage_events
             WHERE timestamp >= ?1 AND (?2 = '' OR provider_id = ?2)
             GROUP BY day ORDER BY day",
        )?;
        let daily = daily_stmt
            .query_map(params, |row| {
                Ok(DailyUsageRow {
                    day: row.get(0)?,
                    request_count: row.get(1)?,
                    tokens_input: row.get(2)?,
                    tokens_output: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut provider_stmt =
            conn.prepare("SELECT DISTINCT provider_id FROM usage_events ORDER BY provider_id")?;
        let providers = provider_stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(UsageHistory {
            window,
            generated_at: now,
            since,
            request_count,
            tokens_input,
            tokens_output,
            models,
            daily,
            providers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    fn insert_event(
        storage: &Storage,
        id: &str,
        provider: &str,
        model: &str,
        input: i64,
        output: i64,
        hours_ago: i64,
    ) {
        let timestamp = (Utc::now() - Duration::hours(hours_ago)).to_rfc3339();
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES (?1, 'b', ?2, ?3, ?4, ?5, ?6, 'Medium', 'local', '0.01')",
                rusqlite::params![id, timestamp, provider, model, input, output],
            )
            .expect("insert event");
    }

    #[test]
    fn empty_history_reports_zero_and_no_share() {
        let storage = open_db();
        let history = QueryUsageHistory::execute(&storage.conn, HistoryWindow::Last7d, None)
            .expect("history");
        assert_eq!(history.request_count, 0);
        assert_eq!(history.tokens_input, 0);
        assert!(history.models.is_empty());
        assert!(history.daily.is_empty());
        assert!(history.providers.is_empty());
        assert!(history.since.is_some());
    }

    #[test]
    fn aggregates_per_model_with_real_shares() {
        let storage = open_db();
        insert_event(&storage, "e1", "opencode", "glm-5", 300, 100, 1);
        insert_event(&storage, "e2", "opencode", "glm-5", 100, 0, 2);
        insert_event(&storage, "e3", "anthropic_claude", "claude-x", 400, 100, 3);

        let history = QueryUsageHistory::execute(&storage.conn, HistoryWindow::Last24h, None)
            .expect("history");
        assert_eq!(history.request_count, 3);
        assert_eq!(history.tokens_input, 800);
        assert_eq!(history.tokens_output, 200);
        assert_eq!(history.models.len(), 2);

        let claude = history
            .models
            .iter()
            .find(|row| row.model == "claude-x")
            .expect("claude row");
        assert_eq!(claude.request_count, 1);
        assert_eq!(claude.tokens_input, 400);
        assert_eq!(claude.token_share_percent, Some(50.0));

        let glm = history
            .models
            .iter()
            .find(|row| row.model == "glm-5")
            .expect("glm row");
        assert_eq!(glm.request_count, 2);
        assert_eq!(glm.token_share_percent, Some(50.0));
        assert_eq!(history.providers.len(), 2);
    }

    #[test]
    fn window_bounds_are_applied() {
        let storage = open_db();
        insert_event(&storage, "recent", "opencode", "m", 10, 5, 1);
        insert_event(&storage, "old", "opencode", "m", 1000, 500, 24 * 10);

        let day =
            QueryUsageHistory::execute(&storage.conn, HistoryWindow::Last24h, None).expect("24h");
        assert_eq!(day.request_count, 1);
        assert_eq!(day.tokens_input, 10);

        let month =
            QueryUsageHistory::execute(&storage.conn, HistoryWindow::Last30d, None).expect("30d");
        assert_eq!(month.request_count, 2);

        let all = QueryUsageHistory::execute(&storage.conn, HistoryWindow::All, None).expect("all");
        assert_eq!(all.request_count, 2);
        assert!(all.since.is_none());
    }

    #[test]
    fn provider_filter_narrows_the_rows_but_not_the_provider_list() {
        let storage = open_db();
        insert_event(&storage, "e1", "opencode", "m1", 10, 5, 1);
        insert_event(&storage, "e2", "anthropic_claude", "m2", 20, 5, 1);

        let filtered =
            QueryUsageHistory::execute(&storage.conn, HistoryWindow::Last7d, Some("opencode"))
                .expect("history");
        assert_eq!(filtered.request_count, 1);
        assert_eq!(filtered.models.len(), 1);
        assert_eq!(filtered.models[0].provider_id, "opencode");
        assert_eq!(
            filtered.providers.len(),
            2,
            "the filter list still offers every provider"
        );
    }

    #[test]
    fn daily_series_groups_by_calendar_day() {
        let storage = open_db();
        insert_event(&storage, "e1", "opencode", "m", 10, 5, 1);
        insert_event(&storage, "e2", "opencode", "m", 20, 5, 25);

        let history = QueryUsageHistory::execute(&storage.conn, HistoryWindow::Last7d, None)
            .expect("history");
        assert!(
            history.daily.len() >= 2,
            "two events a day apart produce two buckets: {:?}",
            history.daily
        );
        let total: i64 = history.daily.iter().map(|row| row.request_count).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn window_parsing_accepts_the_documented_values() {
        assert_eq!(HistoryWindow::parse("24h"), Some(HistoryWindow::Last24h));
        assert_eq!(HistoryWindow::parse("last_7d"), Some(HistoryWindow::Last7d));
        assert_eq!(HistoryWindow::parse("30d"), Some(HistoryWindow::Last30d));
        assert_eq!(HistoryWindow::parse("all"), Some(HistoryWindow::All));
        assert_eq!(HistoryWindow::parse("fortnight"), None);
    }
}
