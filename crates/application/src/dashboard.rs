//! TokenTracker-style dashboard read model.
//!
//! The query keeps the summary, provider breakdown, trend, heatmap and
//! session table on one consistent time/provider filter. It reads only the
//! privacy-safe `usage_events` projection; prompts, responses and raw paths
//! never enter this module.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use lnwdeck_storage::repositories::SessionRepository;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardRange {
    Day,
    Week,
    Month,
    Year,
    Total,
    Custom,
}

impl DashboardRange {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            "total" => Some(Self::Total),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardQuery {
    pub range: DashboardRange,
    pub start: Option<String>,
    pub end: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardProviderUsage {
    pub provider_id: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardTrendPoint {
    pub bucket: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardHeatmapCell {
    pub day: String,
    pub request_count: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSessionProvider {
    pub provider_id: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSession {
    pub session_hash: String,
    pub display_name: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub total_tokens: i64,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub providers: Vec<DashboardSessionProvider>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageDashboard {
    pub range: DashboardRange,
    pub generated_at: DateTime<Utc>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub duration_days: i64,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub total_tokens: i64,
    pub provider_count: i64,
    pub session_count: i64,
    pub providers: Vec<DashboardProviderUsage>,
    pub trend: Vec<DashboardTrendPoint>,
    pub heatmap: Vec<DashboardHeatmapCell>,
    pub sessions: Vec<DashboardSession>,
}

struct Bounds {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

fn local_midnight(date: NaiveDate) -> Result<DateTime<Utc>, String> {
    Local
        .from_local_datetime(
            &date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| "invalid local midnight".to_string())?,
        )
        .single()
        .map(|value| value.with_timezone(&Utc))
        .ok_or_else(|| "local midnight is ambiguous".to_string())
}

fn next_month(date: NaiveDate) -> Result<NaiveDate, String> {
    if date.month() == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
    }
    .ok_or_else(|| "invalid month boundary".to_string())
}

fn bounds_for(query: &DashboardQuery, now: DateTime<Local>) -> Result<Bounds, String> {
    let today = now.date_naive();
    let (start_date, end_date) = match query.range {
        DashboardRange::Day => (Some(today), today.succ_opt()),
        DashboardRange::Week => {
            let monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
            (Some(monday), monday.checked_add_days(chrono::Days::new(7)))
        }
        DashboardRange::Month => {
            let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                .ok_or_else(|| "invalid month boundary".to_string())?;
            (Some(first), Some(next_month(first)?))
        }
        DashboardRange::Year => {
            let first = NaiveDate::from_ymd_opt(today.year(), 1, 1)
                .ok_or_else(|| "invalid year boundary".to_string())?;
            (Some(first), NaiveDate::from_ymd_opt(today.year() + 1, 1, 1))
        }
        DashboardRange::Total => (None, None),
        DashboardRange::Custom => {
            let start = query
                .start
                .as_deref()
                .ok_or_else(|| "custom range needs a start date".to_string())?;
            let end = query
                .end
                .as_deref()
                .ok_or_else(|| "custom range needs an end date".to_string())?;
            let start = NaiveDate::parse_from_str(start, "%Y-%m-%d")
                .map_err(|_| "custom range start must be YYYY-MM-DD".to_string())?;
            let end = NaiveDate::parse_from_str(end, "%Y-%m-%d")
                .map_err(|_| "custom range end must be YYYY-MM-DD".to_string())?;
            if end < start {
                return Err("custom range end must not be before start".to_string());
            }
            (Some(start), end.succ_opt())
        }
    };

    let start = start_date.map(local_midnight).transpose()?;
    let end = end_date.map(local_midnight).transpose()?;
    Ok(Bounds { start, end })
}

fn bound_text(value: Option<DateTime<Utc>>, fallback: &str) -> String {
    value
        .map(|date| date.to_rfc3339())
        .unwrap_or_else(|| fallback.to_string())
}

fn display_date(value: Option<DateTime<Utc>>) -> Option<String> {
    value.map(|date| date.to_rfc3339())
}

fn span_days(bounds: &Bounds, trend: &[DashboardTrendPoint]) -> i64 {
    if let (Some(start), Some(end)) = (bounds.start, bounds.end) {
        return (end - start).num_days().max(1);
    }
    let (Some(first), Some(last)) = (trend.first(), trend.last()) else {
        return 0;
    };
    let Ok(first) = NaiveDate::parse_from_str(&first.bucket, "%Y-%m-%d") else {
        return trend.len() as i64;
    };
    let Ok(last) = NaiveDate::parse_from_str(&last.bucket, "%Y-%m-%d") else {
        return trend.len() as i64;
    };
    (last - first).num_days().abs() + 1
}

pub struct QueryDashboard;

impl QueryDashboard {
    pub fn execute(conn: &Connection, query: DashboardQuery) -> Result<UsageDashboard, String> {
        let generated_at = Utc::now();
        let bounds = bounds_for(&query, generated_at.with_timezone(&Local))?;
        let start = bound_text(bounds.start, "0000-01-01T00:00:00Z");
        let end = bound_text(bounds.end, "9999-12-31T23:59:59Z");
        let provider = query.provider_id.unwrap_or_default();
        let params = rusqlite::params![start, end, provider];

        let (request_count, tokens_input, tokens_output): (i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(tokens_input), 0),
                        COALESCE(SUM(tokens_output), 0)
                 FROM usage_events
                 WHERE timestamp >= ?1 AND timestamp < ?2
                   AND (?3 = '' OR provider_id = ?3)",
                params,
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| format!("dashboard summary: {error}"))?;

        let mut provider_stmt = conn
            .prepare(
                "SELECT provider_id, COUNT(*), COALESCE(SUM(tokens_input), 0),
                        COALESCE(SUM(tokens_output), 0)
                 FROM usage_events
                 WHERE timestamp >= ?1 AND timestamp < ?2
                   AND (?3 = '' OR provider_id = ?3)
                 GROUP BY provider_id
                 ORDER BY SUM(tokens_input + tokens_output) DESC, provider_id",
            )
            .map_err(|error| format!("dashboard providers: {error}"))?;
        let providers = provider_stmt
            .query_map(params, |row| {
                let input: i64 = row.get(2)?;
                let output: i64 = row.get(3)?;
                Ok(DashboardProviderUsage {
                    provider_id: row.get(0)?,
                    request_count: row.get(1)?,
                    tokens_input: input,
                    tokens_output: output,
                    total_tokens: input + output,
                })
            })
            .map_err(|error| format!("dashboard providers: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("dashboard providers: {error}"))?;

        let mut trend_stmt = conn
            .prepare(
                "SELECT date(timestamp, 'localtime') AS bucket, COUNT(*),
                        COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0)
                 FROM usage_events
                 WHERE timestamp >= ?1 AND timestamp < ?2
                   AND (?3 = '' OR provider_id = ?3)
                 GROUP BY bucket ORDER BY bucket",
            )
            .map_err(|error| format!("dashboard trend: {error}"))?;
        let trend = trend_stmt
            .query_map(params, |row| {
                let input: i64 = row.get(2)?;
                let output: i64 = row.get(3)?;
                Ok(DashboardTrendPoint {
                    bucket: row.get(0)?,
                    request_count: row.get(1)?,
                    tokens_input: input,
                    tokens_output: output,
                    total_tokens: input + output,
                })
            })
            .map_err(|error| format!("dashboard trend: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("dashboard trend: {error}"))?;

        let heatmap = trend
            .iter()
            .map(|point| DashboardHeatmapCell {
                day: point.bucket.clone(),
                request_count: point.request_count,
                total_tokens: point.total_tokens,
            })
            .collect::<Vec<_>>();

        let mut session_stmt = conn
            .prepare(
                "SELECT session_hash, provider_id, COUNT(*),
                        COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0),
                        MIN(timestamp), MAX(timestamp)
                 FROM usage_events
                 WHERE timestamp >= ?1 AND timestamp < ?2
                   AND (?3 = '' OR provider_id = ?3)
                 GROUP BY session_hash, provider_id",
            )
            .map_err(|error| format!("dashboard sessions: {error}"))?;
        let session_rows = session_stmt
            .query_map(params, |row| {
                let input: i64 = row.get(3)?;
                let output: i64 = row.get(4)?;
                Ok((
                    row.get::<_, String>(0)?,
                    DashboardSessionProvider {
                        provider_id: row.get(1)?,
                        request_count: row.get(2)?,
                        tokens_input: input,
                        tokens_output: output,
                        total_tokens: input + output,
                    },
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|error| format!("dashboard sessions: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("dashboard sessions: {error}"))?;

        let names = SessionRepository::new(conn)
            .list_session_meta()
            .map_err(|error| format!("dashboard session metadata: {error}"))?
            .into_iter()
            .filter(|row| !row.display_name.is_empty())
            .map(|row| (row.hash, row.display_name))
            .collect::<HashMap<_, _>>();

        let mut sessions: HashMap<String, DashboardSession> = HashMap::new();
        for (session_hash, provider_usage, first_seen_at, last_seen_at) in session_rows {
            let entry = sessions
                .entry(session_hash.clone())
                .or_insert_with(|| DashboardSession {
                    session_hash,
                    display_name: String::new(),
                    request_count: 0,
                    tokens_input: 0,
                    tokens_output: 0,
                    total_tokens: 0,
                    first_seen_at: None,
                    last_seen_at: None,
                    providers: Vec::new(),
                });
            entry.request_count += provider_usage.request_count;
            entry.tokens_input += provider_usage.tokens_input;
            entry.tokens_output += provider_usage.tokens_output;
            entry.total_tokens += provider_usage.total_tokens;
            entry.first_seen_at = min_timestamp(entry.first_seen_at.take(), first_seen_at);
            entry.last_seen_at = max_timestamp(entry.last_seen_at.take(), last_seen_at);
            entry.providers.push(provider_usage);
        }

        let mut sessions = sessions.into_values().collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .total_tokens
                .cmp(&left.total_tokens)
                .then_with(|| left.session_hash.cmp(&right.session_hash))
        });
        for (index, session) in sessions.iter_mut().enumerate() {
            session.display_name = names
                .get(&session.session_hash)
                .cloned()
                .unwrap_or_else(|| format!("Session {:02}", index + 1));
            session
                .providers
                .sort_by_key(|provider| std::cmp::Reverse(provider.total_tokens));
        }

        Ok(UsageDashboard {
            range: query.range,
            generated_at,
            start: display_date(bounds.start),
            end: display_date(bounds.end),
            duration_days: span_days(&bounds, &trend),
            request_count,
            tokens_input,
            tokens_output,
            total_tokens: tokens_input + tokens_output,
            provider_count: providers.len() as i64,
            session_count: sessions.len() as i64,
            providers,
            trend,
            heatmap,
            sessions,
        })
    }
}

fn min_timestamp(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (None, value) | (value, None) => value,
        (Some(left), Some(right)) => Some(left.min(right)),
    }
}

fn max_timestamp(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (None, value) | (value, None) => value,
        (Some(left), Some(right)) => Some(left.max(right)),
    }
}
