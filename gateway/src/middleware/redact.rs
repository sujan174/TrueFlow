//! Policy-driven redaction and transformation.
//!
//! Implements `Action::Redact` (pattern-based PII scrubbing) and
//! `Action::Transform` (header/body mutations) for the condition→action engine.

#![allow(dead_code)]
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

use crate::models::policy::{Action, RedactDirection, RedactOnMatch, TransformOp};

// ── Built-in PII patterns ────────────────────────────────────

/// Registry of named patterns. Policy authors can reference these by name
/// in the `patterns` array (e.g., `"patterns": ["ssn", "email"]`).
struct BuiltinPattern {
    name: &'static str,
    regex: &'static Lazy<Regex>,
    replacement: &'static str,
}

static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap());

static SSN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());

static CREDIT_CARD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(?:\d{4}[ -]){3}\d{1,7}\b|\b\d{15,16}\b").unwrap());

static API_KEY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(sk-[a-zA-Z0-9_\-\.]{20,})\b").unwrap());

static PHONE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\+?1?[-. ]?\(?\d{3}\)?[-. ]?\d{3}[-. ]?\d{4}\b").unwrap());

// Phase 6: Extended PII patterns for healthcare/finance
static IBAN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{4}\d{7}(?:[A-Z0-9]{0,16})\b").unwrap()
});

static DOB_RE: Lazy<Regex> = Lazy::new(|| {
    // MM/DD/YYYY or DD-MM-YYYY — common date-of-birth formats
    Regex::new(r"\b(0[1-9]|1[0-2])/(0[1-9]|[12]\d|3[01])/(19|20)\d{2}\b|\b(0[1-9]|[12]\d|3[01])-(0[1-9]|1[0-2])-(19|20)\d{2}\b").unwrap()
});

static IPV4_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b").unwrap()
});

// Phase 7: Extended PII patterns for enterprise compliance
static PASSPORT_RE: Lazy<Regex> = Lazy::new(|| {
    // BUG-05 FIX: Tighter — 2 letters + 7-9 digits (US/UK/EU passports).
    // Old pattern (1-2 letters + 6-9 digits) matched version codes like V12345678.
    Regex::new(r"\b[A-Z]{2}\d{7,9}\b").unwrap()
});

static AWS_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(AKIA[A-Z0-9]{16})\b").unwrap()
});

static DL_RE: Lazy<Regex> = Lazy::new(|| {
    // BUG-05 FIX: Tighter — 1 letter + 7-12 digits (minimum 7 to avoid A100, B2345, etc.).
    Regex::new(r"\b[A-Z]\d{7,12}\b").unwrap()
});

static MRN_RE: Lazy<Regex> = Lazy::new(|| {
    // Medical Record Number: MRN- or MR- followed by digits
    Regex::new(r"(?i)\b(MRN|MR)[-:]?\s*\d{5,12}\b").unwrap()
});

static BUILTIN_PATTERNS: &[BuiltinPattern] = &[
    BuiltinPattern {
        name: "email",
        regex: &EMAIL_RE,
        replacement: "[REDACTED_EMAIL]",
    },
    BuiltinPattern {
        name: "ssn",
        regex: &SSN_RE,
        replacement: "[REDACTED_SSN]",
    },
    BuiltinPattern {
        name: "credit_card",
        regex: &CREDIT_CARD_RE,
        replacement: "[REDACTED_CC]",
    },
    BuiltinPattern {
        name: "api_key",
        regex: &API_KEY_RE,
        replacement: "[REDACTED_API_KEY]",
    },
    BuiltinPattern {
        name: "phone",
        regex: &PHONE_RE,
        replacement: "[REDACTED_PHONE]",
    },
    BuiltinPattern {
        name: "iban",
        regex: &IBAN_RE,
        replacement: "[REDACTED_IBAN]",
    },
    BuiltinPattern {
        name: "dob",
        regex: &DOB_RE,
        replacement: "[REDACTED_DOB]",
    },
    BuiltinPattern {
        name: "ipv4",
        regex: &IPV4_RE,
        replacement: "[REDACTED_IP]",
    },
    BuiltinPattern {
        name: "passport",
        regex: &PASSPORT_RE,
        replacement: "[REDACTED_PASSPORT]",
    },
    BuiltinPattern {
        name: "aws_key",
        regex: &AWS_KEY_RE,
        replacement: "[REDACTED_AWS_KEY]",
    },
    BuiltinPattern {
        name: "drivers_license",
        regex: &DL_RE,
        replacement: "[REDACTED_DL]",
    },
    BuiltinPattern {
        name: "mrn",
        regex: &MRN_RE,
        replacement: "[REDACTED_MRN]",
    },
];

