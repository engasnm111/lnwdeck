use serde::Serialize;

/// Sanitized provider detection evidence. Never contains paths, file names,
/// credentials, or any forbidden content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetectionResult {
    pub provider_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub detected: bool,
    pub detection_method: String,
    pub source_type: String,
    pub source_exists: bool,
    pub permission_state: String,
    pub adapter_version: String,
    pub last_detection_at: Option<String>,
    pub detection_error_code: String,
}

impl DetectionResult {
    /// Result for adapters that do not implement detection yet.
    pub fn unsupported(provider_id: &str, display_name: &str) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            display_name: display_name.to_string(),
            enabled: true,
            detected: false,
            detection_method: "unsupported".to_string(),
            source_type: String::new(),
            source_exists: false,
            permission_state: "n/a".to_string(),
            adapter_version: "0.1.0".to_string(),
            last_detection_at: None,
            detection_error_code: String::new(),
        }
    }
}
