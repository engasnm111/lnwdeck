//! TokenTracker-style dashboard read model.
//!
//! The query keeps the summary, provider breakdown, trend, heatmap and
//! session table on one consistent time/provider filter. It reads only the
//! privacy-safe `usage_events` projection; prompts, responses and raw paths
//! never enter this module.

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use lnwdeck_provider_runtime::AdapterRegistry;
use lnwdeck_storage::repositories::SessionRepository;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
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
    pub display_name: String,
    pub vendor: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_cached: i64,
    pub tokens_cache_write: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardTrendPoint {
    pub bucket: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_cached: i64,
    pub tokens_cache_write: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
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
    pub display_name: String,
    pub vendor: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_cached: i64,
    pub tokens_cache_write: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSession {
    pub session_hash: String,
    pub display_name: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_cached: i64,
    pub tokens_cache_write: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
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
    pub tokens_cached: i64,
    pub tokens_cache_write: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
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

fn bounds_for(query: &DashboardQuery, now: DateTime<Local>) -> Result<Bounds, String> {
    let today = now.date_naive();
    let tomorrow = today
        .succ_opt()
        .ok_or_else(|| "invalid day boundary".to_string())?;
    let (start_date, end_date) = match query.range {
        DashboardRange::Day => (Some(today), Some(tomorrow)),
        DashboardRange::Week => (Some(today - Duration::days(6)), Some(tomorrow)),
        DashboardRange::Month => (Some(today - Duration::days(29)), Some(tomorrow)),
        DashboardRange::Year => (Some(today - Duration::days(364)), Some(tomorrow)),
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

fn calendar_span(
    bounds: &Bounds,
    points: &[DashboardTrendPoint],
) -> Option<(NaiveDate, NaiveDate)> {
    if let (Some(start), Some(end)) = (bounds.start, bounds.end) {
        let start = start.with_timezone(&Local).date_naive();
        let end = end.with_timezone(&Local).date_naive();
        return (start < end).then_some((start, end));
    }

    let first = points.first()?.bucket.parse::<NaiveDate>().ok()?;
    let last = points.last()?.bucket.parse::<NaiveDate>().ok()?;
    Some((first, last.succ_opt()?))
}

/// Includes zero-usage days in bounded ranges so each preset is a complete
/// trailing calendar window, not a list of only the days with events.
fn fill_calendar_buckets(
    bounds: &Bounds,
    points: Vec<DashboardTrendPoint>,
) -> Vec<DashboardTrendPoint> {
    let Some((mut day, end)) = calendar_span(bounds, &points) else {
        return points;
    };
    let mut by_day = points
        .into_iter()
        .map(|point| (point.bucket.clone(), point))
        .collect::<HashMap<_, _>>();
    let mut filled = Vec::new();
    while day < end {
        let bucket = day.format("%Y-%m-%d").to_string();
        filled.push(by_day.remove(&bucket).unwrap_or(DashboardTrendPoint {
            bucket,
            request_count: 0,
            tokens_input: 0,
            tokens_cached: 0,
            tokens_cache_write: 0,
            tokens_output: 0,
            tokens_reasoning: 0,
            total_tokens: 0,
        }));
        let Some(next) = day.succ_opt() else {
            break;
        };
        day = next;
    }
    filled
}

fn provider_identity(
    catalog: &HashMap<String, (String, String)>,
    provider_id: &str,
) -> (String, String) {
    catalog
        .get(provider_id)
        .cloned()
        .unwrap_or_else(|| ("Other provider".to_string(), "Other".to_string()))
}

fn total_tokens(input: i64, cached: i64, cache_write: i64, output: i64) -> i64 {
    input
        .saturating_add(cached)
        .saturating_add(cache_write)
        .saturating_add(output)
}

/// OpenCode used this provider id in older lnwdeck databases. Keep those
/// historical rows in the same dashboard card and provider filter as the
/// current canonical id without rewriting user history.
const LEGACY_OPENCODE_PROVIDER_ID: &str = "opencode_cli";

fn provider_filter_matches_sql() -> &'static str {
    "(?3 = '' OR provider_id = ?3 OR (?3 = 'opencode' AND provider_id = 'opencode_cli'))"
}

/// Index-friendly time predicate. `usage_events.timestamp` is stored as
/// canonical UTC RFC3339 (`DateTime<Utc>::to_rfc3339()`), so text comparison
/// matches chronological order and lets SQLite use `idx_usage_timestamp`.
/// This contract is why the storage layer writes UTC-normalized timestamps.
pub fn time_filter_sql() -> &'static str {
    "timestamp >= ?1 AND timestamp < ?2"
}

pub struct QueryDashboard;

impl QueryDashboard {
    pub fn execute(conn: &Connection, query: DashboardQuery) -> Result<UsageDashboard, String> {
        Self::execute_inner(conn, query, None)
    }

    /// Executes the dashboard query with the runtime registry so provider
    /// display names and vendors are resolved from the canonical descriptors.
    pub fn execute_with_registry(
        conn: &Connection,
        query: DashboardQuery,
        registry: &AdapterRegistry,
    ) -> Result<UsageDashboard, String> {
        Self::execute_inner(conn, query, Some(registry))
    }

    fn execute_inner(
        conn: &Connection,
        query: DashboardQuery,
        registry: Option<&AdapterRegistry>,
    ) -> Result<UsageDashboard, String> {
        let generated_at = Utc::now();
        let bounds = bounds_for(&query, generated_at.with_timezone(&Local))?;
        let start = bound_text(bounds.start, "0000-01-01T00:00:00+00:00");
        let end = bound_text(bounds.end, "9999-12-31T23:59:59+00:00");
        let provider = query.provider_id.unwrap_or_default();
        let params = rusqlite::params![start, end, provider];
        let catalog = registry
            .map(|registry| {
                registry
                    .descriptors()
                    .into_iter()
                    .map(|descriptor| {
                        (
                            descriptor.id.to_string(),
                            (
                                descriptor.display_name.to_string(),
                                descriptor.vendor.to_string(),
                            ),
                        )
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        let summary_sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(tokens_input), 0),
                    COALESCE(SUM(tokens_cached), 0),
                    COALESCE(SUM(tokens_cache_write), 0),
                    COALESCE(SUM(tokens_output), 0),
                    COALESCE(SUM(tokens_reasoning), 0)
             FROM usage_events
             WHERE {time} AND {filter}",
            time = time_filter_sql(),
            filter = provider_filter_matches_sql(),
        );
        let (
            request_count,
            tokens_input,
            tokens_cached,
            tokens_cache_write,
            tokens_output,
            tokens_reasoning,
        ): (i64, i64, i64, i64, i64, i64) = conn
            .query_row(&summary_sql, params, |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .map_err(|error| format!("dashboard summary: {error}"))?;

        let mut provider_stmt = conn
            .prepare(&format!(
                "SELECT CASE WHEN provider_id = '{legacy}' THEN 'opencode' ELSE provider_id END,
                        COUNT(*), COALESCE(SUM(tokens_input), 0),
                        COALESCE(SUM(tokens_cached), 0),
                        COALESCE(SUM(tokens_cache_write), 0),
                        COALESCE(SUM(tokens_output), 0),
                        COALESCE(SUM(tokens_reasoning), 0)
                 FROM usage_events
                 WHERE {time} AND {filter}
                 GROUP BY CASE WHEN provider_id = '{legacy}' THEN 'opencode' ELSE provider_id END
                 ORDER BY SUM(tokens_input + tokens_cached + tokens_cache_write + tokens_output) DESC,
                          provider_id",
                legacy = LEGACY_OPENCODE_PROVIDER_ID,
                time = time_filter_sql(),
                filter = provider_filter_matches_sql(),
            ))
            .map_err(|error| format!("dashboard providers: {error}"))?;
        let providers = provider_stmt
            .query_map(params, |row| {
                let provider_id: String = row.get(0)?;
                let input: i64 = row.get(2)?;
                let cached: i64 = row.get(3)?;
                let cache_write: i64 = row.get(4)?;
                let output: i64 = row.get(5)?;
                let reasoning: i64 = row.get(6)?;
                let (display_name, vendor) = provider_identity(&catalog, &provider_id);
                Ok(DashboardProviderUsage {
                    provider_id,
                    display_name,
                    vendor,
                    request_count: row.get(1)?,
                    tokens_input: input,
                    tokens_cached: cached,
                    tokens_cache_write: cache_write,
                    tokens_output: output,
                    tokens_reasoning: reasoning,
                    total_tokens: total_tokens(input, cached, cache_write, output),
                })
            })
            .map_err(|error| format!("dashboard providers: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("dashboard providers: {error}"))?;

        let mut trend_stmt = conn
            .prepare(&format!(
                "SELECT date(timestamp, 'localtime') AS bucket, COUNT(*),
                        COALESCE(SUM(tokens_input), 0),
                        COALESCE(SUM(tokens_cached), 0),
                        COALESCE(SUM(tokens_cache_write), 0),
                        COALESCE(SUM(tokens_output), 0),
                        COALESCE(SUM(tokens_reasoning), 0)
                 FROM usage_events
                 WHERE {time} AND {filter}
                 GROUP BY bucket ORDER BY bucket",
                time = time_filter_sql(),
                filter = provider_filter_matches_sql(),
            ))
            .map_err(|error| format!("dashboard trend: {error}"))?;
        let trend = trend_stmt
            .query_map(params, |row| {
                let input: i64 = row.get(2)?;
                let cached: i64 = row.get(3)?;
                let cache_write: i64 = row.get(4)?;
                let output: i64 = row.get(5)?;
                let reasoning: i64 = row.get(6)?;
                Ok(DashboardTrendPoint {
                    bucket: row.get(0)?,
                    request_count: row.get(1)?,
                    tokens_input: input,
                    tokens_cached: cached,
                    tokens_cache_write: cache_write,
                    tokens_output: output,
                    tokens_reasoning: reasoning,
                    total_tokens: total_tokens(input, cached, cache_write, output),
                })
            })
            .map_err(|error| format!("dashboard trend: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("dashboard trend: {error}"))?;
        let trend = fill_calendar_buckets(&bounds, trend);

        let heatmap = trend
            .iter()
            .map(|point| DashboardHeatmapCell {
                day: point.bucket.clone(),
                request_count: point.request_count,
                total_tokens: point.total_tokens,
            })
            .collect::<Vec<_>>();

        let mut session_stmt = conn
            .prepare(&format!(
                "SELECT session_hash,
                        CASE WHEN provider_id = '{legacy}' THEN 'opencode' ELSE provider_id END,
                        COUNT(*),
                        COALESCE(SUM(tokens_input), 0),
                        COALESCE(SUM(tokens_cached), 0),
                        COALESCE(SUM(tokens_cache_write), 0),
                        COALESCE(SUM(tokens_output), 0),
                        COALESCE(SUM(tokens_reasoning), 0),
                        MIN(timestamp), MAX(timestamp)
                 FROM usage_events
                 WHERE {time} AND {filter}
                 GROUP BY session_hash,
                          CASE WHEN provider_id = '{legacy}' THEN 'opencode' ELSE provider_id END",
                legacy = LEGACY_OPENCODE_PROVIDER_ID,
                time = time_filter_sql(),
                filter = provider_filter_matches_sql(),
            ))
            .map_err(|error| format!("dashboard sessions: {error}"))?;
        let session_rows = session_stmt
            .query_map(params, |row| {
                let provider_id: String = row.get(1)?;
                let input: i64 = row.get(3)?;
                let cached: i64 = row.get(4)?;
                let cache_write: i64 = row.get(5)?;
                let output: i64 = row.get(6)?;
                let reasoning: i64 = row.get(7)?;
                let (display_name, vendor) = provider_identity(&catalog, &provider_id);
                Ok((
                    row.get::<_, String>(0)?,
                    DashboardSessionProvider {
                        provider_id,
                        display_name,
                        vendor,
                        request_count: row.get(2)?,
                        tokens_input: input,
                        tokens_cached: cached,
                        tokens_cache_write: cache_write,
                        tokens_output: output,
                        tokens_reasoning: reasoning,
                        total_tokens: total_tokens(input, cached, cache_write, output),
                    },
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
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
                    tokens_cached: 0,
                    tokens_cache_write: 0,
                    tokens_output: 0,
                    tokens_reasoning: 0,
                    total_tokens: 0,
                    first_seen_at: None,
                    last_seen_at: None,
                    providers: Vec::new(),
                });
            entry.request_count += provider_usage.request_count;
            entry.tokens_input += provider_usage.tokens_input;
            entry.tokens_cached += provider_usage.tokens_cached;
            entry.tokens_cache_write += provider_usage.tokens_cache_write;
            entry.tokens_output += provider_usage.tokens_output;
            entry.tokens_reasoning += provider_usage.tokens_reasoning;
            entry.total_tokens = total_tokens(
                entry.tokens_input,
                entry.tokens_cached,
                entry.tokens_cache_write,
                entry.tokens_output,
            );
            entry.first_seen_at = min_timestamp(entry.first_seen_at.take(), first_seen_at);
            entry.last_seen_at = max_timestamp(entry.last_seen_at.take(), last_seen_at);
            entry.providers.push(provider_usage);
        }

        let mut sessions = sessions.into_values().collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            compare_last_seen_desc(&left.last_seen_at, &right.last_seen_at)
                .then_with(|| right.total_tokens.cmp(&left.total_tokens))
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
            tokens_cached,
            tokens_cache_write,
            tokens_output,
            tokens_reasoning,
            total_tokens: total_tokens(
                tokens_input,
                tokens_cached,
                tokens_cache_write,
                tokens_output,
            ),
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

fn compare_last_seen_desc(left: &Option<String>, right: &Option<String>) -> Ordering {
    let left_parsed = left
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok());
    let right_parsed = right
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok());

    match (left_parsed, right_parsed) {
        (Some(left), Some(right)) => right.cmp(&left),
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (None, None) => right.cmp(left),
    }
}

fn max_timestamp(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (None, value) | (value, None) => value,
        (Some(left), Some(right)) => Some(left.max(right)),
    }
}
