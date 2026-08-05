//! Bounded streaming reader for Gemini CLI session transcripts.
//!
//! The Gemini CLI keeps one session transcript per chat under
//! `~/.gemini/tmp/<project>/chats/session-*.jsonl`, one JSON document per
//! line. A session header line is followed by user and model messages; every
//! model message carries **cumulative** `tokens` counters (`input`, `output`,
//! `cached`, `thoughts`, `tool`, `total`). Because the counters are
//! cumulative, the usage of one message is the difference between its
//! counters and the previous message's counters; the first token-bearing
//! message only establishes the baseline and emits no event.
//!
//! Older CLI versions wrote a single JSON document with a `messages` array;
//! that shape is accepted too. Everything is streamed line by line within a
//! byte budget, and only timestamps, model identifiers and token deltas are
//! extracted: prompts, responses, tool outputs, summaries and paths never
//! leave the source.

use chrono::{DateTime, Utc};
use lnwdeck_provider_runtime::token_scan::{ScanReport, TokenSample};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Hard limits for a transcript scan.
#[derive(Debug, Clone, Copy)]
pub struct TranscriptBounds {
    pub max_files: usize,
    pub max_bytes_per_file: u64,
    pub max_total_bytes: u64,
    pub max_samples: usize,
    pub max_line_bytes: usize,
}

impl Default for TranscriptBounds {
    fn default() -> Self {
        Self {
            max_files: 200,
            max_bytes_per_file: 16 * 1024 * 1024,
            max_total_bytes: 64 * 1024 * 1024,
            max_samples: 10_000,
            max_line_bytes: 1024 * 1024,
        }
    }
}

/// True for files that look like Gemini session transcripts.
fn is_transcript(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("session-") && (lower.ends_with(".jsonl") || lower.ends_with(".json"))
}

/// Collects transcript files under `<root>/tmp/<project>/chats/`, bounded by
/// file count. Returns paths only; nothing is read yet.
fn collect_transcripts(root: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let tmp = root.join("tmp");
    let Ok(projects) = std::fs::read_dir(&tmp) else {
        return found;
    };
    for project in projects.flatten() {
        if !project.path().is_dir() {
            continue;
        }
        let Ok(chats) = std::fs::read_dir(project.path().join("chats")) else {
            continue;
        };
        for entry in chats.flatten() {
            if entry.path().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .map(is_transcript)
                    .unwrap_or(false)
            {
                found.push(entry.path());
                if found.len() >= max_files {
                    return found;
                }
            }
        }
    }
    found.sort();
    found
}

/// One message's cumulative token counters, when present.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Totals {
    input: u64,
    output: u64,
    thoughts: u64,
    tool: u64,
    total: u64,
}

impl Totals {
    fn from_value(tokens: &Value) -> Option<Totals> {
        let obj = tokens.as_object()?;
        let get = |key: &str| obj.get(key).and_then(as_u64).unwrap_or(0);
        Some(Totals {
            input: get("input"),
            output: get("output"),
            thoughts: get("thoughts"),
            tool: get("tool"),
            total: get("total"),
        })
    }

    fn is_zero(&self) -> bool {
        self.input == 0 && self.output == 0 && self.thoughts == 0 && self.tool == 0
    }
}

/// Reads a non-negative integer; whole floats and numeric strings are
/// accepted, anything else is ignored.
fn as_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64().or_else(|| {
            let float = number.as_f64()?;
            (float.is_finite() && float >= 0.0 && float.fract() == 0.0).then_some(float as u64)
        }),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// Extracts the timestamp of a message line.
fn timestamp_of(msg: &Value) -> Option<DateTime<Utc>> {
    msg.get("timestamp")
        .and_then(|value| value.as_str())
        .and_then(|text| DateTime::parse_from_rfc3339(text.trim()).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
}

/// Feeds one message (already parsed) through the cumulative-delta logic.
///
/// `baseline` holds the previous message's cumulative totals. The first
/// token-bearing message only sets the baseline; every later message emits a
/// sample with the per-field deltas. A counter reset (totals dropped below
/// the previous value, e.g. after a `/clear`) restarts the baseline instead
/// of producing a huge or negative event.
fn process_message(msg: &Value, baseline: &mut Option<Totals>, out: &mut Vec<TokenSample>) {
    let Some(tokens) = msg.get("tokens") else {
        return;
    };
    let Some(current) = Totals::from_value(tokens) else {
        return;
    };
    if current.is_zero() {
        return;
    }
    let Some(prev) = *baseline else {
        *baseline = Some(current);
        return;
    };
    if current.total < prev.total {
        // Counter reset (e.g. a `/clear`): the fresh counters describe the
        // new session's own first turn, so they are reported in full.
        *baseline = Some(current);
        emit_sample(
            msg,
            current.input,
            current.output + current.thoughts + current.tool,
            out,
        );
        return;
    }
    let input = current.input.saturating_sub(prev.input);
    let output = current
        .output
        .saturating_sub(prev.output)
        .saturating_add(current.thoughts.saturating_sub(prev.thoughts))
        .saturating_add(current.tool.saturating_sub(prev.tool));
    *baseline = Some(current);
    if input == 0 && output == 0 {
        return;
    }
    emit_sample(msg, input, output, out);
}

fn emit_sample(msg: &Value, input_tokens: u64, output_tokens: u64, out: &mut Vec<TokenSample>) {
    let Some(timestamp) = timestamp_of(msg) else {
        return;
    };
    let model = msg
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string);
    out.push(TokenSample {
        timestamp,
        input_tokens,
        output_tokens,
        model,
    });
}

