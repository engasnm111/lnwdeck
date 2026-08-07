//! Shared reader for OpenCode-fork local stores.
//!
//! OpenCode and its forks (ZCode, Kilo CLI, Mimo Code) persist assistant
//! turns in an identical `message` table: `id`, `session_id`,
//! `time_created`, `time_updated` and a JSON `data` column carrying
//! `providerID`, `modelID`, `role` and `tokens`. This module opens that
//! table read-only and turns each counted row into a [`TokenSample`], so a
//! fork adapter only decides which rows belong to it (by provider or model
//! policy) instead of re-implementing the schema.
//!
//! Only numbers, timestamps and model identifiers are extracted. Prompts,
//! responses and paths inside the JSON payload never leave the database.

use crate::token_scan::{ScanBounds, TokenSample};
use chrono::{DateTime, Utc};
use lnwdeck_domain::Confidence;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::path::Path;

/// One parsed `message` row. Rows without usable token numbers or timestamps
/// are dropped while reading; nothing else is carried out of the database.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageSample {
    pub timestamp: DateTime<Utc>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub model: Option<String>,
    pub provider_id: Option<String>,
}

/// Opens the database read-only, or reports `SOURCE_UNAVAILABLE`.
fn open_read_only(db_path: &Path) -> Result<Connection, String> {
    if !db_path.is_file() {
        return Err("SOURCE_UNAVAILABLE".to_string());
    }
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "SOURCE_UNAVAILABLE".to_string())
}

/// Reads every assistant `message` row from an OpenCode-fork store.
///
/// Rows whose JSON payload is unparsable or carries no token numbers are
/// skipped; a corrupt row never fails the whole read. The read is bounded by
/// `bounds.max_files`-style limits through a single bounded query: at most
/// `bounds.max_total_bytes / 64` rows are read so a runaway history cannot
/// stall collection.
pub fn read_messages(db_path: &Path, bounds: &ScanBounds) -> Result<Vec<MessageSample>, String> {
    let conn = open_read_only(db_path)?;
    let row_limit = (bounds.max_total_bytes / 64)
        .min(bounds.max_files as u64)
        .max(1);
    let mut stmt = conn
        .prepare(
            "SELECT time_updated, data
             FROM message
             WHERE json_valid(data)
               AND json_extract(data, '$.role') = 'assistant'
             ORDER BY time_updated
             LIMIT ?1",
        )
        .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;

    let rows = stmt
        .query_map([row_limit as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;

    let mut samples = Vec::new();
    for row in rows {
        let Ok((time_updated, data)) = row else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let Some(sample) = sample_from_row(time_updated, &parsed) else {
            continue;
        };
        samples.push(sample);
    }
    Ok(samples)
}

/// Converts one parsed `message` row into a sample, or `None` when it has no
/// usable timestamp or token numbers.
fn sample_from_row(time_updated_ms: i64, data: &Value) -> Option<MessageSample> {
    let timestamp = DateTime::from_timestamp_millis(time_updated_ms)?;
    let tokens = data.get("tokens")?;
    let input = u64_from(tokens.get("input")).unwrap_or(0);
    let reasoning = u64_from(tokens.get("reasoning")).unwrap_or(0);
    let output = u64_from(tokens.get("output"))
        .unwrap_or(0)
        .saturating_add(reasoning);
    if input == 0 && output == 0 {
        return None;
    }
    Some(MessageSample {
        timestamp,
        input_tokens: input,
        output_tokens: output,
        model: data
            .get("modelID")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_string),
        provider_id: data
            .get("providerID")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|provider| !provider.is_empty())
            .map(str::to_string),
    })
}

fn u64_from(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64(),
        _ => None,
    }
}

/// Adapts [`MessageSample`] rows into the shared [`TokenSample`] shape used
/// by the quota windows and the usage event builder.
pub fn to_token_samples(samples: &[MessageSample]) -> Vec<TokenSample> {
    samples
        .iter()
        .map(|sample| TokenSample {
            timestamp: sample.timestamp,
            input_tokens: sample.input_tokens,
            output_tokens: sample.output_tokens,
            model: sample.model.clone(),
        })
        .collect()
}

/// The confidence local SQLite readers assign to their numbers.
pub const SQLITE_CONFIDENCE: Confidence = Confidence::Medium;

