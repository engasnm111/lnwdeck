//! Bounded reader for Antigravity session transcripts.
//!
//! Antigravity (the Gemini IDE) keeps one step-event transcript per session
//! under `~/.gemini/{antigravity,antigravity-ide,antigravity-cli}/brain/
//! <session>/.system_generated/logs/transcript.jsonl`. The transcripts carry
//! no token counters, so usage is estimated from content length the same way
//! the reference implementations do: one token per CJK character and one per
//! four other characters. Every event contributes its content to the running
//! context total; a `PLANNER_RESPONSE` bills the context delta as input and
//! its own content plus tool calls as output, with `thinking` counted as
//! reasoning. Only timestamps, model identifiers and estimated counts leave
//! the source โ€” never prompts, responses or tool payloads.

use chrono::{DateTime, Utc};
use lnwdeck_provider_runtime::token_scan::TokenSample;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// The three Antigravity roots under `~/.gemini`.
const BRAIN_DIRS: [&str; 3] = ["antigravity", "antigravity-ide", "antigravity-cli"];

/// Estimates tokens from text: one per CJK character, one per four others.
fn estimate_tokens(text: &str) -> u64 {
    let mut cjk: u64 = 0;
    let mut other: u64 = 0;
    for ch in text.chars() {
        let code = ch as u32;
        if (0x3400..=0x4dbf).contains(&code)
            || (0x4e00..=0x9fff).contains(&code)
            || (0x3040..=0x30ff).contains(&code)
        {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    cjk + other.div_ceil(4)
}

fn value_tokens(value: &Value) -> u64 {
    match value {
        Value::String(text) => estimate_tokens(text),
        Value::Null => 0,
        other => estimate_tokens(&other.to_string()),
    }
}

/// Collects Antigravity transcripts under `<root>/<dir>/brain/**/logs/`.
fn collect_transcripts(root: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir_name in BRAIN_DIRS {
        let brain = root.join(dir_name).join("brain");
        let Ok(sessions) = std::fs::read_dir(&brain) else {
            continue;
        };
        for session in sessions.flatten() {
            if !session.path().is_dir() {
                continue;
            }
            let transcript = session
                .path()
                .join(".system_generated")
                .join("logs")
                .join("transcript.jsonl");
            if transcript.is_file() {
                found.push(transcript);
                if found.len() >= max_files {
                    return found;
                }
            }
        }
    }
    found
}

/// Reads the model the user selected, or None when the transcript never
/// mentions a model selection.
fn parse_model_selection(content: &str) -> Option<String> {
    let marker = "changed setting `Model Selection` from ";
    let start = content.find(marker)? + marker.len();
    let rest = &content[start..];
    let after_to = rest.find(" to ")? + " to ".len();
    let tail = &rest[after_to..];
    let end = tail.find(['`', '(', '\n']).unwrap_or(tail.len());
    let raw = tail[..end].trim();
    if raw.is_empty() {
        return None;
    }
    let slug = raw
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric() && c != '.', "-")
        .trim_matches('-')
        .to_string();
    for marker in ["gemini", "claude", "gpt"] {
        if let Some(idx) = slug.find(marker) {
            return Some(slug[idx..].to_string());
        }
    }
    Some(format!("antigravity-{slug}"))
}

/// Default model recorded next to the transcript, when present.
fn default_model(transcript: &Path) -> Option<String> {
    let meta = transcript
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent());
    let meta = meta?;
    let candidates = [meta.join("default_model"), meta.join("model.txt")];
    for path in candidates {
        let text = std::fs::read_to_string(path).ok()?;
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Scans one Antigravity transcript and appends estimated usage samples.
fn scan_file(transcript: &Path, out: &mut Vec<TokenSample>) {
    let file = match File::open(transcript) {
        Ok(file) => file,
        Err(_) => return,
    };
    let mut model = default_model(transcript);
    let mut context_tokens: u64 = 0;
    let mut previous_context: u64 = 0;

    let reader = BufReader::new(file);
    for line in reader.lines() {
        let Ok(line) = line else {
            return;
        };
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(created_at) = event.get("created_at").and_then(|v| v.as_str()) else {
            continue;
        };
        let Ok(timestamp) = DateTime::parse_from_rfc3339(created_at) else {
            continue;
        };
        let timestamp = timestamp.with_timezone(&Utc);
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if event_type == "USER_INPUT" || event_type == "USER_SETTINGS_CHANGE" {
            if let Some(content) = event.get("content").and_then(|v| v.as_str()) {
                if let Some(selected) = parse_model_selection(content) {
                    model = Some(selected);
                }
            }
        }

        let event_tokens = value_tokens(event.get("content").unwrap_or(&Value::Null))
            + if event_type == "PLANNER_RESPONSE" {
                value_tokens(event.get("tool_calls").unwrap_or(&Value::Null))
            } else {
                0
            };

        if event_type == "PLANNER_RESPONSE" {
            let input_delta = context_tokens.saturating_sub(previous_context);
            let output = value_tokens(event.get("content").unwrap_or(&Value::Null))
                + value_tokens(event.get("tool_calls").unwrap_or(&Value::Null));
            let reasoning = value_tokens(event.get("thinking").unwrap_or(&Value::Null));
            let total = input_delta + output + reasoning;
            if total > 0 {
                out.push(TokenSample {
                    timestamp,
                    input_tokens: input_delta,
                    output_tokens: output + reasoning,
                    model: model.clone(),
                });
                previous_context = context_tokens;
            }
        }
        context_tokens += event_tokens;
    }
}

/// Scans every Antigravity transcript under `root` into the report.
pub fn scan_antigravity(root: &Path, max_files: usize, out: &mut Vec<TokenSample>) {
    for transcript in collect_transcripts(root, max_files) {
        scan_file(&transcript, out);
    }
    out.sort_by_key(|sample| sample.timestamp);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_transcript(root: &Path, session: &str, lines: &[&str]) -> PathBuf {
        let logs = root
            .join("antigravity")
            .join("brain")
            .join(session)
            .join(".system_generated")
            .join("logs");
        std::fs::create_dir_all(&logs).expect("create logs dir");
        let path = logs.join("transcript.jsonl");
        let mut file = std::fs::File::create(&path).expect("create transcript");
        for line in lines {
            writeln!(file, "{line}").expect("write line");
        }
        path
    }

    fn planner(step: u32, ts: &str, content: &str, thinking: &str) -> String {
        format!(
            r#"{{"step_index":{step},"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"{ts}","thinking":"{thinking}","content":"{content}","tool_calls":[]}}"#
        )
    }

    fn user_input(ts: &str, content: &str) -> String {
        format!(
            r#"{{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"{ts}","content":"{content}"}}"#
        )
    }

    #[test]
    fn estimates_usage_from_planner_responses() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_transcript(
            &root,
            "sess-1",
            &[
                &user_input("2026-08-06T03:32:10Z", "some user request"),
                &planner(1, "2026-08-06T03:32:11Z", "response one", "thinking one"),
                &planner(2, "2026-08-06T03:32:13Z", "response two", "thinking two"),
            ],
        );

        let mut samples = Vec::new();
        scan_antigravity(&root, 200, &mut samples);
        assert_eq!(samples.len(), 2, "each planner response bills usage");
        let first = &samples[0];
        // Input = context delta before the first planner (the user request);
        // output = response + tool_calls + thinking, all estimated.
        assert_eq!(first.input_tokens, estimate_tokens("some user request"));
        assert_eq!(
            first.output_tokens,
            estimate_tokens("response one")
                + estimate_tokens("[]")
                + estimate_tokens("thinking one")
        );
        let second = &samples[1];
        assert_eq!(
            second.input_tokens,
            estimate_tokens("response one") + estimate_tokens("[]"),
            "the previous planner output (content + tool calls) becomes the next turn's context"
        );
    }

    #[test]
    fn cjk_counts_one_token_per_character() {
        assert_eq!(estimate_tokens("ไฝ ๅฅฝ"), 2);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("ab"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn model_selection_is_picked_up_from_user_settings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(".gemini");
        write_transcript(
            &root,
            "sess-2",
            &[
                r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_SETTINGS_CHANGE","status":"DONE","created_at":"2026-08-06T03:32:10Z","content":"changed setting `Model Selection` from Gemini 2.5 Flash to Gemini 2.5 Pro (thinking)."}"#,
                &planner(1, "2026-08-06T03:32:11Z", "hello", "hmm"),
            ],
        );
        let mut samples = Vec::new();
        scan_antigravity(&root, 200, &mut samples);
        assert_eq!(samples.len(), 1);
        assert_eq!(
            samples[0].model.as_deref(),
            Some("gemini-2.5-pro"),
            "model selection is normalized to a slug"
        );
    }

    #[test]
    fn missing_transcripts_produce_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut samples = Vec::new();
        scan_antigravity(&dir.path().join(".gemini"), 200, &mut samples);
        assert!(samples.is_empty());
    }
}