/// Streams one transcript file into samples. Files larger than the per-file
/// budget are scanned from the tail so the most recent usage survives; the
/// report is marked truncated in that case.
fn scan_file(path: &Path, bounds: &TranscriptBounds, report: &mut ScanReport) {
    let Ok(file) = File::open(path) else {
        return;
    };
    let Ok(meta) = file.metadata() else {
        return;
    };
    report.files_seen += 1;
    if report.samples.len() >= bounds.max_samples {
        report.truncated = true;
        return;
    }

    let tail_mode = meta.len() > bounds.max_bytes_per_file;
    let (mut reader, skipped) = if tail_mode {
        let Ok(mut file) = file.try_clone() else {
            return;
        };
        // Seek back by exactly the budget so the tail (most recent usage)
        // is what gets scanned; the first line in the window is partial and
        // is discarded below.
        if file
            .seek(SeekFrom::End(-(bounds.max_bytes_per_file as i64)))
            .is_err()
        {
            return;
        }
        let mut reader = BufReader::new(file);
        let mut first = String::new();
        match reader.read_line(&mut first) {
            Ok(0) => (reader, 0u64),
            Ok(_) => (reader, first.len() as u64 + 1),
            Err(_) => return,
        }
    } else {
        (BufReader::new(file), 0u64)
    };
    report.truncated |= tail_mode;

    let mut baseline: Option<Totals> = None;
    let mut single_doc_messages: Option<Vec<Value>> = None;
    let mut bytes_read = skipped;

    loop {
        if report.samples.len() >= bounds.max_samples {
            report.truncated = true;
            break;
        }
        if let Some(messages) = single_doc_messages.as_mut() {
            while let Some(message) = messages.pop() {
                if report.samples.len() >= bounds.max_samples {
                    report.truncated = true;
                    break;
                }
                let before = report.samples.len();
                process_message(&message, &mut baseline, &mut report.samples);
                if report.samples.len() > before {
                    report.files_parsed += 1;
                }
            }
            single_doc_messages = None;
        }
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let consumed = line.len() as u64 + 1;
        if consumed > bounds.max_line_bytes as u64 {
            report.truncated = true;
            bytes_read += consumed;
            continue;
        }
        bytes_read += consumed;
        if bytes_read > bounds.max_total_bytes {
            report.truncated = true;
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        if let Some(array) = value.get("messages").and_then(Value::as_array) {
            // Single-document format: the whole session is one JSON object.
            single_doc_messages = Some(array.iter().rev().cloned().collect());
            continue;
        }

        let before = report.samples.len();
        process_message(&value, &mut baseline, &mut report.samples);
        if report.samples.len() > before {
            report.files_parsed += 1;
        }
    }
    report.bytes_read += bytes_read;
}

/// Scans every Gemini transcript under `<root>/tmp` and merges the results.
pub fn scan_transcripts(root: &Path, bounds: &TranscriptBounds) -> ScanReport {
    let mut report = ScanReport::default();
    for path in collect_transcripts(root, bounds.max_files) {
        scan_file(&path, bounds, &mut report);
        if report.bytes_read >= bounds.max_total_bytes {
            report.truncated = true;
            break;
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn header(session_id: &str) -> String {
        format!(
            r#"{{"sessionId":"{session_id}","projectHash":"abc","startTime":"2026-05-24T06:55:16Z","kind":"main"}}"#
        )
    }

    fn msg(id: &str, ts: &str, input: u64, output: u64, thoughts: u64, tool: u64) -> String {
        let total = input + output + thoughts + tool;
        format!(
            r#"{{"id":"{id}","timestamp":"{ts}","type":"gemini","content":"","tokens":{{"input":{input},"output":{output},"cached":0,"thoughts":{thoughts},"tool":{tool},"total":{total}}},"model":"gemini-x"}}"#
        )
    }

    fn scan_text(text: &str) -> ScanReport {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        let chats = root.join("tmp").join("p").join("chats");
        std::fs::create_dir_all(&chats).expect("chats dir");
        std::fs::write(chats.join("session-a.jsonl"), text).expect("write");
        scan_transcripts(&root, &TranscriptBounds::default())
    }

    #[test]
    fn cumulative_tokens_become_per_message_deltas() {
        let report = scan_text(&format!(
            "{}\n{}\n{}\n",
            header("s"),
            msg("g1", "2026-05-24T07:00:00Z", 100, 20, 5, 0),
            msg("g2", "2026-05-24T07:01:00Z", 250, 60, 8, 2),
        ));
        assert_eq!(report.samples.len(), 1, "first message is the baseline");
        let sample = &report.samples[0];
        assert_eq!(sample.input_tokens, 150);
        assert_eq!(sample.output_tokens, 45, "output + thoughts + tool deltas");
        assert_eq!(sample.model.as_deref(), Some("gemini-x"));
        assert_eq!(report.files_seen, 1);
        assert_eq!(report.files_parsed, 1);
        assert!(!report.truncated);
    }

    #[test]
    fn counter_reset_restarts_the_baseline() {
        let report = scan_text(&format!(
            "{}\n{}\n{}\n",
            header("s"),
            msg("g1", "2026-05-24T07:00:00Z", 1000, 200, 0, 0),
            msg("g2", "2026-05-24T08:00:00Z", 300, 50, 0, 0),
        ));
        assert_eq!(report.samples.len(), 1);
        assert_eq!(report.samples[0].input_tokens, 300);
        assert_eq!(report.samples[0].output_tokens, 50);
    }

    #[test]
    fn non_token_lines_and_bookkeeping_are_ignored() {
        let report = scan_text(&format!(
            "{}\n{}\n{}\n{}\n",
            header("s"),
            r#"{"id":"u1","timestamp":"2026-05-24T07:00:00Z","type":"user","content":[{"text":"prompt"}]}"#,
            r#"{"$set":{"lastUpdated":"2026-05-24T07:00:01Z"}}"#,
            msg("g1", "2026-05-24T07:00:02Z", 10, 5, 0, 0),
        ));
        assert!(
            report.samples.is_empty(),
            "single message is only a baseline"
        );
        assert_eq!(report.files_parsed, 0);
    }

    #[test]
    fn single_document_format_is_supported() {
        let report = scan_text(&format!(
            r#"{{"sessionId":"s","messages":[{},{}]}}"#,
            msg("a", "2026-05-24T07:00:00Z", 100, 20, 0, 0),
            msg("b", "2026-05-24T07:30:00Z", 150, 40, 0, 0),
        ));
        assert_eq!(report.samples.len(), 1);
        assert_eq!(report.samples[0].input_tokens, 50);
        assert_eq!(report.samples[0].output_tokens, 20);
    }

    #[test]
    fn only_transcript_files_under_chats_are_collected() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        std::fs::create_dir_all(root.join("tmp").join("p").join("chats")).expect("dirs");
        std::fs::write(
            root.join("tmp").join("p").join("logs.json"),
            r#"[{"type":"user"}]"#,
        )
        .expect("logs");
        std::fs::write(
            root.join("tmp").join("p").join("chats").join("other.json"),
            "{}",
        )
        .expect("other");
        std::fs::write(
            root.join("tmp")
                .join("p")
                .join("chats")
                .join("session-x.jsonl"),
            format!("{}\n", msg("g1", "2026-05-24T07:00:00Z", 10, 5, 0, 0)),
        )
        .expect("session");
        let report = scan_transcripts(&root, &TranscriptBounds::default());
        assert_eq!(report.files_seen, 1, "only session-* files are scanned");
    }

    #[test]
    fn oversized_files_are_read_from_the_tail() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        let chats = root.join("tmp").join("p").join("chats");
        std::fs::create_dir_all(&chats).expect("dirs");
        let path = chats.join("session-big.jsonl");
        let mut file = std::fs::File::create(&path).expect("create");
        writeln!(file, "{}", header("s")).expect("header");
        writeln!(
            file,
            "{}",
            msg("g_old", "2026-05-24T07:00:00Z", 1_000_000, 100, 0, 0)
        )
        .expect("old message");
        for (i, (input, output)) in [(100u64, 20u64), (150, 40), (200, 60)].iter().enumerate() {
            writeln!(
                file,
                "{}",
                msg(
                    &format!("g{i}"),
                    &format!("2026-05-24T{:02}:00:00Z", 8 + i),
                    *input,
                    *output,
                    0,
                    0
                )
            )
            .expect("message");
        }
        drop(file);

        // The tail budget starts one byte inside the old message's line, so
        // the scan deterministically discards a partial `g_old` line and
        // keeps g0 (baseline), g1 and g2 (samples).
        let content = std::fs::read_to_string(&path).expect("read back");
        let g_old_offset = content.find('\n').expect("header newline") + 1;
        let bounds = TranscriptBounds {
            max_bytes_per_file: (content.len() - g_old_offset - 1) as u64,
            ..TranscriptBounds::default()
        };
        let report = scan_transcripts(&root, &bounds);
        assert!(report.truncated, "tail read must be reported");
        assert_eq!(report.samples.len(), 2, "recent messages survive");
        for sample in &report.samples {
            assert!(
                !sample.timestamp.to_rfc3339().starts_with("2026-05-24T07"),
                "only tail messages are scanned: {}",
                sample.timestamp
            );
        }
        assert_eq!(report.samples[0].input_tokens, 50);
        assert_eq!(report.samples[1].input_tokens, 50);
    }

    #[test]
    fn missing_root_scans_to_empty() {
        let report = scan_transcripts(
            Path::new("Z:/definitely/not/here"),
            &TranscriptBounds::default(),
        );
        assert!(report.is_empty());
        assert_eq!(report.files_seen, 0);
    }
}
