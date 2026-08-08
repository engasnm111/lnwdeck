//! Bounded passive scanner for local provider artifacts.
//!
//! Several AI tools keep their session history on disk as JSON or JSONL with
//! token counts attached, but every tool names the fields differently and the
//! shapes change between versions. This module reads those files read-only,
//! walks the parsed JSON, and records a usage sample wherever an object
//! carries both an input-token and an output-token number together with a
//! usable timestamp.
//!
//! Rules this module exists to enforce:
//!
//! - Nothing is invented. When no recognizable record is found the scan
//!   returns no samples, and the caller reports "no data" instead of zeros.
//! - Only numbers, timestamps and model identifiers are extracted. Prompts,
//!   responses, file names and absolute paths are never carried out.
//! - Every scan is bounded by file count and byte budget, so a corrupted or
//!   enormous history cannot stall collection.

use chrono::{DateTime, Utc};
use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope, UsageEvent};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Hard limits for a single scan.
#[derive(Debug, Clone, Copy)]
pub struct ScanBounds {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_depth: usize,
}

impl Default for ScanBounds {
    fn default() -> Self {
        Self {
            max_files: 400,
            max_total_bytes: 32 * 1024 * 1024,
            max_file_bytes: 8 * 1024 * 1024,
            max_depth: 6,
        }
    }
}

/// One usage sample extracted from a local artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenSample {
    pub timestamp: DateTime<Utc>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub model: Option<String>,
}

/// A normalized sample when a provider exposes cached, cache-write and
/// reasoning token counters separately.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageBreakdownSample {
    pub timestamp: DateTime<Utc>,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub model: Option<String>,
}

/// Result of a bounded scan.
#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub samples: Vec<TokenSample>,
    pub files_seen: u64,
    pub files_parsed: u64,
    pub bytes_read: u64,
    pub truncated: bool,
}

impl ScanReport {
    /// True when the scan found no usable record at all.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Total input + output tokens of samples inside the given rolling
    /// window, measured backwards from `now`.
    pub fn tokens_within(&self, now: DateTime<Utc>, window: chrono::Duration) -> u64 {
        let cutoff = now - window;
        self.samples
            .iter()
            .filter(|sample| sample.timestamp > cutoff && sample.timestamp <= now)
            .fold(0u64, |acc, sample| {
                acc.saturating_add(sample.input_tokens)
                    .saturating_add(sample.output_tokens)
            })
    }

    /// Newest sample timestamp, if any.
    pub fn latest(&self) -> Option<DateTime<Utc>> {
        self.samples.iter().map(|sample| sample.timestamp).max()
    }
}

const TIMESTAMP_KEYS: &[&str] = &[
    "timestamp",
    "time",
    "ts",
    "createdAt",
    "created_at",
    "created",
    "date",
    "startTime",
    "start_time",
    "endTime",
    "end_time",
    "time_updated",
    "updatedAt",
    "updated_at",
    "requestTime",
];

/// Only explicit token fields are accepted. Bare `input` and `output` keys were
/// tried and removed: they match unrelated numbers in editor and CLI logs, which
/// inflated recorded usage by orders of magnitude.
const INPUT_KEYS: &[&str] = &[
    "input_tokens",
    "inputTokens",
    "prompt_tokens",
    "promptTokens",
    "promptTokenCount",
    "tokens_input",
    "inputTokenCount",
];

const OUTPUT_KEYS: &[&str] = &[
    "output_tokens",
    "outputTokens",
    "completion_tokens",
    "completionTokens",
    "candidatesTokenCount",
    "tokens_output",
    "outputTokenCount",
];

const MODEL_KEYS: &[&str] = &["model", "modelId", "model_id", "modelName", "model_name"];

/// File extensions considered scannable.
const SCANNABLE_EXTENSIONS: &[&str] = &["json", "jsonl", "log", "ndjson"];

/// Collects scannable files under `root`, breadth-limited by `bounds`.
/// Returns paths only; nothing is read yet.
pub fn collect_files(root: &Path, bounds: &ScanBounds) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if !root.is_dir() {
        return found;
    }
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > bounds.max_depth || found.len() >= bounds.max_files {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            let is_scannable = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| SCANNABLE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            if is_scannable {
                found.push(path);
                if found.len() >= bounds.max_files {
                    break;
                }
            }
        }
    }
    found.sort();
    found
}