// ── Redact ───────────────────────────────────────────────────

/// The outcome of an `apply_redact` call.
#[derive(Debug, Default)]
pub struct RedactResult {
    /// PII type names that were matched (for audit logging).
    pub matched_types: Vec<String>,
    /// True when `on_match = "block"` AND PII was found — the caller should deny the request.
    pub should_block: bool,
}

/// Apply policy-driven redaction to a JSON body.
///
/// Supports two modes:
/// - **Pattern-based**: Named patterns (`ssn`, `email`) or custom regex strings.
/// - **Field-based**: Blanks specific JSON keys listed in `fields`.
///
/// Returns a `RedactResult` describing what matched and whether the request should be blocked.
pub fn apply_redact(body: &mut Value, action: &Action, is_request: bool) -> RedactResult {
    let (direction, patterns, fields, on_match) = match action {
        Action::Redact {
            direction,
            patterns,
            fields,
            on_match,
        } => (direction, patterns, fields, on_match),
        _ => return RedactResult::default(),
    };

    // Direction check: should we redact in this phase?
    let should_run = match direction {
        RedactDirection::Request  => is_request,
        RedactDirection::Response => !is_request,
        RedactDirection::Both     => true,
    };

    if !should_run {
        return RedactResult::default();
    }

    let mut matched = Vec::new();

    // 1. Pattern-based redaction (walk all string values)
    if !patterns.is_empty() {
        let compiled = compile_patterns(patterns);
        redact_value(body, &compiled, &mut matched);
    }

    // 2. Field-based redaction (blank named keys)
    if !fields.is_empty() {
        redact_fields(body, fields, &mut matched);
    }

    let should_block = !matched.is_empty() && *on_match == RedactOnMatch::Block;

    RedactResult { matched_types: matched, should_block }
}

/// Compile pattern names into (regex, replacement, name) tuples.
/// If a pattern name matches a built-in, use that; otherwise treat it as raw regex.
fn compile_patterns(patterns: &[String]) -> Vec<(Regex, String, String)> {
    patterns
        .iter()
        .filter_map(|p| {
            // Check built-in patterns first
            if let Some(builtin) = BUILTIN_PATTERNS.iter().find(|b| b.name == p) {
                // Clone the inner Regex from the Lazy
                let re: &Regex = builtin.regex;
                return Some((re.clone(), builtin.replacement.to_string(), p.clone()));
            }
            // Try compiling as custom regex with size limit to prevent ReDoS
            regex::RegexBuilder::new(p)
                .size_limit(1_000_000) // 1MB limit on compiled regex size
                .build()
                .ok()
                .map(|re| (re, format!("[REDACTED_{}]", p.to_uppercase()), p.clone()))
        })
        .collect()
}

/// Recursively walk a JSON value and apply pattern-based redaction to strings.
fn redact_value(v: &mut Value, patterns: &[(Regex, String, String)], matched: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            for (re, replacement, name) in patterns {
                if re.is_match(s) {
                    *s = re.replace_all(s, replacement.as_str()).to_string();
                    if !matched.contains(name) {
                        matched.push(name.clone());
                    }
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                redact_value(item, patterns, matched);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                redact_value(val, patterns, matched);
            }
        }
        _ => {}
    }
}

