use inwdeck_domain::UsageBatch;
use serde_json::Value;

pub struct PrivacyGuard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyViolation {
    pub code: String,
}

impl PrivacyViolation {
    fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
        }
    }
}

impl PrivacyGuard {
    pub fn validate_usage_batch(batch: &UsageBatch) -> Result<(), PrivacyViolation> {
        let json = serde_json::to_value(batch)
            .map_err(|_| PrivacyViolation::new("SERIALIZATION_FAILED"))?;
        validate_value(&json)?;
        Ok(())
    }

    pub fn validate_raw_json(json: &Value) -> Result<(), PrivacyViolation> {
        validate_value(json)
    }
}

fn validate_value(value: &Value) -> Result<(), PrivacyViolation> {
    match value {
        Value::String(s) => validate_string(s)?,
        Value::Array(arr) => {
            for item in arr {
                validate_value(item)?;
            }
        }
        Value::Object(map) => {
            for (key, val) in map {
                validate_key(key)?;
                validate_value(val)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), PrivacyViolation> {
    let exact_forbidden: &[&str] = &[
        "prompt",
        "response",
        "path",
        "file_name",
        "cookie",
        "token",
        "api_key",
        "secret",
        "password",
        "credential",
    ];

    let lower = key.to_lowercase();
    for forbidden in exact_forbidden {
        if lower == *forbidden || lower.ends_with(&format!("_{}", forbidden)) {
            return Err(PrivacyViolation::new("FORBIDDEN_KEY"));
        }
    }
    Ok(())
}

fn validate_string(value: &str) -> Result<(), PrivacyViolation> {
    if contains_windows_path(value) {
        return Err(PrivacyViolation::new("WINDOWS_PATH_DETECTED"));
    }
    if contains_unix_path(value) {
        return Err(PrivacyViolation::new("UNIX_PATH_DETECTED"));
    }
    if contains_bearer_token(value) {
        return Err(PrivacyViolation::new("BEARER_TOKEN_DETECTED"));
    }
    if contains_api_key(value) {
        return Err(PrivacyViolation::new("API_KEY_DETECTED"));
    }
    Ok(())
}

fn contains_windows_path(s: &str) -> bool {
    let upper = s.to_uppercase();
    for c in 'A'..='Z' {
        let prefix = format!("{}:\\", c);
        if upper.contains(&prefix) {
            return true;
        }
    }
    if s.contains("\\\\") || s.contains("//") {
        return true;
    }
    false
}

fn contains_unix_path(s: &str) -> bool {
    let unix_paths = [
        "/home/", "/etc/", "/usr/", "/var/", "/tmp/", "/root/", "/opt/",
    ];
    for path in &unix_paths {
        if s.contains(path) {
            return true;
        }
    }
    false
}

fn contains_bearer_token(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("bearer") && s.len() > 20
}

fn contains_api_key(s: &str) -> bool {
    let lower = s.to_lowercase();
    let key_patterns = [
        "sk-",
        "sk_proj_",
        "key-",
        "api_key",
        "apikey",
        "secret_key",
        "access_key",
    ];
    for pattern in &key_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_windows_path() {
        assert!(contains_windows_path("C:\\Users\\test\\file.txt"));
        assert!(!contains_windows_path("Model: gpt-4o"));
    }

    #[test]
    fn detects_unix_path() {
        assert!(contains_unix_path("/home/user/config.json"));
        assert!(!contains_unix_path("Provider home directory not found"));
    }

    #[test]
    fn detects_bearer_token() {
        assert!(contains_bearer_token(
            "Authorization: Bearer abcdefghijklmnopqrstuv"
        ));
        assert!(!contains_bearer_token("token count: 150"));
    }

    #[test]
    fn detects_api_key() {
        assert!(contains_api_key("sk-proj-abc123def456"));
        assert!(!contains_api_key("task description here"));
    }
}