/// Scans every scannable file under `root` and extracts token samples.
///
/// Unreadable files and unparsable lines are skipped rather than failing the
/// whole scan: a single corrupt session file must not hide the rest of the
/// history. The report states how much was read so the caller can record
/// honest evidence.
pub fn scan_directory(root: &Path, bounds: &ScanBounds) -> ScanReport {
    let mut report = ScanReport::default();
    for path in collect_files(root, bounds) {
        report.files_seen += 1;
        let Ok(meta) = path.metadata() else {
            continue;
        };
        if meta.len() > bounds.max_file_bytes {
            report.truncated = true;
            continue;
        }
        if report.bytes_read + meta.len() > bounds.max_total_bytes {
            report.truncated = true;
            break;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        report.bytes_read += meta.len();
        let before = report.samples.len();
        extract_from_text(&content, &mut report.samples);
        if report.samples.len() > before {
            report.files_parsed += 1;
        }
    }
    report
}

/// Scans several roots and merges the reports. Roots that do not exist are
/// skipped; the merged report states how much was actually read.
pub fn scan_directories(roots: &[PathBuf], bounds: &ScanBounds) -> ScanReport {
    let mut merged = ScanReport::default();
    for root in roots {
        let report = scan_directory(root, bounds);
        merged.files_seen += report.files_seen;
        merged.files_parsed += report.files_parsed;
        merged.bytes_read += report.bytes_read;
        merged.truncated |= report.truncated;
        merged.samples.extend(report.samples);
    }
    merged
}

/// Extracts samples from one file's text. Accepts a JSON document, a JSON
/// array, or newline-delimited JSON, and tolerates non-JSON lines.
pub fn extract_from_text(content: &str, out: &mut Vec<TokenSample>) {
    let trimmed = content.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(content) {
            walk(&value, None, None, out);
            if !out.is_empty() {
                return;
            }
        }
    }
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !(line.starts_with('{') || line.starts_with('[')) {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            walk(&value, None, None, out);
        }
    }
}

/// Recursive descent that carries the nearest enclosing timestamp and model
/// down the tree, because tools often place them on the parent record and the
/// token counts on a nested `usage` object.
fn walk(
    value: &Value,
    inherited_time: Option<DateTime<Utc>>,
    inherited_model: Option<&str>,
    out: &mut Vec<TokenSample>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                walk(item, inherited_time, inherited_model, out);
            }
        }
        Value::Object(map) => {
            let timestamp = TIMESTAMP_KEYS
                .iter()
                .find_map(|key| map.get(*key).and_then(parse_timestamp))
                .or(inherited_time);
            let model_owned = MODEL_KEYS
                .iter()
                .find_map(|key| map.get(*key).and_then(normalize_model));
            let model = model_owned.as_deref().or(inherited_model);

            let input = INPUT_KEYS
                .iter()
                .find_map(|key| map.get(*key).and_then(as_u64));
            let output = OUTPUT_KEYS
                .iter()
                .find_map(|key| map.get(*key).and_then(as_u64));

            if let (Some(input_tokens), Some(output_tokens), Some(time)) =
                (input, output, timestamp)
            {
                if input_tokens > 0 || output_tokens > 0 {
                    out.push(TokenSample {
                        timestamp: time,
                        input_tokens,
                        output_tokens,
                        model: model.map(str::to_string),
                    });
                }
            }

            for child in map.values() {
                if child.is_object() || child.is_array() {
                    walk(child, timestamp, model, out);
                }
            }
        }
        _ => {}
    }
}

/// Reads a non-negative integer token count. Floats are accepted only when
/// they are whole numbers; strings are accepted when they parse cleanly.
fn as_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => {
            if let Some(unsigned) = number.as_u64() {
                Some(unsigned)
            } else {
                let float = number.as_f64()?;
                if float.is_finite() && float >= 0.0 && float.fract() == 0.0 {
                    Some(float as u64)
                } else {
                    None
                }
            }
        }
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// Parses a timestamp expressed as RFC3339 text, epoch seconds, epoch
/// milliseconds, or epoch microseconds.
fn parse_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
                return Some(parsed.with_timezone(&Utc));
            }
            if let Ok(number) = text.parse::<i64>() {
                return from_epoch(number);
            }
            None
        }
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                from_epoch(integer)
            } else {
                number.as_f64().map(|f| f as i64).and_then(from_epoch)
            }
        }
        _ => None,
    }
}