/// Blank specific JSON fields by name (recursive).
fn redact_fields(v: &mut Value, fields: &[String], matched: &mut Vec<String>) {
    if let Some(obj) = v.as_object_mut() {
        for field_name in fields {
            if obj.contains_key(field_name) {
                obj.insert(field_name.clone(), Value::String("[REDACTED]".to_string()));
                let tag = format!("field:{}", field_name);
                if !matched.contains(&tag) {
                    matched.push(tag);
                }
            }
        }
        // Recurse into nested objects and arrays
        for (_, val) in obj {
            redact_fields(val, fields, matched);
        }
    } else if let Some(arr) = v.as_array_mut() {
        for item in arr {
            redact_fields(item, fields, matched);
        }
    }
}

// ── Tokenization bridge ─────────────────────────────────────

/// Compile pattern names into `PiiPattern` structs for the tokenization vault.
/// Reuses the same builtin pattern registry as `compile_patterns`.
pub fn compile_pii_patterns(pattern_names: &[String]) -> Vec<crate::middleware::pii_vault::PiiPattern> {
    pattern_names
        .iter()
        .filter_map(|p| {
            // Check built-in patterns first
            if let Some(builtin) = BUILTIN_PATTERNS.iter().find(|b| b.name == p) {
                let re: &Regex = builtin.regex;
                return Some(crate::middleware::pii_vault::PiiPattern {
                    name: p.clone(),
                    regex: re.clone(),
                });
            }
            // Try compiling as custom regex
            regex::RegexBuilder::new(p)
                .size_limit(1_000_000)
                .build()
                .ok()
                .map(|re| crate::middleware::pii_vault::PiiPattern {
                    name: p.clone(),
                    regex: re,
                })
        })
        .collect()
}

/// Async tokenization entry point — called by the proxy handler when
/// `on_match == RedactOnMatch::Tokenize`.
///
/// Replaces PII with vault-backed tokens instead of destructive `[REDACTED_*]`.
pub async fn apply_redact_tokenize(
    body: &mut Value,
    action: &Action,
    is_request: bool,
    project_id: uuid::Uuid,
    audit_log_id: Option<uuid::Uuid>,
    pool: &sqlx::PgPool,
    vault: &crate::vault::builtin::VaultCrypto,
) -> RedactResult {
    let (direction, patterns, fields, _) = match action {
        Action::Redact {
            direction,
            patterns,
            fields,
            on_match: _,
        } => (direction, patterns, fields, ()),
        _ => return RedactResult::default(),
    };

    // Direction check
    let should_run = match direction {
        RedactDirection::Request  => is_request,
        RedactDirection::Response => !is_request,
        RedactDirection::Both     => true,
    };

    if !should_run {
        return RedactResult::default();
    }

    let mut matched = Vec::new();

    // 1. Pattern-based tokenization (async — stores tokens in PG)
    if !patterns.is_empty() {
        let pii_patterns = compile_pii_patterns(patterns);
        let tok_result = crate::middleware::pii_vault::tokenize_in_value(
            body, &pii_patterns, project_id, audit_log_id, pool, vault,
        ).await;
        matched.extend(tok_result.matched_types);
    }

    // 2. Field-based redaction is always destructive (fields don't have a "value" to tokenize)
    if !fields.is_empty() {
        redact_fields(body, fields, &mut matched);
    }

    RedactResult { matched_types: matched, should_block: false }
}

/// Collected header mutations from Transform actions.
/// Applied after the pre-flight loop completes.
#[derive(Debug, Default)]
pub struct HeaderMutations {
    pub inserts: Vec<(String, String)>,
    pub removals: Vec<String>,
}

/// Apply collected header mutations to a header map.
#[allow(dead_code)]
pub fn apply_header_mutations(headers: &mut hyper::HeaderMap, mutations: &HeaderMutations) {
    for name in &mutations.removals {
        if let Ok(key) = hyper::header::HeaderName::from_bytes(name.as_bytes()) {
            headers.remove(&key);
        }
    }
    for (name, value) in &mutations.inserts {
        if let (Ok(key), Ok(val)) = (
            hyper::header::HeaderName::from_bytes(name.as_bytes()),
            hyper::header::HeaderValue::from_str(value),
        ) {
            headers.insert(key, val);
        }
    }
}

