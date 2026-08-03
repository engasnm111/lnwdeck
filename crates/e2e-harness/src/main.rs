//! End-to-end harness.
//!
//! Runs the real refresh pipeline against caller-supplied source directories and
//! prints a JSON summary of what was stored. The end-to-end test drives this
//! binary so the assertions cover the shipped code path rather than a mock.

use lnwdeck_application::refresh::RefreshAll;
use lnwdeck_provider_claude::ClaudeAdapter;
use lnwdeck_provider_runtime::ProviderAdapter;
use lnwdeck_storage::{migrations::apply_all, Storage};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct Summary {
    events_inserted: u64,
    duplicates_skipped: u64,
    quota_windows: u64,
    privacy_rejections: u64,
    providers: Vec<String>,
    error_codes: Vec<String>,
}

struct Args {
    db: PathBuf,
    claude_projects: PathBuf,
    export: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut db = None;
    let mut claude_projects = None;
    let mut export = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--db" => db = Some(PathBuf::from(value)),
            "--claude-projects" => claude_projects = Some(PathBuf::from(value)),
            "--export" => export = Some(PathBuf::from(value)),
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(Args {
        db: db.ok_or("--db is required")?,
        claude_projects: claude_projects.ok_or("--claude-projects is required")?,
        export,
    })
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let storage = Storage::open(&args.db).expect("open database");
    apply_all(&storage.conn).expect("apply migrations");

    let adapter = ClaudeAdapter::with_paths(
        args.claude_projects.clone(),
        args.claude_projects.join(".credentials.json"),
    );
    let adapters: Vec<&dyn ProviderAdapter> = vec![&adapter];
    let cycle = RefreshAll::execute(&storage.conn, &adapters);

    let mut error_codes: Vec<String> = cycle
        .usage
        .iter()
        .map(|outcome| outcome.error_code.clone())
        .chain(cycle.quota.iter().map(|outcome| outcome.error_code.clone()))
        .filter(|code| !code.is_empty())
        .collect();
    error_codes.sort();
    error_codes.dedup();

    let mut providers: Vec<String> = storage
        .conn
        .prepare("SELECT DISTINCT provider_id FROM usage_events")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<String>, _>>()
        })
        .unwrap_or_default();
    providers.sort();

    let quota_windows: u64 = storage
        .conn
        .query_row("SELECT COUNT(*) FROM quota_windows", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0)
        .max(0) as u64;

    let totals = lnwdeck_storage::repositories::DiagnosticsRepository::new(&storage.conn)
        .pipeline_totals()
        .expect("pipeline totals");

    let summary = Summary {
        events_inserted: cycle
            .usage
            .iter()
            .map(|outcome| outcome.events_inserted)
            .sum(),
        duplicates_skipped: cycle
            .usage
            .iter()
            .map(|outcome| outcome.duplicates_skipped)
            .sum(),
        quota_windows,
        privacy_rejections: totals.privacy_rejections,
        providers,
        error_codes,
    };

    if let Some(path) = args.export {
        let export = export_rows(&storage);
        std::fs::write(path, export).expect("write export");
    }

    println!(
        "{}",
        serde_json::to_string(&summary).expect("serialize summary")
    );
}

/// Dumps every stored usage event and quota window as JSON so the test can
/// assert on exactly what was persisted.
fn export_rows(storage: &Storage) -> String {
    let mut events = Vec::new();
    let mut stmt = storage
        .conn
        .prepare(
            "SELECT id, provider_id, model, tokens_input, tokens_output, confidence,
                    data_source, cost, timestamp
             FROM usage_events ORDER BY timestamp",
        )
        .expect("prepare events");
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "provider_id": row.get::<_, String>(1)?,
                "model": row.get::<_, String>(2)?,
                "tokens_input": row.get::<_, i64>(3)?,
                "tokens_output": row.get::<_, i64>(4)?,
                "confidence": row.get::<_, String>(5)?,
                "data_source": row.get::<_, String>(6)?,
                "cost": row.get::<_, String>(7)?,
                "timestamp": row.get::<_, String>(8)?,
            }))
        })
        .expect("query events");
    for row in rows {
        events.push(row.expect("event row"));
    }

    let mut windows = Vec::new();
    let mut stmt = storage
        .conn
        .prepare(
            "SELECT provider_id, window_key, label, used, quota_limit, remaining_percent
             FROM quota_windows ORDER BY id",
        )
        .expect("prepare windows");
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "provider_id": row.get::<_, String>(0)?,
                "window_key": row.get::<_, String>(1)?,
                "label": row.get::<_, String>(2)?,
                "used": row.get::<_, i64>(3)?,
                "quota_limit": row.get::<_, Option<i64>>(4)?,
                "remaining_percent": row.get::<_, Option<f64>>(5)?,
            }))
        })
        .expect("query windows");
    for row in rows {
        windows.push(row.expect("window row"));
    }

    serde_json::to_string_pretty(&serde_json::json!({
        "usage_events": events,
        "quota_windows": windows,
    }))
    .expect("serialize export")
}
