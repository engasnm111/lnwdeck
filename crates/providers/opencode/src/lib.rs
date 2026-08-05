use chrono::Datelike;
use chrono::{DateTime, Duration, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch, UsageEvent,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    CollectionOutcome, CollectionResult, DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use lnwdeck_security::IdentifierHasher;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::num::NonZeroU64;
use std::path::PathBuf;

/// OpenCode CLI local-session collector.
///
/// Reads only metadata columns (`tokens_*`, `cost`, `model`, timestamps)
/// from the OpenCode SQLite store. Session identifiers are persisted as
/// keyed hashes; raw ids, prompts, responses and paths never leave the
/// source as normalized data.
pub struct OpenCodeAdapter {
    hasher: IdentifierHasher,
    db_path: PathBuf,
}

const ADAPTER_VERSION: &str = "0.2.0";

/// OpenCode Go's published USD caps (https://opencode.ai/docs/go): $12 per
/// 5 hours, $30 per week, $60 per month. The caps are not stored in the local
/// database, so they are hardcoded exactly like TokenTracker does. The
/// estimate proves Go turns were billed, never that the account still holds
/// an active Go subscription.
const GO_SESSION_LIMIT_MICRO: u64 = 12_000_000;
const GO_WEEK_LIMIT_MICRO: u64 = 30_000_000;
const GO_MONTH_LIMIT_MICRO: u64 = 60_000_000;
const MICRO_DOLLARS: f64 = 1_000_000.0;
const GO_SESSION_MS: i64 = 5 * 3600 * 1000;
const GO_WEEK_MS: i64 = 7 * 24 * 3600 * 1000;

/// OpenCode may store the model as a JSON descriptor like
/// `{"id":"glm-5.2","providerID":"opencode-go"}`. Reduce it to the model id
/// when possible; otherwise keep the raw string.
fn normalize_model(raw: Option<String>) -> String {
    let Some(model) = raw else {
        return "unknown".to_string();
    };
    let trimmed = model.trim();
    if trimmed.starts_with('{') {
        if let Ok(serde_json::Value::Object(map)) =
            serde_json::from_str::<serde_json::Value>(trimmed)
        {
            if let Some(serde_json::Value::String(id)) = map.get("id") {
                return id.clone();
            }
        }
    }
    trimmed.to_string()
}

impl OpenCodeAdapter {
    /// Resolves the OpenCode data location from environment variables and
    /// builds an adapter that hashes source identifiers with `hash_key`.
    pub fn new(hash_key: &[u8]) -> Self {
        let db_path = Self::default_db_path().unwrap_or_else(|| PathBuf::from("opencode.db"));
        Self::with_db_path(hash_key, db_path)
    }

    /// Adapter pinned to an explicit database path (used by tests and
    /// future user-configured sources).
    pub fn with_db_path(hash_key: &[u8], db_path: PathBuf) -> Self {
        Self {
            hasher: IdentifierHasher::new(hash_key),
            db_path,
        }
    }

    fn default_db_path() -> Option<PathBuf> {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            let candidate = PathBuf::from(xdg).join("opencode").join("opencode.db");
            if candidate.exists() {
                return Some(candidate);
            }
        }
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join(".local/share/opencode/opencode.db"))
    }

    fn open_read_only(&self) -> Result<Connection, String> {
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "SOURCE_UNAVAILABLE".to_string())
    }

    fn detection(&self) -> Result<DetectionResult, String> {
        let source_exists = self.db_path.is_file();
        let mut result = DetectionResult {
            provider_id: "opencode".to_string(),
            display_name: "OpenCode".to_string(),
            enabled: true,
            detected: false,
            detection_method: "local_sqlite".to_string(),
            source_type: "sqlite".to_string(),
            source_exists,
            permission_state: "n/a".to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        };
        if !source_exists {
            result.permission_state = "not_found".to_string();
            return Ok(result);
        }

        match self.open_read_only() {
            Err(code) => {
                result.detection_error_code = code;
                result.permission_state = "permission_required".to_string();
                Ok(result)
            }
            Ok(conn) => {
                let has_session_table = conn.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM sqlite_master
                        WHERE type = 'table' AND name = 'session'
                     )",
                    [],
                    |row| row.get(0),
                );
                match has_session_table {
                    Ok(true) => {
                        result.detected = true;
                        result.permission_state = "read_ok".to_string();
                        Ok(result)
                    }
                    Ok(false) => {
                        result.detection_error_code = "INVALID_PROVIDER_DATA".to_string();
                        Ok(result)
                    }
                    Err(_) => {
                        result.detection_error_code = "INVALID_PROVIDER_DATA".to_string();
                        Ok(result)
                    }
                }
            }
        }
    }

    fn collect(&self, cursor: Option<&str>) -> CollectionResult {
        let started_at = Utc::now();
        let conn = match self.open_read_only() {
            Ok(conn) => conn,
            Err(code) => {
                return CollectionResult {
                    batch: None,
                    outcome: CollectionOutcome::failure(
                        "opencode",
                        "passive_scan",
                        started_at,
                        code,
                    ),
                    next_cursor: cursor.map(str::to_string),
                };
            }
        };

        let mut stmt = match conn.prepare(
            "SELECT id, project_id, model, cost, tokens_input, tokens_output,
                    tokens_reasoning, tokens_cache_read, tokens_cache_write, time_updated
             FROM session
             WHERE (tokens_input > 0 OR tokens_output > 0 OR cost > 0)
               AND (?1 IS NULL OR time_updated > ?1)
             ORDER BY time_updated",
        ) {
            Ok(stmt) => stmt,
            Err(_) => {
                return CollectionResult {
                    batch: None,
                    outcome: CollectionOutcome::failure(
                        "opencode",
                        "passive_scan",
                        started_at,
                        "INVALID_PROVIDER_DATA",
                    ),
                    next_cursor: cursor.map(str::to_string),
                };
            }
        };

        let rows = match stmt.query_map([cursor], |row| {
            let session_id: String = row.get(0)?;
            let project_id: String = row.get(1)?;
            let model: Option<String> = row.get(2)?;
            let cost: f64 = row.get(3)?;
            let tokens_input: i64 = row.get(4)?;
            let tokens_output: i64 = row.get(5)?;
            let tokens_reasoning: i64 = row.get(6)?;
            let _tokens_cache_read: i64 = row.get(7)?;
            let _tokens_cache_write: i64 = row.get(8)?;
            let time_updated: i64 = row.get(9)?;
            Ok((
                session_id,
                project_id,
                model,
                cost,
                tokens_input,
                tokens_output,
                tokens_reasoning,
                time_updated,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => {
                return CollectionResult {
                    batch: None,
                    outcome: CollectionOutcome::failure(
                        "opencode",
                        "passive_scan",
                        started_at,
                        "INVALID_PROVIDER_DATA",
                    ),
                    next_cursor: cursor.map(str::to_string),
                };
            }
        };

        let mut events = Vec::new();
        let mut seen: u64 = 0;
        let mut parsed: u64 = 0;
        let mut warnings = Vec::new();
        let mut next_cursor = cursor.map(str::to_string);

        for row in rows {
            seen += 1;
            let (
                session_id,
                _project_id,
                model,
                cost,
                tokens_input,
                tokens_output,
                tokens_reasoning,
                time_updated,
            ) = match row {
                Ok(values) => values,
                Err(_) => {
                    warnings.push("ROW_SKIPPED".to_string());
                    continue;
                }
            };
            let Some(timestamp) = DateTime::from_timestamp_millis(time_updated) else {
                warnings.push("ROW_SKIPPED".to_string());
                continue;
            };

            let fingerprint = self
                .hasher
                .hash(format!("opencode:{session_id}:{time_updated}").as_bytes());

            events.push(UsageEvent {
                id: fingerprint,
                timestamp,
                provider_id: "opencode".to_string(),
                model: normalize_model(model),
                tokens_input: tokens_input.max(0) as u64,
                tokens_output: (tokens_output + tokens_reasoning).max(0) as u64,
                confidence: Confidence::High,
                data_source: "opencode_db".to_string(),
                cost: format!("{cost:.6}"),
            });
            parsed += 1;
            next_cursor = Some(time_updated.to_string());
        }

        let events_normalized = events.len() as u64;
        let mut outcome =
            CollectionOutcome::success("opencode", "passive_scan", started_at, events_normalized);
        outcome.source_records_seen = seen;
        outcome.records_parsed = parsed;
        if !warnings.is_empty() {
            outcome = outcome.with_warning("ROW_SKIPPED");
        }

        CollectionResult {
            batch: Some(UsageBatch {
                batch_id: format!("opencode_{}", Utc::now().timestamp()),
                events,
            }),
            outcome,
            next_cursor,
        }
    }

    /// Local quota estimate with two tiers, mirroring TokenTracker's sources:
    ///
    /// 1. When the `message` table carries opencode-go assistant turns, their
    ///    billed USD cost is compared against Go's published dollar caps
    ///    ($12 / 5h, $30 / week, $60 / month) and reported as credit windows
    ///    with real limits, remaining and percentages.
    /// 2. Otherwise the session table's token history produces usage-only
    ///    windows: real consumption with an unknown limit, so the UI never
    ///    renders a fake remaining bar.
    fn quota_estimate(&self) -> Result<Option<QuotaReport>, String> {
        if !self.db_path.is_file() {
            return Ok(None);
        }
        let conn = self.open_read_only()?;
        let now = Utc::now();
        if let Some(windows) = opencode_go_windows(&conn, now)? {
            return Ok(Some(QuotaReport::new(
                "opencode",
                "local_estimate",
                windows,
                DEFAULT_FRESHNESS,
            )));
        }
        let now_ms = now.timestamp_millis();
        let buckets = [
            (
                "5h",
                "5-hour",
                QuotaWindowScope::Rolling,
                5 * 3600 * 1000i64,
            ),
            (
                "7d",
                "7-day",
                QuotaWindowScope::Weekly,
                7 * 24 * 3600 * 1000i64,
            ),
            (
                "30d",
                "30-day",
                QuotaWindowScope::Monthly,
                30 * 24 * 3600 * 1000i64,
            ),
        ];
        let mut windows = Vec::with_capacity(buckets.len());
        for (key, label, scope, window_ms) in buckets {
            // A query failure is reported, never silently counted as zero
            // usage: a zero would be indistinguishable from "no activity".
            let used: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(tokens_input + tokens_output + tokens_reasoning), 0)
                     FROM session
                     WHERE (tokens_input > 0 OR tokens_output > 0 OR cost > 0)
                       AND time_updated >= ?1",
                    [now_ms - window_ms],
                    |row| row.get(0),
                )
                .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
            // Without opencode-go rows the plan limit is unknown, so the
            // window records real usage with an unknown limit instead of a
            // fake percentage.
            windows.push(QuotaWindow::usage_only(
                key,
                label,
                scope,
                QuotaKind::Tokens,
                used.max(0) as u64,
                None,
                Confidence::Medium,
            ));
        }
        let report = QuotaReport::new("opencode", "local_estimate", windows, DEFAULT_FRESHNESS);
        Ok(Some(report))
    }
}

