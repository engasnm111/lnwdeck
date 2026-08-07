//! Shared parser for Cline-derived `ui_messages.json` task logs.
//!
//! Kilo Code and Roo Code (both Cline forks) persist one `ui_messages.json`
//! per task under `User/globalStorage/<extension>/tasks/<uuid>/`. Each record
//! is `{"say": "api_req_started", "text": "{...}", "ts": <epoch ms>}`; the
//! `text` field is a JSON payload carrying `tokensIn`, `tokensOut`,
//! `cacheReads`, `cacheWrites` and the provider that served the turn.
//!
//! This module turns those records into [`TokenSample`]s. Only numbers,
//! timestamps and model identifiers are extracted; prompts and responses
//! never leave the log. `api_req_deleted` records keep the same payload after
//! a user removes a turn (Cline-style edit-and-retry), and the tokens were
//! already consumed, so they are counted too. Records with zero tokens are
//! skipped, because `api_req_started` is written at request start with zeroes
//! and back-filled in place on completion.

use crate::token_scan::TokenSample;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Number of `ui_messages.json` records read per file before giving up, so a
/// pathological history cannot stall collection.
const MAX_RECORDS: usize = 200_000;

/// Discovers `ui_messages.json` task files for one VS Code extension across
/// every configured IDE root (`<root>/User/globalStorage/<extension>/tasks/
/// <uuid>/ui_messages.json`). Missing roots are skipped.
pub fn task_files(roots: &[PathBuf], extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots {
        let tasks_dir = root
            .join("User")
            .join("globalStorage")
            .join(extension)
            .join("tasks");
        let Ok(entries) = std::fs::read_dir(&tasks_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path().join("ui_messages.json");
            if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Extracts token samples from a task's `ui_messages.json` content.
///
/// `model_for` maps a parsed payload to the model label (Kilo Code derives it
/// from the provider, Roo Code from its `api_conversation_history.json`).
pub fn samples_from_content(
    content: &str,
    model_for: &dyn Fn(&Value) -> Option<String>,
) -> Vec<TokenSample> {
    let Ok(records) = serde_json::from_str::<Value>(content) else {
        return Vec::new();
    };
    let Some(messages) = records.as_array() else {
        return Vec::new();
    };
    let mut samples = Vec::new();
    for message in messages.iter().take(MAX_RECORDS) {
        let Some(say) = message.get("say").and_then(Value::as_str) else {
            continue;
        };
        if say != "api_req_started" && say != "api_req_deleted" {
            continue;
        }
        let Some(text) = message.get("text").and_then(Value::as_str) else {
            continue;
        };
        let Some(ts) = message.get("ts").and_then(Value::as_i64) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        let input = u64_field(&payload, "tokensIn").unwrap_or(0);
        let output = u64_field(&payload, "tokensOut").unwrap_or(0);
        let cache_read = u64_field(&payload, "cacheReads").unwrap_or(0);
        let cache_write = u64_field(&payload, "cacheWrites").unwrap_or(0);
        if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
            continue;
        }
        let Some(timestamp) = chrono::DateTime::from_timestamp_millis(ts) else {
            continue;
        };
        samples.push(TokenSample {
            timestamp,
            input_tokens: input.saturating_add(cache_read).saturating_add(cache_write),
            output_tokens: output,
            model: model_for(&payload),
        });
    }
    samples
}

/// Reads one `ui_messages.json` file (bounded to `max_file_bytes`).
pub fn read_file(path: &Path, max_file_bytes: u64) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|_| "SOURCE_UNAVAILABLE")?;
    if meta.len() > max_file_bytes {
        return Err("FILE_TOO_LARGE".to_string());
    }
    std::fs::read_to_string(path).map_err(|_| "SOURCE_UNAVAILABLE".to_string())
}

fn u64_field(payload: &Value, key: &str) -> Option<u64> {
    payload.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64(),
        _ => None,
    })
}

/// The model label Kilo Code uses: the inference provider, never a guessed
/// model id, because only the provider is persisted per turn.
pub fn provider_label(provider: Option<&str>) -> Option<String> {
    let provider = provider?.trim();
    if provider.is_empty() {
        return None;
    }
    let slug: String = provider
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if slug.is_empty() || !slug.chars().any(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(format!("provider:{slug}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(content: &str) -> Vec<TokenSample> {
        samples_from_content(content, &|payload| {
            provider_label(payload.get("inferenceProvider").and_then(Value::as_str))
        })
    }

    #[test]
    fn counts_billed_api_requests() {
        let content = r#"[
            {"say":"ask_user","text":"hello","ts":1700000000000},
            {"say":"api_req_started","ts":1700000001000,"text":"{\"tokensIn\":120,\"tokensOut\":30,\"cacheReads\":10,\"cacheWrites\":2,\"inferenceProvider\":\"Moonshot AI\"}"},
            {"say":"api_req_deleted","ts":1700000002000,"text":"{\"tokensIn\":50,\"tokensOut\":20,\"cacheReads\":0,\"cacheWrites\":0,\"inferenceProvider\":\"minimax\"}"}
        ]"#;
        let samples = count(content);
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].input_tokens, 132);
        assert_eq!(samples[0].output_tokens, 30);
        assert_eq!(samples[0].model.as_deref(), Some("provider:moonshot-ai"));
        assert_eq!(samples[1].model.as_deref(), Some("provider:minimax"));
        assert_eq!(samples[1].timestamp.timestamp_millis(), 1_700_000_002_000);
    }

    #[test]
    fn zero_token_placeholders_are_skipped() {
        let content = r#"[
            {"say":"api_req_started","ts":1700000000000,"text":"{\"tokensIn\":0,\"tokensOut\":0,\"cacheReads\":0,\"cacheWrites\":0}"}
        ]"#;
        assert!(
            count(content).is_empty(),
            "in-flight placeholder must not count"
        );
    }

    #[test]
    fn malformed_records_are_ignored() {
        let content = r#"[
            {"say":"api_req_started","ts":1700000000000,"text":"not json"},
            {"say":"api_req_started","ts":"nope","text":"{\"tokensIn\":1}"},
            "junk"
        ]"#;
        assert!(count(content).is_empty());
    }

    #[test]
    fn provider_label_slugs_are_safe() {
        assert_eq!(
            provider_label(Some("Moonshot AI")).as_deref(),
            Some("provider:moonshot-ai")
        );
        assert_eq!(provider_label(Some("   ")), None);
        assert_eq!(provider_label(Some("---")), None);
        assert_eq!(provider_label(None), None);
    }

    #[test]
    fn model_callback_can_override_the_provider_label() {
        let content = r#"[{"say":"api_req_started","ts":1700000000000,"text":"{\"tokensIn\":1,\"tokensOut\":1}"}]"#;
        let samples = samples_from_content(content, &|_| Some("claude-3-7-sonnet".to_string()));
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].model.as_deref(), Some("claude-3-7-sonnet"));
    }
}
