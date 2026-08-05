//! Minimal HTTPS client for network provider adapters.
//!
//! Adapters that read quota from a provider API share this client so that
//! every outbound request has the same guarantees:
//!
//! - GET only, with an explicit timeout, and no redirect following.
//! - The API key is sent in the `Authorization` header and never appears in a
//!   returned error, so a failure can be recorded verbatim in diagnostics.
//! - Errors are reduced to sanitized, stable codes (`AUTH_EXPIRED`,
//!   `RATE_LIMITED`, `SOURCE_UNAVAILABLE`, ...) that the quota pipeline maps
//!   to statuses. No response body text is ever propagated.
//! - Response headers the caller asked for are returned, because several
//!   providers publish their rate limits only in headers.

use std::collections::HashMap;
use std::io::Read as _;
use std::time::Duration;

/// Successful response: parsed JSON plus the requested headers.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: HashMap<String, String>,
}

impl HttpResponse {
    /// Reads a header value that was requested by name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(|s| s.as_str())
    }

    /// Reads a header as an unsigned integer.
    pub fn header_u64(&self, name: &str) -> Option<u64> {
        self.header(name)?.trim().parse().ok()
    }

    /// Reads a header as a floating point number.
    pub fn header_f64(&self, name: &str) -> Option<f64> {
        self.header(name)?.trim().parse().ok()
    }
}

/// A GET request against a provider API.
pub struct JsonRequest<'a> {
    pub url: &'a str,
    pub bearer_token: Option<&'a str>,
    pub timeout: Duration,
    /// Header names to return with the response, lowercase.
    pub capture_headers: &'a [&'a str],
    /// Extra request headers, for provider APIs that require an opt-in header.
    /// Never used for credentials: those go through `bearer_token`.
    pub extra_headers: &'a [(&'a str, &'a str)],
}

impl<'a> JsonRequest<'a> {
    pub fn new(url: &'a str) -> Self {
        Self {
            url,
            bearer_token: None,
            timeout: Duration::from_secs(10),
            capture_headers: &[],
            extra_headers: &[],
        }
    }

    pub fn bearer(mut self, token: &'a str) -> Self {
        self.bearer_token = Some(token);
        self
    }

    pub fn capture(mut self, headers: &'a [&'a str]) -> Self {
        self.capture_headers = headers;
        self
    }

    pub fn with_headers(mut self, headers: &'a [(&'a str, &'a str)]) -> Self {
        self.extra_headers = headers;
        self
    }
}

/// Maps an HTTP status code to a sanitized collector error code.
pub fn code_for_status(status: u16) -> &'static str {
    match status {
        401 | 403 => "AUTH_EXPIRED",
        429 => "RATE_LIMITED",
        400 | 404 | 405 | 422 => "SOURCE_SCHEMA_MISMATCH",
        500..=599 => "PROVIDER_UNAVAILABLE",
        _ => "PROVIDER_ERROR",
    }
}

/// Performs the request and parses the JSON body.
///
/// The returned `Err` is always a sanitized code: no URL, no key, no response
/// text, so callers may record it directly.
pub fn get_json(request: JsonRequest<'_>) -> Result<HttpResponse, String> {
    if !request.url.starts_with("https://") {
        return Err("INSECURE_ENDPOINT".to_string());
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .max_redirects(0)
        .build()
        .new_agent();

    let mut call = agent.get(request.url).header("accept", "application/json");
    for (name, value) in request.extra_headers {
        call = call.header(*name, *value);
    }
    if let Some(token) = request.bearer_token {
        if token.trim().is_empty() {
            return Err("NOT_CONFIGURED".to_string());
        }
        call = call.header("authorization", &format!("Bearer {token}"));
    }

    let mut response = match call.call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => return Err(code_for_status(status).to_string()),
        Err(ureq::Error::Timeout(_)) => return Err("PROVIDER_TIMEOUT".to_string()),
        Err(_) => return Err("SOURCE_UNAVAILABLE".to_string()),
    };

    let status = response.status().as_u16();
    let mut headers = HashMap::new();
    for name in request.capture_headers {
        if let Some(value) = response.headers().get(*name).and_then(|v| v.to_str().ok()) {
            headers.insert(name.to_ascii_lowercase(), value.to_string());
        }
    }

    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|_| "PROVIDER_UNREADABLE_BODY".to_string())?;
    let body = serde_json::from_str(&text).map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;

    Ok(HttpResponse {
        status,
        body,
        headers,
    })
}

/// Performs the request and returns the raw body text.
///
/// Several provider APIs (Cursor's usage export) return CSV instead of JSON.
/// The request and error-contract are identical to [`get_json`]: GET only,
/// explicit timeout, no redirects, and every failure is a sanitized code.
pub fn get_text(request: JsonRequest<'_>) -> Result<(u16, String), String> {
    if !request.url.starts_with("https://") {
        return Err("INSECURE_ENDPOINT".to_string());
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .max_redirects(0)
        .build()
        .new_agent();

    let mut call = agent.get(request.url).header("accept", "application/json");
    for (name, value) in request.extra_headers {
        call = call.header(*name, *value);
    }
    if let Some(token) = request.bearer_token {
        if token.trim().is_empty() {
            return Err("NOT_CONFIGURED".to_string());
        }
        call = call.header("authorization", &format!("Bearer {token}"));
    }

    let mut response = match call.call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => return Err(code_for_status(status).to_string()),
        Err(ureq::Error::Timeout(_)) => return Err("PROVIDER_TIMEOUT".to_string()),
        Err(_) => return Err("SOURCE_UNAVAILABLE".to_string()),
    };

    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|_| "PROVIDER_UNREADABLE_BODY".to_string())?;
    Ok((status, text))
}