/// Monday 00:00 UTC of the week containing `now`.
fn week_start_utc(now: DateTime<Utc>) -> DateTime<Utc> {
    let days = now.weekday().num_days_from_monday() as i64;
    (now - Duration::days(days))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight exists")
        .and_utc()
}

/// First day 00:00 UTC of the month containing `now`.
fn month_start_utc(now: DateTime<Utc>) -> DateTime<Utc> {
    now.date_naive()
        .with_day(1)
        .expect("first of month exists")
        .and_hms_opt(0, 0, 0)
        .expect("midnight exists")
        .and_utc()
}

/// Aggregates opencode-go assistant turns from the `message` table and
/// compares their billed USD cost against Go's published dollar caps.
///
/// Returns `Ok(None)` when the local store predates the `message` table or
/// holds no opencode-go rows, so the caller falls back to usage-only token
/// windows. A query failure against an existing table is reported instead,
/// so a corrupt store is never silently counted as zero usage. Only the
/// numeric `cost` and the `time.created` timestamp are extracted; paths,
/// prompts and responses inside the JSON payload never leave the database.
fn opencode_go_windows(
    conn: &Connection,
    now: DateTime<Utc>,
) -> Result<Option<Vec<QuotaWindow>>, String> {
    let has_message_table: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'message'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    if !has_message_table {
        return Ok(None);
    }

    let session_start_ms = now.timestamp_millis() - GO_SESSION_MS;
    let week_start_ms = week_start_utc(now).timestamp_millis();
    let week_end_ms = week_start_ms + GO_WEEK_MS;
    let month_start_ms = month_start_utc(now).timestamp_millis();
    let month_end_ms = month_start_utc(now + Duration::days(32)).timestamp_millis();

    let row: Option<(f64, Option<i64>, f64, f64, i64)> = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN created_ms >= ?1 THEN cost ELSE 0 END), 0),
                MIN(CASE WHEN created_ms >= ?1 THEN created_ms END),
                COALESCE(SUM(CASE WHEN created_ms >= ?2 AND created_ms < ?3 THEN cost ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN created_ms >= ?4 AND created_ms < ?5 THEN cost ELSE 0 END), 0),
                COUNT(*)
             FROM (
                SELECT
                    CAST(COALESCE(json_extract(data, '$.time.created'), time_created) AS INTEGER) AS created_ms,
                    CAST(json_extract(data, '$.cost') AS REAL) AS cost
                FROM message
                WHERE json_valid(data)
                  AND json_extract(data, '$.providerID') = 'opencode-go'
                  AND json_extract(data, '$.role') = 'assistant'
                  AND json_type(data, '$.cost') IN ('integer', 'real')
             )",
            rusqlite::params![
                session_start_ms,
                week_start_ms,
                week_end_ms,
                month_start_ms,
                month_end_ms
            ],
            |row| {
                Ok((
                    row.get::<_, f64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let Some((session_used, session_oldest, week_used, month_used, row_count)) = row else {
        return Ok(None);
    };
    if row_count <= 0 {
        return Ok(None);
    }

    let to_micro = |dollars: f64| (dollars.max(0.0) * MICRO_DOLLARS).round() as u64;
    let session_limit = NonZeroU64::new(GO_SESSION_LIMIT_MICRO).expect("non-zero cap");
    let week_limit = NonZeroU64::new(GO_WEEK_LIMIT_MICRO).expect("non-zero cap");
    let month_limit = NonZeroU64::new(GO_MONTH_LIMIT_MICRO).expect("non-zero cap");

    let session_reset =
        session_oldest.and_then(|oldest| DateTime::from_timestamp_millis(oldest + GO_SESSION_MS));

    let windows = vec![
        QuotaWindow::with_limit(
            "5h",
            "5-hour",
            QuotaWindowScope::Rolling,
            QuotaKind::Credits,
            to_micro(session_used),
            session_limit,
            session_reset,
            Confidence::Medium,
        ),
        QuotaWindow::with_limit(
            "7d",
            "7-day",
            QuotaWindowScope::Weekly,
            QuotaKind::Credits,
            to_micro(week_used),
            week_limit,
            DateTime::from_timestamp_millis(week_end_ms),
            Confidence::Medium,
        ),
        QuotaWindow::with_limit(
            "30d",
            "30-day",
            QuotaWindowScope::Monthly,
            QuotaKind::Credits,
            to_micro(month_used),
            month_limit,
            DateTime::from_timestamp_millis(month_end_ms),
            Confidence::Medium,
        ),
    ];
    Ok(Some(windows))
}

impl ProviderAdapter for OpenCodeAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: "opencode",
            display_name: "OpenCode",
            vendor: "OpenCode",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::LocalEstimate,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        self.collect(None)
            .batch
            .ok_or_else(|| "SOURCE_UNAVAILABLE".to_string())
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.quota_estimate()
    }
    fn health_check(&self) -> AdapterHealth {
        match self.detection() {
            Ok(detection) if detection.detected => AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "OpenCode local store detected".to_string(),
            },
            Ok(_) => AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "OpenCode local store not found".to_string(),
            },
            Err(code) => AdapterHealth {
                status: AdapterHealthStatus::Unhealthy,
                message: code,
            },
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem]
    }
    fn detect(&self) -> Result<DetectionResult, String> {
        self.detection()
    }
    fn collect_usage_with_cursor(&self, cursor: Option<&str>) -> CollectionResult {
        self.collect(cursor)
    }
}
