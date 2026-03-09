use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

// Regex patterns for PII detection
static EMAIL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap());

static CREDIT_CARD_REGEX: Lazy<Regex> = Lazy::new(|| {
    // BUG-04 FIX: Two alternatives:
    //   1) Groups of 4 digits with required separators (space or dash)
    //   2) Exactly 15-16 contiguous digits (Amex=15, Visa/MC=16)
    Regex::new(r"\b(?:\d{4}[ -]){3}\d{1,7}\b|\b\d{15,16}\b").unwrap()
});

static SSN_REGEX: Lazy<Regex> = Lazy::new(|| {
    // SEC 3C-4 FIX: Match both common SSN formats:
    //   Format 1: 123-45-6789 (dashed — standard)
    //   Format 2: 123456789   (9 contiguous digits — common in exports/forms)
    // NOTE: The Rust `regex` crate does not support lookaheads — plain \b\d{9}\b used.
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b|\b\d{9}\b").unwrap()
});

static API_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Matches common sk- patterns (OpenAI, Stripe, etc)
    Regex::new(r"(?i)\b(sk-[a-zA-Z0-9_\-\.]{20,})\b").unwrap()
});

use std::collections::HashSet;

/// Result of sanitization.
pub struct SanitizationResult {
    pub body: Vec<u8>,
    pub redacted_types: Vec<String>,
}

/// Sanitize the accumulated content from a completed SSE stream.
///
/// Operates on the full assembled text (not individual chunks), which solves
/// the split-PII problem where a pattern like "user@example.com" could be
/// split across two SSE chunks and missed by per-chunk sanitization.
///
/// This is called on the audit log copy — the in-flight stream is forwarded
/// as-is to minimize latency.
pub fn sanitize_stream_content(content: &str) -> SanitizationResult {
    let mut redacted = HashSet::new();
    let sanitized = sanitize_text(content, &mut redacted);
    SanitizationResult {
        body: sanitized.into_bytes(),
        redacted_types: redacted.into_iter().collect(),
    }
}

/// Redact PII in a raw SSE text chunk before it reaches the client.
///
/// Processes the chunk line-by-line. Only lines starting with `data: ` are
/// scanned for PII — SSE framing lines (`event:`, empty lines, `data: [DONE]`)
/// pass through unchanged.
///
/// Returns `(output, did_redact)`:
/// - `output`: the (possibly redacted) chunk text
/// - `did_redact`: true if any PII was found and replaced
///
/// **Performance:** On the happy path (no PII), each `data:` line pays only
/// the cost of four `is_match()` calls which short-circuit on non-match.
/// No allocations occur until a match is found.
pub fn redact_sse_chunk(chunk: &str) -> (String, bool) {
    let mut redacted_any = false;
    let mut output = String::with_capacity(chunk.len());
    let mut first = true;

    for line in chunk.split('\n') {
        // Preserve original newline structure
        if !first {
            output.push('\n');
        }
        first = false;

        // Only scan `data: ` lines (not `event:`, `id:`, empty, or `data: [DONE]`)
        if let Some(payload) = line.strip_prefix("data: ") {
            // Skip the [DONE] sentinel — never contains PII
            if payload == "[DONE]" {
                output.push_str(line);
                continue;
            }

            // Try to parse as JSON and redact string values within
            if let Ok(mut json_val) = serde_json::from_str::<serde_json::Value>(payload) {
                let mut types = HashSet::new();
                sanitize_json_value(&mut json_val, &mut types);
                if !types.is_empty() {
                    redacted_any = true;
                    output.push_str("data: ");
                    output.push_str(
                        &serde_json::to_string(&json_val).unwrap_or_else(|_| payload.to_string()),
                    );
                    continue;
                }
            }

            // Fallback: plain text data line (rare in SSE, but handle it)
            let mut types = HashSet::new();
            let sanitized = sanitize_text(payload, &mut types);
            if !types.is_empty() {
                redacted_any = true;
                output.push_str("data: ");
                output.push_str(&sanitized);
                continue;
            }
        }

        // No redaction needed — pass line through unchanged
        output.push_str(line);
    }

    (output, redacted_any)
}

/// Streaming-aware response sanitization.
///
/// Strategy:
/// - JSON: Recursively walk and sanitize string values.
/// - Text: Regex replacement on full body.
/// - Binary: Pass-through.
pub fn sanitize_response(body: &[u8], content_type: &str) -> SanitizationResult {
    let mut redacted = HashSet::new();

    // 1. JSON handling
    if content_type.contains("application/json") {
        if let Ok(mut value) = serde_json::from_slice::<Value>(body) {
            sanitize_json_value(&mut value, &mut redacted);
            if let Ok(sanitized) = serde_json::to_vec(&value) {
                return SanitizationResult {
                    body: sanitized,
                    redacted_types: redacted.into_iter().collect(),
                };
            }
        }
    }

    // 2. Text (or failed JSON) handling
    if let Ok(text) = std::str::from_utf8(body) {
        let sanitized = sanitize_text(text, &mut redacted);
        return SanitizationResult {
            body: sanitized.into_bytes(),
            redacted_types: redacted.into_iter().collect(),
        };
    }

    // 3. Binary pass-through
    SanitizationResult {
        body: body.to_vec(),
        redacted_types: vec![],
    }
}

