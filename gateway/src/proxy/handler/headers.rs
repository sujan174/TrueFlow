use axum::http::HeaderMap;

/// SEC-03 FIX: Headers that must be redacted in audit logs (credential-bearing).
pub(crate) const REDACTED_HEADER_NAMES: &[&str] = &[
    "authorization",
    "x-admin-key",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-real-authorization",
    "x-upstream-authorization",
    "x-trueflow-auth",   // BYOK: virtual token header
    "x-tf-real-auth",    // BYOK: real API key header
    "proxy-authorization",
];

/// Returns true if a header name should be redacted in audit logs.
pub(crate) fn is_sensitive_header(name: &str) -> bool {
    REDACTED_HEADER_NAMES
        .iter()
        .any(|h| name.eq_ignore_ascii_case(h))
}

/// Convert axum HeaderMap to JSON object for Level 2 logging.
pub(crate) fn headers_to_json(headers: &HeaderMap) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            if is_sensitive_header(key.as_str()) {
                map.insert(key.to_string(), serde_json::json!("[REDACTED]"));
            } else {
                map.insert(key.to_string(), serde_json::json!(v));
            }
        }
    }
    serde_json::Value::Object(map)
}

/// Convert reqwest HeaderMap to JSON object for Level 2 logging.
/// SEC-03 FIX: Also redacts credential-bearing response headers.
pub(crate) fn headers_to_json_reqwest(headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            if is_sensitive_header(key.as_str()) {
                map.insert(key.to_string(), serde_json::json!("[REDACTED]"));
            } else {
                map.insert(key.to_string(), serde_json::json!(v));
            }
        }
    }
    serde_json::Value::Object(map)
}

// ── Unit Tests ──────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    // ── Header redaction: is_sensitive_header ──────────────────────────

    #[test]
    fn test_header_redaction_sensitive() {
        assert!(is_sensitive_header("authorization"));
        assert!(is_sensitive_header("Authorization"));
        assert!(is_sensitive_header("AUTHORIZATION"));
        assert!(is_sensitive_header("cookie"));
        assert!(is_sensitive_header("Cookie"));
        assert!(is_sensitive_header("set-cookie"));
        assert!(is_sensitive_header("x-admin-key"));
        assert!(is_sensitive_header("X-Admin-Key"));
        assert!(is_sensitive_header("x-api-key"));
        assert!(is_sensitive_header("proxy-authorization"));
        assert!(is_sensitive_header("x-real-authorization"));
        assert!(is_sensitive_header("x-upstream-authorization"));
        assert!(is_sensitive_header("x-trueflow-auth"));
        assert!(is_sensitive_header("X-TrueFlow-Auth"));
        assert!(is_sensitive_header("x-tf-real-auth"));
        assert!(is_sensitive_header("X-TF-Real-Auth"));
    }

    #[test]
    fn test_header_redaction_passes_normal() {
        assert!(!is_sensitive_header("content-type"));
        assert!(!is_sensitive_header("x-request-id"));
        assert!(!is_sensitive_header("accept"));
        assert!(!is_sensitive_header("user-agent"));
        assert!(!is_sensitive_header("x-session-id"));
    }
}
