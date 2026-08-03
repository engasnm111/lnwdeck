#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CredentialState {
    Missing,
    Configured,
    Expired,
}

impl CredentialState {
    pub fn is_secret_exposed(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CredentialStatus {
    pub provider: String,
    pub state: CredentialState,
}

impl CredentialStatus {
    pub fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
            state: CredentialState::Missing,
        }
    }

    pub fn configured(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
            state: CredentialState::Configured,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_state_never_exposes_secret() {
        assert!(!CredentialState::Missing.is_secret_exposed());
        assert!(!CredentialState::Configured.is_secret_exposed());
        assert!(!CredentialState::Expired.is_secret_exposed());
    }

    #[test]
    fn serialized_credential_has_no_secret_field() {
        let status = CredentialStatus::configured("openai");
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("key"));
        assert!(!json.contains("token"));
        assert!(!json.contains("password"));
    }

    #[test]
    fn credential_state_deserializes_correctly() {
        let json = r#"{"provider":"openai","state":"Configured"}"#;
        let status: CredentialStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.provider, "openai");
        assert_eq!(status.state, CredentialState::Configured);
    }

    #[test]
    fn all_states_are_distinct() {
        assert_ne!(CredentialState::Missing, CredentialState::Configured);
        assert_ne!(CredentialState::Configured, CredentialState::Expired);
        assert_ne!(CredentialState::Missing, CredentialState::Expired);
    }
}