/// Apply a single transform operation.
///
/// - `SetHeader`/`RemoveHeader` → collected into `HeaderMutations` for deferred application
/// - `AppendSystemPrompt` → modifies the body in-place (OpenAI messages format)
pub fn apply_transform(body: &mut Value, header_mutations: &mut HeaderMutations, op: &TransformOp) {
    match op {
        TransformOp::SetHeader { name, value } => {
            // SEC: Block reserved headers to prevent credential injection override
            let reserved = ["authorization", "host", "cookie", "set-cookie", "x-admin-key"];
            if reserved.contains(&name.to_lowercase().as_str()) {
                tracing::warn!(header = %name, "transform: blocked reserved header");
                return;
            }
            tracing::info!(header = %name, "transform: set header");
            header_mutations.inserts.push((name.clone(), value.clone()));
        }
        TransformOp::RemoveHeader { name } => {
            tracing::info!(header = %name, "transform: remove header");
            header_mutations.removals.push(name.clone());
        }
        TransformOp::AppendSystemPrompt { text } => {
            tracing::info!("transform: append system prompt");
            append_system_prompt(body, text);
        }
        TransformOp::PrependSystemPrompt { text } => {
            tracing::info!("transform: prepend system prompt");
            prepend_system_prompt(body, text);
        }
        TransformOp::RegexReplace { pattern, replacement, global } => {
            tracing::info!(pattern = %pattern, "transform: regex replace");
            // B-REDACT-1 FIX: Use size_limit to prevent ReDoS attacks
            if let Ok(re) = regex::RegexBuilder::new(pattern)
                .size_limit(1_000_000)
                .build()
            {
                apply_regex_replace_to_value(body, &re, replacement, *global);
            } else {
                tracing::warn!(pattern = %pattern, "transform: invalid or too-complex regex pattern, skipping");
            }
        }
        TransformOp::SetBodyField { path, value } => {
            tracing::info!(path = %path, "transform: set body field");
            set_body_field_by_path(body, path, value.clone());
        }
        TransformOp::RemoveBodyField { path } => {
            tracing::info!(path = %path, "transform: remove body field");
            remove_body_field_by_path(body, path);
        }
        TransformOp::AddToMessageList { role, content, position } => {
            tracing::info!(role = %role, position = %position, "transform: add to message list");
            add_to_message_list(body, role, content, position);
        }
    }
}

// ── Logging Redaction ────────────────────────────────────────

/// Redact all known PII patterns from a JSON value for safe storage (Level 1 logging).
/// Applies every built-in pattern (SSN, email, credit card, API key, phone) and returns
/// the serialised JSON string, or None if input is None.
pub fn redact_for_logging(body: &Option<serde_json::Value>) -> Option<String> {
    let body = body.as_ref()?;
    let mut clone = body.clone();
    redact_all_patterns(&mut clone);
    Some(serde_json::to_string(&clone).unwrap_or_default())
}

/// Apply every built-in PII pattern to all string values in a JSON tree.
fn redact_all_patterns(v: &mut Value) {
    match v {
        Value::String(s) => {
            for pat in BUILTIN_PATTERNS {
                let re: &Regex = pat.regex;
                if re.is_match(s) {
                    *s = re.replace_all(s, pat.replacement).to_string();
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                redact_all_patterns(item);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                redact_all_patterns(val);
            }
        }
        _ => {}
    }
}

/// Append a system message to an OpenAI-format messages array.
fn append_system_prompt(body: &mut Value, text: &str) {
    if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        messages.push(serde_json::json!({
            "role": "system",
            "content": text
        }));
    }
}