fn sanitize_json_value(v: &mut Value, redacted: &mut HashSet<String>) {
    match v {
        Value::String(s) => *s = sanitize_text(s, redacted),
        Value::Array(arr) => {
            for i in arr {
                sanitize_json_value(i, redacted);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                sanitize_json_value(val, redacted);
            }
        }
        _ => {}
    }
}

fn sanitize_text(s: &str, redacted: &mut HashSet<String>) -> String {
    let mut s = s.to_string();

    if EMAIL_REGEX.is_match(&s) {
        s = EMAIL_REGEX.replace_all(&s, "[REDACTED_EMAIL]").to_string();
        redacted.insert("email".to_string());
    }
    if CREDIT_CARD_REGEX.is_match(&s) {
        s = CREDIT_CARD_REGEX
            .replace_all(&s, "[REDACTED_CC]")
            .to_string();
        redacted.insert("credit_card".to_string());
    }
    if SSN_REGEX.is_match(&s) {
        s = SSN_REGEX.replace_all(&s, "[REDACTED_SSN]").to_string();
        redacted.insert("ssn".to_string());
    }
    if API_KEY_REGEX.is_match(&s) {
        s = API_KEY_REGEX
            .replace_all(&s, "[REDACTED_API_KEY]")
            .to_string();
        redacted.insert("api_key".to_string());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_email() {
        let input = "Contact us at support@trueflow.dev for help.";
        let res = sanitize_response(input.as_bytes(), "text/plain");
        assert_eq!(
            String::from_utf8(res.body).unwrap(),
            "Contact us at [REDACTED_EMAIL] for help."
        );
        assert!(res.redacted_types.contains(&"email".to_string()));
    }

    #[test]
    fn test_sanitize_json() {
        let json = serde_json::json!({
            "user": {
                "email": "user@example.com",
                "id": 123
            },
            "api_key": "sk-1234567890abcdef1234567890abcdef"
        });
        let body = serde_json::to_vec(&json).unwrap();
        let res = sanitize_response(&body, "application/json");
        let sanitized_json: Value = serde_json::from_slice(&res.body).unwrap();

        assert_eq!(sanitized_json["user"]["email"], "[REDACTED_EMAIL]");
        assert_eq!(sanitized_json["api_key"], "[REDACTED_API_KEY]");
        assert!(res.redacted_types.contains(&"email".to_string()));
        assert!(res.redacted_types.contains(&"api_key".to_string()));
    }

    #[test]
    fn test_sanitize_cc() {
        let input = "Payment: 4111 1111 1111 1111"; // Vista valid-ish
        let res = sanitize_response(input.as_bytes(), "text/plain");
        assert_eq!(
            String::from_utf8(res.body).unwrap(),
            "Payment: [REDACTED_CC]"
        );
        assert!(res.redacted_types.contains(&"credit_card".to_string()));
    }

    // ── Streaming PII redaction (redact_sse_chunk) ──────────────

    #[test]
    fn test_sse_redact_ssn_in_json_data_line() {
        // Simulates SSE chunk: data: {"choices":[{"delta":{"content":"SSN: 123-45-6789"}}]}
        let chunk = r#"data: {"choices":[{"delta":{"content":"SSN: 123-45-6789"}}]}"#;
        let (output, did_redact) = redact_sse_chunk(chunk);
        assert!(did_redact, "Should detect SSN in SSE data line");
        assert!(
            !output.contains("123-45-6789"),
            "SSN should be redacted in output: '{}'",
            output
        );
        assert!(
            output.contains("[REDACTED_SSN]"),
            "Should contain redaction marker: '{}'",
            output
        );
        assert!(output.starts_with("data: "), "Should preserve data: prefix");
    }

    #[test]
    fn test_sse_redact_clean_passthrough() {
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hello world"}}]}"#;
        let (output, did_redact) = redact_sse_chunk(chunk);
        assert!(!did_redact, "No PII present — should not redact");
        assert_eq!(output, chunk, "Clean chunk should pass through unchanged");
    }

    #[test]
    fn test_sse_redact_multiline_mixed() {
        // Two SSE events in one chunk — only the one with PII should be modified
        let chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"email: user@test.com\"}}]}\n\n";
        let (output, did_redact) = redact_sse_chunk(chunk);
        assert!(did_redact, "Should detect email in second data line");
        assert!(
            output.contains("Hello"),
            "First data line should be preserved: '{}'",
            output
        );
        assert!(
            !output.contains("user@test.com"),
            "Email should be redacted: '{}'",
            output
        );
        assert!(
            output.contains("[REDACTED_EMAIL]"),
            "Should contain email redaction marker"
        );
    }

    #[test]
    fn test_sse_redact_framing_untouched() {
        // SSE framing lines and [DONE] should never be modified
        let chunk = "event: message\ndata: {\"choices\":[]}\n\ndata: [DONE]\n\n";
        let (output, did_redact) = redact_sse_chunk(chunk);
        assert!(!did_redact, "No PII in framing lines");
        assert_eq!(output, chunk, "Framing should pass through unchanged");
    }

    #[test]
    fn test_sse_redact_credit_card_in_content() {
        // Credit card number in LLM response content
        let chunk = r#"data: {"choices":[{"delta":{"content":"Card: 4111 1111 1111 1111"}}]}"#;
        let (output, did_redact) = redact_sse_chunk(chunk);
        assert!(did_redact, "Should detect credit card");
        assert!(
            !output.contains("4111 1111 1111 1111"),
            "CC should be redacted: '{}'",
            output
        );
        assert!(output.contains("[REDACTED_CC]"));
    }
}
