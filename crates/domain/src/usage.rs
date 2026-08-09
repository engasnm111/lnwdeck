use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Confidence;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UsageEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub provider_id: String,
    pub model: String,
    pub tokens_input: u64,
    /// Non-cached input tokens. Cached input is kept separately so totals and
    /// pricing never double-count the same prompt bytes.
    #[serde(default)]
    pub tokens_cached: u64,
    /// Input tokens written into the provider cache, when the source exposes
    /// that breakdown.
    #[serde(default)]
    pub tokens_cache_write: u64,
    pub tokens_output: u64,
    /// Reasoning tokens are a provider-reported subset of output tokens.
    #[serde(default)]
    pub tokens_reasoning: u64,
    pub confidence: Confidence,
    pub data_source: String,
    pub cost: String,
    /// Keyed hash of the source's raw session identifier. `None` when the
    /// source does not expose a session. Raw identifiers never leave the
    /// adapter; only the hash is persisted.
    #[serde(default)]
    pub session_hash: Option<String>,
    /// Keyed hash of the source's raw project/folder identifier. `None` when
    /// the source does not expose a project. The raw path is never persisted.
    #[serde(default)]
    pub project_hash: Option<String>,
    /// Keyed hash of the provider account identity. Reports collected from
    /// the same account in different runtimes share this value; different
    /// accounts are kept separate without exposing the raw credential.
    #[serde(default)]
    pub account_fingerprint: Option<String>,
}

impl UsageEvent {
    pub fn new_sample() -> Self {
        Self {
            id: "evt_001".to_string(),
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            provider_id: "prov_openai".to_string(),
            model: "gpt-4o".to_string(),
            tokens_input: 150,
            tokens_cached: 0,
            tokens_cache_write: 0,
            tokens_output: 80,
            tokens_reasoning: 0,
            confidence: Confidence::High,
            data_source: "web".to_string(),
            cost: "0.0025".to_string(),
            session_hash: None,
            project_hash: None,
            account_fingerprint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UsageBatch {
    pub events: Vec<UsageEvent>,
    pub batch_id: String,
}