/// Interprets an epoch value whose unit is not stated. The thresholds keep
/// seconds, milliseconds and microseconds apart for any plausible date.
fn from_epoch(value: i64) -> Option<DateTime<Utc>> {
    if value <= 0 {
        return None;
    }
    if value >= 100_000_000_000_000 {
        DateTime::from_timestamp_micros(value)
    } else if value >= 100_000_000_000 {
        DateTime::from_timestamp_millis(value)
    } else {
        DateTime::from_timestamp(value, 0)
    }
}

/// Model identifiers may be plain strings or JSON descriptors such as
/// `{"id":"gemini-2.5-pro"}`. Anything else is ignored.
fn normalize_model(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Object(map) => map
            .get("id")
            .and_then(|id| id.as_str())
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty()),
        _ => None,
    }
}

/// Rolling windows every local collector reports: 5 hours, 7 days, 30 days.
pub const ROLLING_WINDOWS: &[(&str, &str, QuotaWindowScope, i64)] = &[
    ("5h", "5-hour", QuotaWindowScope::Rolling, 5 * 3600),
    ("7d", "7-day", QuotaWindowScope::Weekly, 7 * 24 * 3600),
    ("30d", "30-day", QuotaWindowScope::Monthly, 30 * 24 * 3600),
];

/// Builds usage-only quota windows from a scan report.
///
/// Local artifacts record what was consumed but not the plan limit, so every
/// window is usage-only: a real `used` value with no limit, no remaining and
/// no percentage. Returns an empty vector when the scan found nothing, so the
/// caller reports "no data" rather than three zeroed windows.
pub fn rolling_usage_windows(
    report: &ScanReport,
    now: DateTime<Utc>,
    confidence: Confidence,
) -> Vec<QuotaWindow> {
    if report.is_empty() {
        return Vec::new();
    }
    ROLLING_WINDOWS
        .iter()
        .map(|(key, label, scope, seconds)| {
            let used = report.tokens_within(now, chrono::Duration::seconds(*seconds));
            QuotaWindow::usage_only(
                *key,
                *label,
                *scope,
                QuotaKind::Tokens,
                used,
                None,
                confidence,
            )
        })
        .collect()
}

/// Deterministic 64-bit FNV-1a hash. Used to derive stable event ids so the
/// same local record ingested twice is recognised as a duplicate instead of
/// being counted again. FNV is specified, so the value does not drift between
/// Rust or dependency versions the way `DefaultHasher` would.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Converts samples into normalized usage events.
///
/// Ids are derived from the provider, timestamp, model and token counts, so
/// re-scanning the same files produces the same ids and storage skips them as
/// duplicates. Cost is left empty here: pricing is applied later by the
/// pricing crate, never guessed by a collector.
pub fn usage_events(
    provider_id: &str,
    data_source: &str,
    samples: &[TokenSample],
    confidence: Confidence,
) -> Vec<UsageEvent> {
    samples
        .iter()
        .map(|sample| {
            let model = sample
                .model
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let fingerprint = format!(
                "{provider_id}|{}|{model}|{}|{}",
                sample.timestamp.timestamp_millis(),
                sample.input_tokens,
                sample.output_tokens
            );
            UsageEvent {
                id: format!("{provider_id}_{:016x}", fnv1a(fingerprint.as_bytes())),
                timestamp: sample.timestamp,
                provider_id: provider_id.to_string(),
                model,
                tokens_input: sample.input_tokens,
                tokens_cached: 0,
                tokens_cache_write: 0,
                tokens_output: sample.output_tokens,
                tokens_reasoning: 0,
                confidence,
                data_source: data_source.to_string(),
                cost: String::new(),
                session_hash: None,
                project_hash: None,
            }
        })
        .collect()
}

