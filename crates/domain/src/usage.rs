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
    pub tokens_output: u64,
    pub confidence: Confidence,
    pub data_source: String,
    pub cost: String,
}

impl UsageEvent {
    pub fn new_sample() -> Self {
        Self {
            id: "evt_001".to_string(),
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            provider_id: "prov_openai".to_string(),
            model: "gpt-4o".to_string(),
            tokens_input: 150,
            tokens_output: 80,
            confidence: Confidence::High,
            data_source: "web".to_string(),
            cost: "0.0025".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UsageBatch {
    pub events: Vec<UsageEvent>,
    pub batch_id: String,
}