/// Prepend text to the first system message, or insert a system message at index 0.
fn prepend_system_prompt(body: &mut Value, text: &str) {
    if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        // Find existing system message and prepend
        if let Some(sys_msg) = messages.iter_mut().find(|m| m["role"] == "system") {
            if let Some(content) = sys_msg["content"].as_str() {
                let new_content = format!("{}\n{}", text, content);
                sys_msg["content"] = Value::String(new_content);
                return;
            }
        }
        // No system message — insert at position 0
        messages.insert(0, serde_json::json!({
            "role": "system",
            "content": text
        }));
    }
}

/// Apply a regex find/replace to every string value in a JSON tree.
fn apply_regex_replace_to_value(v: &mut Value, re: &Regex, replacement: &str, global: bool) {
    match v {
        Value::String(s) => {
            let new = if global {
                re.replace_all(s, replacement).to_string()
            } else {
                re.replace(s, replacement).to_string()
            };
            *s = new;
        }
        Value::Array(arr) => {
            for item in arr {
                apply_regex_replace_to_value(item, re, replacement, global);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                apply_regex_replace_to_value(val, re, replacement, global);
            }
        }
        _ => {}
    }
}

/// Set a JSON field by dot-separated path, creating intermediate objects as needed.
/// e.g. `"temperature"` sets `body.temperature`; `"user.name"` sets `body.user.name`.
fn set_body_field_by_path(body: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.splitn(2, '.').collect();
    if parts.len() == 1 {
        // Leaf — set directly
        if let Some(obj) = body.as_object_mut() {
            obj.insert(parts[0].to_owned(), value);
        }
    } else {
        // Recurse into nested object, creating it if absent
        if let Some(obj) = body.as_object_mut() {
            let child = obj.entry(parts[0]).or_insert(Value::Object(Default::default()));
            set_body_field_by_path(child, parts[1], value);
        }
    }
}

/// Remove a JSON field by dot-separated path.
fn remove_body_field_by_path(body: &mut Value, path: &str) {
    let parts: Vec<&str> = path.splitn(2, '.').collect();
    if parts.len() == 1 {
        if let Some(obj) = body.as_object_mut() {
            obj.remove(parts[0]);
        }
    } else if let Some(obj) = body.as_object_mut() {
        if let Some(child) = obj.get_mut(parts[0]) {
            remove_body_field_by_path(child, parts[1]);
        }
    }
}

