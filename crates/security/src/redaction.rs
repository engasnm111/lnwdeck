pub struct Redactor;

impl Redactor {
    pub fn sanitize(input: &str) -> String {
        let mut output = input.to_string();

        output = redact_bearer_tokens(&output);
        output = redact_windows_paths(&output);
        output = redact_api_keys(&output);

        output
    }
}

fn redact_bearer_tokens(s: &str) -> String {
    let mut result = s.to_string();
    let lower = s.to_lowercase();
    if let Some(pos) = lower.find("bearer") {
        let after_bearer = &s[pos..];
        if let Some(space_pos) = after_bearer.find(' ') {
            let token_start = pos + space_pos + 1;
            let token_end = s[token_start..]
                .find(|c: char| c.is_whitespace() || c == ',')
                .map(|p| token_start + p)
                .unwrap_or(s.len());
            result.replace_range(pos..token_end, "[REDACTED]");
        } else {
            result.replace_range(pos.., "[REDACTED]");
        }
    }
    result
}

fn redact_windows_paths(s: &str) -> String {
    let mut result = s.to_string();
    let upper = s.to_uppercase();
    for c in 'A'..='Z' {
        let prefix = format!("{}:\\", c);
        if let Some(pos) = upper.find(&prefix) {
            let path_start = pos;
            let path_end = s[path_start..]
                .find(|ch: char| ch.is_whitespace() || ch == ',' || ch == '"')
                .map(|p| path_start + p)
                .unwrap_or(s.len());
            result.replace_range(path_start..path_end, "[REDACTED]");
            break;
        }
    }
    result
}

fn redact_api_keys(s: &str) -> String {
    let mut result = s.to_string();
    let lower = s.to_lowercase();
    for pattern in &["sk-", "sk_proj_", "key-"] {
        if let Some(pos) = lower.find(pattern) {
            let value_start = pos;
            let value_end = s[value_start..]
                .find(|ch: char| ch.is_whitespace() || ch == ',' || ch == '"' || ch == '\'')
                .map(|p| value_start + p)
                .unwrap_or(s.len());
            result.replace_range(value_start..value_end, "[REDACTED]");
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_in_header() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.token";
        let output = Redactor::sanitize(input);
        assert!(!output.contains("eyJ"));
    }

    #[test]
    fn redacts_windows_path() {
        let input = "config at C:\\Users\\name\\app.json loaded";
        let output = Redactor::sanitize(input);
        assert!(!output.contains("C:\\Users"));
    }

    #[test]
    fn preserves_model_name() {
        let input = "model: gpt-4o, tokens: 100";
        let output = Redactor::sanitize(input);
        assert_eq!(output, input);
    }

    #[test]
    fn redacts_sk_key() {
        let input = "API key: sk-abc123def456ghijklmnop";
        let output = Redactor::sanitize(input);
        assert!(!output.contains("sk-abc"));
    }
}
