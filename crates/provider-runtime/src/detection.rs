use crate::adapter::NOT_SUPPORTED;
use crate::descriptor::AdapterDescriptor;
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
    /// Detection evidence for an adapter that has no detectable local source
    /// yet, derived from its descriptor so the recorded id, name, version and
    /// source type always match the declaration.
    ///
    /// The `detection_error_code` states the reason explicitly, so the System
    /// page can distinguish "not implemented" and "waiting for credentials"
    /// from a real failure.
    pub fn from_descriptor(descriptor: &AdapterDescriptor) -> Self {
        let (method, error_code, permission_state) = if descriptor.is_inert() {
            ("unsupported", NOT_SUPPORTED, "n/a")
        } else if descriptor.needs_credentials() {
            ("credential", "NOT_CONFIGURED", "credential_required")
        } else {
            ("unsupported", NOT_SUPPORTED, "n/a")
        };
        Self {
            provider_id: descriptor.id.to_string(),
            display_name: descriptor.display_name.to_string(),
            enabled: true,
            detected: false,
            detection_method: method.to_string(),
            source_type: descriptor.source_kind.label().to_string(),
            source_exists: false,
            permission_state: permission_state.to_string(),
            adapter_version: descriptor.adapter_version.to_string(),
            last_detection_at: Some(chrono::Utc::now().to_rfc3339()),
            detection_error_code: error_code.to_string(),
        }
    }
}