/// Inject a synthetic message into the `messages` array.
/// `position`:
///   - `"first"` — insert at index 0
///   - `"last"` — append at the end
///   - `"before_last"` (default) — insert before the final message
fn add_to_message_list(body: &mut Value, role: &str, content: &str, position: &str) {
    if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        let msg = serde_json::json!({ "role": role, "content": content });
        match position {
            "first" => messages.insert(0, msg),
            "last" => messages.push(msg),
            _ => {
                // "before_last" — insert before the last element
                let len = messages.len();
                if len == 0 {
                    messages.push(msg);
                } else {
                    messages.insert(len - 1, msg);
                }
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::HeaderMap;
    use serde_json::json;

    // ── Pattern-based Redaction ──────────────────────────────

    #[test]
    fn test_redact_email_pattern() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec!["email".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"user": {"email": "alice@example.com", "name": "Alice"}});
        let result = apply_redact(&mut body, &action, true);

        assert_eq!(body["user"]["email"], "[REDACTED_EMAIL]");
        assert_eq!(body["user"]["name"], "Alice"); // untouched
        assert!(result.matched_types.contains(&"email".to_string()));
    }

    #[test]
    fn test_redact_ssn_pattern() {
        let action = Action::Redact {
            direction: RedactDirection::Both,
            patterns: vec!["ssn".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"data": "My SSN is 123-45-6789"});
        let result = apply_redact(&mut body, &action, true);

        assert_eq!(body["data"], "My SSN is [REDACTED_SSN]");
        assert!(result.matched_types.contains(&"ssn".to_string()));
    }

    #[test]
    fn test_redact_multiple_patterns() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec!["email".to_string(), "api_key".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({
            "from": "user@test.com",
            "key": "sk-abcdefghijklmnopqrstuvwxyz1234"
        });
        let result = apply_redact(&mut body, &action, true);

        assert_eq!(body["from"], "[REDACTED_EMAIL]");
        assert_eq!(body["key"], "[REDACTED_API_KEY]");
        assert_eq!(result.matched_types.len(), 2);
    }

    #[test]
    fn test_redact_custom_regex_pattern() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec![r"\b[A-Z]{2}\d{6}\b".to_string()], // passport-like
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"passport": "AB123456"});
        let result = apply_redact(&mut body, &action, true);

        assert!(body["passport"].as_str().unwrap().contains("[REDACTED_"));
        assert_eq!(result.matched_types.len(), 1);
    }

    #[test]
    fn test_redact_nested_arrays() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec!["email".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({
            "users": [
                {"email": "a@b.com"},
                {"email": "c@d.com"}
            ]
        });
        let result = apply_redact(&mut body, &action, true);

        assert_eq!(body["users"][0]["email"], "[REDACTED_EMAIL]");
        assert_eq!(body["users"][1]["email"], "[REDACTED_EMAIL]");
        assert_eq!(result.matched_types.len(), 1); // deduplicated
    }

    // ── Field-based Redaction ────────────────────────────────

    #[test]
    fn test_redact_named_fields() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec![],
            fields: vec!["password".to_string(), "secret".to_string()],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({
            "user": "alice",
            "password": "hunter2",
            "secret": "s3cr3t"
        });
        let result = apply_redact(&mut body, &action, true);

        assert_eq!(body["password"], "[REDACTED]");
        assert_eq!(body["secret"], "[REDACTED]");
        assert_eq!(body["user"], "alice");
        assert!(result.matched_types.contains(&"field:password".to_string()));
    }

    #[test]
    fn test_redact_fields_nested() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec![],
            fields: vec!["token".to_string()],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({
            "auth": {"token": "xyz"},
            "data": {"nested": {"token": "abc"}}
        });
        apply_redact(&mut body, &action, true);

        assert_eq!(body["auth"]["token"], "[REDACTED]");
        assert_eq!(body["data"]["nested"]["token"], "[REDACTED]");
    }

    // ── Direction Filtering ──────────────────────────────────

    #[test]
    fn test_redact_direction_request_only() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec!["email".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"email": "a@b.com"});

        // Should run on request
        let result = apply_redact(&mut body, &action, true);
        assert_eq!(result.matched_types.len(), 1);

        // Should NOT run on response
        let mut body2 = json!({"email": "a@b.com"});
        let result2 = apply_redact(&mut body2, &action, false);
        assert!(result2.matched_types.is_empty());
        assert_eq!(body2["email"], "a@b.com"); // untouched
    }

    #[test]
    fn test_redact_direction_response_only() {
        let action = Action::Redact {
            direction: RedactDirection::Response,
            patterns: vec!["ssn".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"data": "SSN: 123-45-6789"});

        // Should NOT run on request
        let result = apply_redact(&mut body, &action, true);
        assert!(result.matched_types.is_empty());

        // Should run on response
        let result2 = apply_redact(&mut body, &action, false);
        assert_eq!(result2.matched_types.len(), 1);
    }

    #[test]
    fn test_redact_direction_both() {
        let action = Action::Redact {
            direction: RedactDirection::Both,
            patterns: vec!["email".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body_req = json!({"email": "a@b.com"});
        let mut body_resp = json!({"email": "c@d.com"});

        assert_eq!(apply_redact(&mut body_req, &action, true).matched_types.len(), 1);
        assert_eq!(apply_redact(&mut body_resp, &action, false).matched_types.len(), 1);
    }

    // ── Transform: AppendSystemPrompt ────────────────────────

    #[test]
    fn test_transform_append_system_prompt() {
        let mut body = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });
        let mut mutations = HeaderMutations::default();
        let op = TransformOp::AppendSystemPrompt {
            text: "Always be helpful and safe.".to_string(),
        };

        apply_transform(&mut body, &mut mutations, &op);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["role"], "system");
        assert_eq!(messages[1]["content"], "Always be helpful and safe.");
    }

    #[test]
    fn test_transform_append_no_messages_key() {
        let mut body = json!({"model": "gpt-4"});
        let mut mutations = HeaderMutations::default();
        let op = TransformOp::AppendSystemPrompt {
            text: "Be safe.".to_string(),
        };

        apply_transform(&mut body, &mut mutations, &op);

        // No messages array → no change
        assert!(body.get("messages").is_none());
    }

    // ── Transform: Headers ───────────────────────────────────

    #[test]
    fn test_transform_set_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();

        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetHeader {
                name: "X-Custom".to_string(),
                value: "true".to_string(),
            },
        );

        assert_eq!(mutations.inserts.len(), 1);
        assert_eq!(
            mutations.inserts[0],
            ("X-Custom".to_string(), "true".to_string())
        );
    }

    #[test]
    fn test_transform_remove_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();

        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::RemoveHeader {
                name: "Authorization".to_string(),
            },
        );

        assert_eq!(mutations.removals.len(), 1);
        assert_eq!(mutations.removals[0], "Authorization");
    }

    #[test]
    fn test_apply_header_mutations() {
        let mut headers = HeaderMap::new();
        headers.insert("x-old", "remove-me".parse().unwrap());
        headers.insert("x-keep", "keep-me".parse().unwrap());

        let mutations = HeaderMutations {
            inserts: vec![("x-new".to_string(), "added".to_string())],
            removals: vec!["x-old".to_string()],
        };

        apply_header_mutations(&mut headers, &mutations);

        assert!(headers.get("x-old").is_none());
        assert_eq!(headers.get("x-new").unwrap(), "added");
        assert_eq!(headers.get("x-keep").unwrap(), "keep-me");
    }

    // ── No-op cases ──────────────────────────────────────────

    #[test]
    fn test_redact_no_patterns_no_fields() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec![],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"email": "a@b.com"});
        let result = apply_redact(&mut body, &action, true);

        assert!(result.matched_types.is_empty());
        assert_eq!(body["email"], "a@b.com"); // untouched
    }

    #[test]
    fn test_redact_wrong_action_type() {
        let action = Action::Deny {
            status: 403,
            message: "no".to_string(),
        };
        let mut body = json!({"data": "test"});
        let result = apply_redact(&mut body, &action, true);
        assert!(result.matched_types.is_empty());
    }

    #[test]
    fn test_redact_phone_pattern() {
        let action = Action::Redact {
            direction: RedactDirection::Request,
            patterns: vec!["phone".to_string()],
            fields: vec![],
            on_match: RedactOnMatch::Redact,
        };
        let mut body = json!({"contact": "Call me at 555-123-4567"});
        let result = apply_redact(&mut body, &action, true);

        assert!(body["contact"]
            .as_str()
            .unwrap()
            .contains("[REDACTED_PHONE]"));
        assert!(result.matched_types.contains(&"phone".to_string()));
    }

    // ── SEC: Transform reserved header injection block ────────

    #[test]
    fn test_transform_blocks_authorization_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetHeader {
                name: "Authorization".to_string(),
                value: "Bearer stolen-token".to_string(),
            },
        );
        assert!(mutations.inserts.is_empty(), "Authorization header must be blocked");
    }

    #[test]
    fn test_transform_blocks_host_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetHeader {
                name: "Host".to_string(),
                value: "evil.com".to_string(),
            },
        );
        assert!(mutations.inserts.is_empty(), "Host header must be blocked");
    }

    #[test]
    fn test_transform_blocks_cookie_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetHeader {
                name: "cookie".to_string(),
                value: "session=hijacked".to_string(),
            },
        );
        assert!(mutations.inserts.is_empty(), "Cookie header must be blocked");
    }

    #[test]
    fn test_transform_blocks_admin_key_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetHeader {
                name: "X-Admin-Key".to_string(),
                value: "admin-override".to_string(),
            },
        );
        assert!(mutations.inserts.is_empty(), "X-Admin-Key header must be blocked");
    }

    #[test]
    fn test_transform_allows_non_reserved_header() {
        let mut body = json!({});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetHeader {
                name: "X-Request-Id".to_string(),
                value: "abc-123".to_string(),
            },
        );
        assert_eq!(mutations.inserts.len(), 1, "Non-reserved header should be allowed");
    }

    // ── redact_for_logging ────────────────────────────────────

    #[test]
    fn test_redact_for_logging_removes_pii() {
        let body = Some(json!({
            "message": "My SSN is 123-45-6789 and email is test@example.com"
        }));
        let result = redact_for_logging(&body).unwrap();
        assert!(result.contains("[REDACTED_SSN]"), "SSN should be redacted for logging");
        assert!(result.contains("[REDACTED_EMAIL]"), "Email should be redacted for logging");
        assert!(!result.contains("123-45-6789"), "Raw SSN must not appear in logged output");
        assert!(!result.contains("test@example.com"), "Raw email must not appear in logged output");
    }

    #[test]
    fn test_redact_for_logging_none_returns_none() {
        let result = redact_for_logging(&None);
        assert!(result.is_none(), "None body should return None");
    }

    // ── Transform: PrependSystemPrompt ────────────────────────

    #[test]
    fn test_transform_prepend_system_prompt_merges() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hello"}
            ]
        });
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::PrependSystemPrompt {
                text: "IMPORTANT CONTEXT:".to_string(),
            },
        );
        let sys = body["messages"][0]["content"].as_str().unwrap();
        assert!(sys.starts_with("IMPORTANT CONTEXT:"), "Prepend should come first: {}", sys);
        assert!(sys.contains("You are helpful."), "Original content must be preserved");
    }

    #[test]
    fn test_transform_prepend_no_existing_system_inserts_at_0() {
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::PrependSystemPrompt {
                text: "Be safe.".to_string(),
            },
        );
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "Be safe.");
    }

    // ── Transform: RegexReplace ───────────────────────────────

    #[test]
    fn test_transform_regex_replace_global() {
        let mut body = json!({
            "messages": [{"role": "user", "content": "foo bar foo baz foo"}]
        });
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::RegexReplace {
                pattern: "foo".to_string(),
                replacement: "XXX".to_string(),
                global: true,
            },
        );
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "XXX bar XXX baz XXX");
    }

    #[test]
    fn test_transform_regex_replace_single() {
        let mut body = json!({
            "messages": [{"role": "user", "content": "foo bar foo baz foo"}]
        });
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::RegexReplace {
                pattern: "foo".to_string(),
                replacement: "XXX".to_string(),
                global: false,
            },
        );
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "XXX bar foo baz foo", "Non-global should replace only first match");
    }

    // ── Transform: SetBodyField / RemoveBodyField ─────────────

    #[test]
    fn test_transform_set_body_field_flat() {
        let mut body = json!({"model": "gpt-4", "messages": []});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetBodyField {
                path: "temperature".to_string(),
                value: json!(0.7),
            },
        );
        assert_eq!(body["temperature"], 0.7);
    }

    #[test]
    fn test_transform_set_body_field_nested() {
        let mut body = json!({"model": "gpt-4"});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::SetBodyField {
                path: "metadata.source".to_string(),
                value: json!("trueflow"),
            },
        );
        assert_eq!(body["metadata"]["source"], "trueflow");
    }

    #[test]
    fn test_transform_remove_body_field() {
        let mut body = json!({"model": "gpt-4", "stream": true, "temperature": 0.5});
        let mut mutations = HeaderMutations::default();
        apply_transform(
            &mut body,
            &mut mutations,
            &TransformOp::RemoveBodyField {
                path: "stream".to_string(),
            },
        );
        assert!(body.get("stream").is_none(), "stream field should be removed");
        assert_eq!(body["model"], "gpt-4", "Other fields must be preserved");
    }
}