/// Upper bound for a binary response body (pet packages: 12 MB).
pub const MAX_BINARY_BYTES: usize = 12 * 1024 * 1024;

/// Performs the request and returns the raw body bytes.
///
/// The request and error contract are identical to [`get_json`]: GET only,
/// HTTPS only, explicit timeout, no redirects, and every failure is a
/// sanitized code. The body is size-capped, so a hostile or broken endpoint
/// cannot exhaust memory.
pub fn get_bytes(request: JsonRequest<'_>) -> Result<(u16, Vec<u8>), String> {
    if !request.url.starts_with("https://") {
        return Err("INSECURE_ENDPOINT".to_string());
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .max_redirects(0)
        .build()
        .new_agent();

    let mut call = agent
        .get(request.url)
        .header("accept", "application/octet-stream");
    for (name, value) in request.extra_headers {
        call = call.header(*name, *value);
    }
    if let Some(token) = request.bearer_token {
        if token.trim().is_empty() {
            return Err("NOT_CONFIGURED".to_string());
        }
        call = call.header("authorization", &format!("Bearer {token}"));
    }

    let mut response = match call.call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => return Err(code_for_status(status).to_string()),
        Err(ureq::Error::Timeout(_)) => return Err("PROVIDER_TIMEOUT".to_string()),
        Err(_) => return Err("SOURCE_UNAVAILABLE".to_string()),
    };

    let status = response.status().as_u16();
    // Read one byte past the cap to distinguish "too large" from "exact size".
    let mut reader = response.body_mut().as_reader();
    let mut bytes = Vec::new();
    std::io::Read::take(&mut reader, MAX_BINARY_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "PROVIDER_UNREADABLE_BODY".to_string())?;
    if bytes.len() > MAX_BINARY_BYTES {
        return Err("PROVIDER_RESPONSE_TOO_LARGE".to_string());
    }
    Ok((status, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_map_to_stable_error_codes() {
        assert_eq!(code_for_status(401), "AUTH_EXPIRED");
        assert_eq!(code_for_status(403), "AUTH_EXPIRED");
        assert_eq!(code_for_status(429), "RATE_LIMITED");
        assert_eq!(code_for_status(404), "SOURCE_SCHEMA_MISMATCH");
        assert_eq!(code_for_status(503), "PROVIDER_UNAVAILABLE");
        assert_eq!(code_for_status(418), "PROVIDER_ERROR");
    }

    #[test]
    fn extra_headers_are_carried_on_the_request() {
        let headers = [("anthropic-beta", "oauth-2025-04-20")];
        let request = JsonRequest::new("https://example.invalid/api").with_headers(&headers);
        assert_eq!(request.extra_headers.len(), 1);
        assert_eq!(request.extra_headers[0].0, "anthropic-beta");
    }

    #[test]
    fn plain_http_endpoints_are_refused() {
        let error = get_json(JsonRequest::new("http://example.com/api")).expect_err("must refuse");
        assert_eq!(error, "INSECURE_ENDPOINT");
    }

    #[test]
    fn get_bytes_refuses_plain_http_endpoints() {
        let error = get_bytes(JsonRequest::new("http://example.com/api")).expect_err("must refuse");
        assert_eq!(error, "INSECURE_ENDPOINT");
    }

    #[test]
    fn get_bytes_refuses_an_empty_token() {
        let error = get_bytes(JsonRequest::new("https://example.invalid/api").bearer("  "))
            .expect_err("must refuse");
        assert_eq!(error, "NOT_CONFIGURED");
    }

    #[test]
    fn empty_token_is_reported_as_not_configured() {
        let error = get_json(JsonRequest::new("https://example.invalid/api").bearer("   "))
            .expect_err("must refuse");
        assert_eq!(error, "NOT_CONFIGURED");
    }

    #[test]
    fn unreachable_host_is_reported_as_source_unavailable() {
        let error = get_json(
            JsonRequest {
                timeout: Duration::from_millis(1500),
                ..JsonRequest::new("https://lnwdeck-nonexistent-host.invalid/api")
            }
            .bearer("token"),
        )
        .expect_err("must fail");
        assert!(
            error == "SOURCE_UNAVAILABLE" || error == "PROVIDER_TIMEOUT",
            "unexpected error: {error}"
        );
    }

    #[test]
    fn response_headers_are_parsed_as_numbers() {
        let response = HttpResponse {
            status: 200,
            body: serde_json::json!({}),
            headers: HashMap::from([
                ("x-ratelimit-limit-requests".to_string(), "480".to_string()),
                ("x-credits".to_string(), "1.25".to_string()),
                ("x-bad".to_string(), "n/a".to_string()),
            ]),
        };
        assert_eq!(response.header_u64("X-RateLimit-Limit-Requests"), Some(480));
        assert_eq!(response.header_f64("x-credits"), Some(1.25));
        assert_eq!(response.header_u64("x-bad"), None);
        assert_eq!(response.header("missing"), None);
    }
}
