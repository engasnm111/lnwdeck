use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderDescriptor {
    pub id: String,
    pub name: String,
    pub adapter_id: String,
    pub enabled: bool,
}