/// Converts provider-specific token breakdown samples into normalized usage
/// events. `input_tokens` is non-cached input; cached and cache-write input are
/// stored separately, while reasoning remains a subset of output.
pub fn usage_events_with_breakdown(
    provider_id: &str,
    data_source: &str,
    samples: &[UsageBreakdownSample],
    confidence: Confidence,
) -> Vec<UsageEvent> {
    samples
        .iter()
        .map(|sample| {
            let model = sample
                .model
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let fingerprint = format!(
                "{provider_id}|{}|{model}|{}|{}|{}|{}|{}",
                sample.timestamp.timestamp_millis(),
                sample.input_tokens,
                sample.cached_tokens,
                sample.cache_write_tokens,
                sample.output_tokens,
                sample.reasoning_tokens,
            );
            UsageEvent {
                id: format!("{provider_id}_{:016x}", fnv1a(fingerprint.as_bytes())),
                timestamp: sample.timestamp,
                provider_id: provider_id.to_string(),
                model,
                tokens_input: sample.input_tokens,
                tokens_cached: sample.cached_tokens,
                tokens_cache_write: sample.cache_write_tokens,
                tokens_output: sample.output_tokens,
                tokens_reasoning: sample.reasoning_tokens,
                confidence,
                data_source: data_source.to_string(),
                cost: String::new(),
                session_hash: None,
                project_hash: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 0).expect("fixed now")
    }

    #[test]
    fn extracts_openai_style_usage_from_jsonl() {
        let mut samples = Vec::new();
        extract_from_text(
            r#"{"timestamp":"2027-01-15T10:00:00Z","model":"gpt-5","usage":{"prompt_tokens":120,"completion_tokens":30}}
not json at all
{"timestamp":"2027-01-15T11:00:00Z","model":"gpt-5","usage":{"prompt_tokens":5,"completion_tokens":7}}"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].input_tokens, 120);
        assert_eq!(samples[0].output_tokens, 30);
        assert_eq!(samples[0].model.as_deref(), Some("gpt-5"));
    }

    #[test]
    fn extracts_gemini_style_usage_metadata() {
        let mut samples = Vec::new();
        extract_from_text(
            r#"{"date":1793000000,"response":{"modelId":"gemini-3-pro","usageMetadata":{"promptTokenCount":200,"candidatesTokenCount":80}}}"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].input_tokens, 200);
        assert_eq!(samples[0].output_tokens, 80);
        assert_eq!(samples[0].model.as_deref(), Some("gemini-3-pro"));
    }

    #[test]
    fn extracts_from_json_array_documents() {
        let mut samples = Vec::new();
        extract_from_text(
            r#"[{"ts":1793000000000,"model":{"id":"claude-x"},"input_tokens":10,"output_tokens":20}]"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].model.as_deref(), Some("claude-x"));
        assert_eq!(samples[0].timestamp.timestamp(), 1_793_000_000);
    }

    #[test]
    fn records_without_tokens_or_timestamp_are_ignored() {
        let mut samples = Vec::new();
        extract_from_text(
            r#"{"timestamp":"2027-01-15T10:00:00Z","message":"hello"}
{"input_tokens":10,"output_tokens":5}
{"timestamp":"2027-01-15T10:00:00Z","input_tokens":0,"output_tokens":0}"#,
            &mut samples,
        );
        assert!(
            samples.is_empty(),
            "no timestamp, no tokens, or all-zero records must not become samples: {samples:?}"
        );
    }

    #[test]
    fn prompts_and_paths_are_never_carried_out() {
        let mut samples = Vec::new();
        extract_from_text(
            r#"{"timestamp":"2027-01-15T10:00:00Z","prompt":"my secret prompt","cwd":"C:\\Users\\person\\project","input_tokens":10,"output_tokens":5}"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 1);
        let debug = format!("{:?}", samples[0]);
        assert!(!debug.contains("secret prompt"));
        assert!(!debug.contains("Users"));
    }

    #[test]
    fn tokens_within_sums_only_the_window() {
        let report = ScanReport {
            samples: vec![
                TokenSample {
                    timestamp: now() - Duration::hours(1),
                    input_tokens: 10,
                    output_tokens: 5,
                    model: None,
                },
                TokenSample {
                    timestamp: now() - Duration::days(10),
                    input_tokens: 100,
                    output_tokens: 50,
                    model: None,
                },
            ],
            ..Default::default()
        };
        assert_eq!(report.tokens_within(now(), Duration::hours(5)), 15);
        assert_eq!(report.tokens_within(now(), Duration::days(30)), 165);
        assert_eq!(report.latest(), Some(now() - Duration::hours(1)));
        assert!(!report.is_empty());
    }

    #[test]
    fn scan_directory_walks_nested_files_and_respects_bounds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("projects").join("alpha");
        std::fs::create_dir_all(&nested).expect("create nested dir");
        std::fs::write(
            nested.join("session.jsonl"),
            r#"{"timestamp":"2027-01-15T10:00:00Z","input_tokens":10,"output_tokens":5}"#,
        )
        .expect("write session");
        std::fs::write(dir.path().join("notes.txt"), "ignored").expect("write txt");

        let report = scan_directory(dir.path(), &ScanBounds::default());
        assert_eq!(report.files_seen, 1, "only scannable extensions are read");
        assert_eq!(report.files_parsed, 1);
        assert_eq!(report.samples.len(), 1);
        assert!(report.bytes_read > 0);
        assert!(!report.truncated);

        let tight = ScanBounds {
            max_file_bytes: 1,
            ..ScanBounds::default()
        };
        let bounded = scan_directory(dir.path(), &tight);
        assert!(bounded.is_empty());
        assert!(bounded.truncated, "hitting a bound must be reported");
    }

    #[test]
    fn missing_directory_scans_to_empty_without_error() {
        let report = scan_directory(Path::new("Z:/definitely/not/here"), &ScanBounds::default());
        assert!(report.is_empty());
        assert_eq!(report.files_seen, 0);
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use chrono::Duration;

    fn report_with(samples: Vec<TokenSample>) -> ScanReport {
        ScanReport {
            samples,
            ..Default::default()
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 0).expect("fixed now")
    }

    #[test]
    fn rolling_windows_are_usage_only_with_real_totals() {
        let report = report_with(vec![
            TokenSample {
                timestamp: now() - Duration::hours(2),
                input_tokens: 100,
                output_tokens: 20,
                model: Some("m".to_string()),
            },
            TokenSample {
                timestamp: now() - Duration::days(9),
                input_tokens: 1,
                output_tokens: 1,
                model: None,
            },
        ]);
        let windows = rolling_usage_windows(&report, now(), Confidence::Medium);
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window_key, "5h");
        assert_eq!(windows[0].used, 120);
        assert_eq!(windows[1].used, 120, "the 9-day-old sample is outside 7d");
        assert_eq!(windows[2].used, 122);
        for window in &windows {
            assert_eq!(window.limit, None);
            assert_eq!(window.remaining_percent, None);
            window.check_invariants().expect("consistent window");
        }
    }

    #[test]
    fn empty_scan_produces_no_windows() {
        let windows = rolling_usage_windows(&report_with(vec![]), now(), Confidence::Low);
        assert!(
            windows.is_empty(),
            "no data must not become three zeroed windows"
        );
    }

    #[test]
    fn usage_event_ids_are_stable_and_carry_no_cost_guess() {
        let samples = vec![TokenSample {
            timestamp: now(),
            input_tokens: 10,
            output_tokens: 5,
            model: Some("gpt-5".to_string()),
        }];
        let first = usage_events("provider_x", "local_jsonl", &samples, Confidence::Medium);
        let second = usage_events("provider_x", "local_jsonl", &samples, Confidence::Medium);
        assert_eq!(first, second, "ids must be deterministic for deduplication");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].provider_id, "provider_x");
        assert_eq!(first[0].model, "gpt-5");
        assert_eq!(first[0].tokens_input, 10);
        assert!(
            first[0].cost.is_empty(),
            "a collector must not invent a cost"
        );

        let different = usage_events(
            "provider_x",
            "local_jsonl",
            &[TokenSample {
                output_tokens: 6,
                ..samples[0].clone()
            }],
            Confidence::Medium,
        );
        assert_ne!(
            first[0].id, different[0].id,
            "different token counts must not collide"
        );
    }

    #[test]
    fn missing_model_is_recorded_as_unknown_not_guessed() {
        let events = usage_events(
            "provider_x",
            "local_jsonl",
            &[TokenSample {
                timestamp: now(),
                input_tokens: 1,
                output_tokens: 1,
                model: None,
            }],
            Confidence::Low,
        );
        assert_eq!(events[0].model, "unknown");
    }
}

#[cfg(test)]
mod precision_tests {
    use super::*;

    #[test]
    fn generic_input_and_output_numbers_are_not_treated_as_tokens() {
        let mut samples = Vec::new();
        // A log line from an editor: `input` and `output` here are device ids
        // and byte counts, not token counts.
        extract_from_text(
            r#"{"timestamp":"2027-01-15T10:00:00Z","audio":{"input":2,"output":7}}
{"timestamp":"2027-01-15T10:05:00Z","transfer":{"input":8206900000,"output":22000000}}"#,
            &mut samples,
        );
        assert!(
            samples.is_empty(),
            "only explicit token fields may be counted: {samples:?}"
        );
    }

    #[test]
    fn explicit_token_fields_are_still_recognized() {
        let mut samples = Vec::new();
        extract_from_text(
            r#"{"timestamp":"2027-01-15T10:00:00Z","usage":{"input_tokens":10,"output_tokens":5}}
{"timestamp":"2027-01-15T10:01:00Z","usage":{"promptTokenCount":3,"candidatesTokenCount":4}}"#,
            &mut samples,
        );
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].input_tokens, 10);
        assert_eq!(samples[1].output_tokens, 4);
    }
}
