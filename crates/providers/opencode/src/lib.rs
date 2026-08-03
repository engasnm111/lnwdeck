use chrono::{DateTime, Utc};
use lnwdeck_domain::{Confidence, QuotaSnapshot, UsageBatch, UsageEvent};
use lnwdeck_provider_runtime::{
    AdapterHealth, AdapterHealthStatus, CollectionOutcome, CollectionResult, DetectionResult,
    Permission, ProviderAdapter,
};
use lnwdeck_security::IdentifierHasher;
use rusqlite::{Connection, OpenFlags};
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

const ADAPTER_VERSION: &str = "0.1.0";

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
            provider_id: "opencode_cli".to_string(),
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
                        "opencode_cli",
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
                        "opencode_cli",
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
                        "opencode_cli",
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
                provider_id: "opencode_cli".to_string(),
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
        let mut outcome = CollectionOutcome::success(
            "opencode_cli",
            "passive_scan",
            started_at,
            events_normalized,
        );
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
}

impl ProviderAdapter for OpenCodeAdapter {
    fn id(&self) -> &str {
        "opencode_cli"
    }
    fn name(&self) -> &str {
        "OpenCode"
    }
    fn collect_usage(&self) -> Result<UsageBatch, String> {
        self.collect(None)
            .batch
            .ok_or_else(|| "SOURCE_UNAVAILABLE".to_string())
    }
    fn collect_quota(&self) -> Result<Option<QuotaSnapshot>, String> {
        Ok(None)
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