/// Runs a bounded count query and returns the row, or `Ok(None)` when the
/// store has no `message` table (a fresh fork database).
pub fn has_message_table(db_path: &Path) -> Result<bool, String> {
    let conn = open_read_only(db_path)?;
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'message'
         )",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())
    .map(|exists| exists.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn open_db(dir: &std::path::Path) -> Connection {
        let conn = Connection::open(dir.join("fork.db")).expect("open db");
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT,
                time_created INTEGER,
                time_updated INTEGER,
                data TEXT
             );",
        )
        .expect("schema");
        conn
    }

    fn insert(conn: &Connection, id: &str, time_updated: i64, data: &str) {
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES (?1, 'sess', ?2, ?2, ?3)",
            rusqlite::params![id, time_updated, data],
        )
        .expect("insert");
    }

    fn db_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("fork.db");
        (dir, path)
    }

    fn sample(id: &str, provider: &str, model: &str, input: u64, output: u64) -> String {
        format!(
            r#"{{"id":"{id}","providerID":"{provider}","modelID":"{model}","role":"assistant","tokens":{{"input":{input},"output":{output},"reasoning":0}},"time":{{"created":1700000000000}}}}"#
        )
    }

    #[test]
    fn reads_assistant_rows_with_token_counts() {
        let (dir, path) = db_path();
        let conn = open_db(dir.path());
        insert(
            &conn,
            "m1",
            1_700_000_000_000,
            &sample("m1", "zai", "glm-5.2", 100, 50),
        );
        insert(
            &conn,
            "m2",
            1_700_000_001_000,
            &sample("m2", "zai", "glm-5.2", 0, 0),
        );
        drop(conn);

        let samples = read_messages(&path, &ScanBounds::default()).expect("read");
        assert_eq!(samples.len(), 1, "zero-token rows are dropped");
        assert_eq!(samples[0].input_tokens, 100);
        assert_eq!(samples[0].output_tokens, 50);
        assert_eq!(samples[0].model.as_deref(), Some("glm-5.2"));
        assert_eq!(samples[0].provider_id.as_deref(), Some("zai"));
    }

    #[test]
    fn missing_store_reports_source_unavailable() {
        let (_dir, path) = db_path();
        assert_eq!(
            read_messages(&path, &ScanBounds::default()),
            Err("SOURCE_UNAVAILABLE".to_string())
        );
    }

    #[test]
    fn corrupt_rows_are_skipped_not_fatal() {
        let (dir, path) = db_path();
        let conn = open_db(dir.path());
        insert(
            &conn,
            "m1",
            1_700_000_000_000,
            &sample("m1", "zai", "glm-5.2", 10, 20),
        );
        conn.execute(
            "INSERT INTO message (id, session_id, time_updated, data)
             VALUES ('bad', 'sess', 1700000001000, 'not json')",
            [],
        )
        .expect("insert bad");
        insert(
            &conn,
            "m2",
            1_700_000_002_000,
            &sample("m2", "zai", "glm-5.2", 1, 1),
        );
        drop(conn);

        let samples = read_messages(&path, &ScanBounds::default()).expect("read");
        assert_eq!(samples.len(), 2, "bad row is skipped, good rows survive");
    }

    #[test]
    fn to_token_samples_preserves_numbers_only() {
        let sample = MessageSample {
            timestamp: DateTime::from_timestamp_millis(1_700_000_000_000).unwrap(),
            input_tokens: 10,
            output_tokens: 20,
            model: Some("glm-5.2".to_string()),
            provider_id: Some("zai".to_string()),
        };
        let tokens = to_token_samples(&[sample]);
        assert_eq!(tokens[0].input_tokens, 10);
        assert_eq!(tokens[0].output_tokens, 20);
        assert_eq!(tokens[0].model.as_deref(), Some("glm-5.2"));
    }

    #[test]
    fn has_message_table_detects_schema() {
        let (dir, path) = db_path();
        let conn = open_db(dir.path());
        drop(conn);
        assert!(has_message_table(&path).expect("exists"));

        let empty = tempdir().expect("temp");
        let empty_path = empty.path().join("none.db");
        Connection::open(&empty_path).expect("open empty");
        assert!(!has_message_table(&empty_path).expect("exists"));
    }
}
