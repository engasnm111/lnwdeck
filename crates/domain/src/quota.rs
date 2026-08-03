use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuotaSnapshot {
    pub provider_id: String,
    pub quota_limit: u64,
    pub quota_used: u64,
    pub recorded_at: DateTime<Utc>,
}
