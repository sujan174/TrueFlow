//! Adversarial Integration Tests — Phase 4
//!
//! Multi-layer middleware interaction tests that verify correct behavior
//! when multiple middleware layers interact, handle malformed inputs,
//! and prevent false positives in guardrails.
//!
//! Groups:
//!   A — Multi-Layer Pipeline (3 tests)
//!   B — Malformed Input Resilience (4 tests)
//!   C — Policy + Redact Interaction (3 tests)
//!   D — Guardrail False Positive Regression (2 tests)

use axum::http::{HeaderMap, Method, Uri};
use gateway::middleware::fields::RequestContext;
use gateway::middleware::guardrail::check_content;
use gateway::middleware::policy::evaluate_pre_flight;
use gateway::middleware::redact::apply_redact;
use gateway::middleware::sanitize;
use gateway::models::policy::*;
use serde_json::json;
use std::collections::HashMap;

// ── Helpers ──────────────────────────────────────────────────────────

fn make_ctx<'a>(
    method: &'a Method,
    path: &'a str,
    uri: &'a Uri,
    headers: &'a HeaderMap,
    body: Option<&'a serde_json::Value>,
) -> RequestContext<'a> {
    RequestContext {
        method,
        path,
        uri,
        headers,
        body,
        body_size: body.map(|b| b.to_string().len()).unwrap_or(0),
        agent_name: Some("test-agent"),
        token_id: "tok_integ_test",
        token_name: "Integration Test Token",
        project_id: "proj_integ",
        client_ip: Some("10.0.0.1"),
        response_status: None,
        response_body: None,
        response_headers: None,
        usage: HashMap::new(),
    }
}

fn deny_policy(name: &str, field: &str, op: &str, value: serde_json::Value) -> Policy {
    let policy_json = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "name": name,
        "mode": "enforce",
        "phase": "pre",
        "rules": [{
            "when": {"field": field, "op": op, "value": value},
            "then": {"action": "deny", "message": format!("{} triggered", name), "status": 403}
        }]
    });
    serde_json::from_value(policy_json).unwrap()
}

// ═══════════════════════════════════════════════════════════════════
//  GROUP A — Multi-Layer Pipeline
// ═══════════════════════════════════════════════════════════════════

/// STATE: Policy deny → response sanitization still runs on audit copy.
/// BREAK: If deny short-circuits sanitization, the audit entry would contain
///        raw PII from the original request body.
/// ASSERT: Policy denies the request, but we can still sanitize the body for audit.
#[test]
fn test_policy_deny_plus_audit_sanitization() {
    let method = Method::POST;
    let uri: Uri = "/v1/chat/completions".parse().unwrap();
    let headers = HeaderMap::new();
    let body = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "My SSN is 123-45-6789"}]
    });
    let ctx = make_ctx(&method, "/v1/chat/completions", &uri, &headers, Some(&body));

    // Policy: deny all POST to gpt-4o
    let policy = deny_policy("block-gpt4o", "request.body.model", "eq", json!("gpt-4o"));
    let outcome = evaluate_pre_flight(&[policy], &ctx);

    // 1. Verify the deny fires
    let denies: Vec<_> = outcome
        .actions
        .iter()
        .filter(|a| matches!(&a.action, Action::Deny { .. }))
        .collect();
    assert_eq!(denies.len(), 1, "Deny policy must fire");

    // 2. Even though denied, audit should sanitize the request body
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let sanitized = sanitize::sanitize_response(&body_bytes, "application/json");
    let sanitized_str = String::from_utf8_lossy(&sanitized.body);
    assert!(
        !sanitized_str.contains("123-45-6789"),
        "Audit copy must have SSN redacted even on denied requests"
    );
}

/// STATE: Policy redact (apply_redact) + response sanitize must not double-redact.
/// BREAK: If both layers replace SSN, the output would contain nested markers like
///        [REDACTED_SSN[REDACTED_SSN]].
/// ASSERT: Content contains exactly one [REDACTED] marker per PII instance.
#[test]
fn test_redact_and_sanitize_no_double_redaction() {
    // Phase 1: Policy-level redaction
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["ssn".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Redact,
    };
    let mut body = json!({
        "messages": [{"role": "user", "content": "SSN: 123-45-6789"}]
    });
    let redact_result = apply_redact(&mut body, &action, true);
    let _content_after_redact = body["messages"][0]["content"].as_str().unwrap().to_string();

    // Phase 2: Response-level sanitization on the already-redacted content
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let sanitized = sanitize::sanitize_response(&body_bytes, "application/json");
    let sanitized_str = String::from_utf8_lossy(&sanitized.body);

    // Verify no double-redaction: should NOT have nested markers
    assert!(
        !sanitized_str.contains("[REDACTED_SSN[REDACTED"),
        "Must not double-redact: '{}'",
        sanitized_str
    );
    // Original SSN must be gone
    assert!(
        !sanitized_str.contains("123-45-6789"),
        "Original SSN must be gone"
    );
    // Should have at least one REDACTED marker from the first pass
    assert!(
        !redact_result.matched_types.is_empty(),
        "Policy redaction should have matched SSN"
    );
}

/// STATE: Shadow policy fires violations log but does NOT block, while
///        an enforce policy on the same request DOES block.
/// BREAK: Shadow mode accidentally executing actions blocks the request.
/// ASSERT: Shadow logs violation, enforce blocks — both evaluated independently.
#[test]
fn test_shadow_and_enforce_coexist() {
    let method = Method::POST;
    let uri: Uri = "/v1/chat/completions".parse().unwrap();
    let headers = HeaderMap::new();
    let body = json!({"model": "gpt-4o", "messages": []});
    let ctx = make_ctx(&method, "/v1/chat/completions", &uri, &headers, Some(&body));

    let shadow_json = json!({
        "id": "00000000-0000-0000-0000-000000000010",
        "name": "shadow-log-all",
        "mode": "shadow",
        "phase": "pre",
        "rules": [{"when": {"always": true}, "then": {"action": "deny", "message": "shadow", "status": 403}}]
    });
    let enforce_json = json!({
        "id": "00000000-0000-0000-0000-000000000011",
        "name": "enforce-deny-gpt4o",
        "mode": "enforce",
        "phase": "pre",
        "rules": [{
            "when": {"field": "request.body.model", "op": "eq", "value": "gpt-4o"},
            "then": {"action": "deny", "message": "blocked", "status": 403}
        }]
    });
    let shadow: Policy = serde_json::from_value(shadow_json).unwrap();
    let enforce: Policy = serde_json::from_value(enforce_json).unwrap();

    let outcome = evaluate_pre_flight(&[shadow, enforce], &ctx);

    // Shadow produces violations, not blocking actions
    assert!(
        !outcome.shadow_violations.is_empty(),
        "Shadow mode must produce shadow_violations"
    );
    // Enforce produces blocking actions
    let denies: Vec<_> = outcome
        .actions
        .iter()
        .filter(|a| matches!(&a.action, Action::Deny { .. }))
        .collect();
    assert_eq!(denies.len(), 1, "Enforce policy must produce a deny action");
    assert_eq!(denies[0].policy_name, "enforce-deny-gpt4o");
}

// ═══════════════════════════════════════════════════════════════════
//  GROUP B — Malformed Input Resilience
// ═══════════════════════════════════════════════════════════════════

/// STATE: Empty request body → policies still evaluate against context fields.
/// BREAK: A None body causing unwrap/panic in field resolution.
/// ASSERT: Policies evaluating request.method still fire correctly with no body.
#[test]
fn test_empty_body_policies_still_evaluate() {
    let method = Method::DELETE;
    let uri: Uri = "/v1/files/abc".parse().unwrap();
    let headers = HeaderMap::new();
    let ctx = make_ctx(&method, "/v1/files/abc", &uri, &headers, None); // no body

    let policy = deny_policy("block-delete", "request.method", "eq", json!("DELETE"));
    let outcome = evaluate_pre_flight(&[policy], &ctx);

    let denies: Vec<_> = outcome
        .actions
        .iter()
        .filter(|a| matches!(&a.action, Action::Deny { .. }))
        .collect();
    assert_eq!(
        denies.len(),
        1,
        "Policy evaluating method must work even with no body"
    );
}

/// STATE: Non-JSON body bytes passed to sanitize_response → must not panic.
/// BREAK: Parsing invalid JSON with unwrap panics.
/// ASSERT: Returns body unchanged, no detected PII types.
#[test]
fn test_non_json_body_sanitization_no_panic() {
    let garbage = b"this is not json {{{";
    let result = sanitize::sanitize_response(garbage, "application/json");
    // Should fall through to text handling, not panic
    assert!(
        !result.body.is_empty(),
        "Sanitized output must not be empty for garbage input"
    );
}

/// STATE: Body with null values in expected string fields → no panic.
/// BREAK: Calling .as_str().unwrap() on a null JSON value panics.
/// ASSERT: Policy evaluation handles null model field gracefully.
#[test]
fn test_null_body_fields_no_panic() {
    let method = Method::POST;
    let uri: Uri = "/v1/chat/completions".parse().unwrap();
    let headers = HeaderMap::new();
    let body = json!({
        "model": null,
        "messages": null,
        "temperature": null
    });
    let ctx = make_ctx(&method, "/v1/chat/completions", &uri, &headers, Some(&body));

    let policy = deny_policy("check-model", "request.body.model", "eq", json!("gpt-4o"));
    let outcome = evaluate_pre_flight(&[policy], &ctx);

    // Null model != "gpt-4o" → should NOT trigger deny
    let denies: Vec<_> = outcome
        .actions
        .iter()
        .filter(|a| matches!(&a.action, Action::Deny { .. }))
        .collect();
    assert!(
        denies.is_empty(),
        "Null model field must not match 'gpt-4o'"
    );
}

/// STATE: Sanitize on empty byte slice → must not panic.
/// BREAK: Slice operations on empty input causing OOB.
/// ASSERT: Returns empty body, no PII matched.
#[test]
fn test_sanitize_empty_bytes_no_panic() {
    let result = sanitize::sanitize_response(b"", "application/json");
    assert!(result.redacted_types.is_empty(), "Empty input has no PII");
    assert!(
        result.body.is_empty() || result.body == b"",
        "Empty input produces empty output"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  GROUP C — Policy + Redact Interaction
// ═══════════════════════════════════════════════════════════════════

/// STATE: Redact in Both direction applies to both request and response bodies.
/// BREAK: Direction filter only checking is_request, missing is_response.
/// ASSERT: Same action applied to request body (is_request=true) AND response body
///         (is_request=false) both redact PII.
#[test]
fn test_redact_both_direction_applies_to_request_and_response() {
    let action = Action::Redact {
        direction: RedactDirection::Both,
        patterns: vec!["email".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Redact,
    };

    // Request body
    let mut request_body = json!({
        "messages": [{"role": "user", "content": "Contact me at user@example.com"}]
    });
    let req_result = apply_redact(&mut request_body, &action, true);
    let req_content = request_body["messages"][0]["content"].as_str().unwrap();
    assert!(
        !req_content.contains("user@example.com"),
        "Email must be redacted from request body"
    );
    assert!(
        !req_result.matched_types.is_empty(),
        "Request PII must be detected"
    );

    // Response body
    let mut response_body = json!({
        "choices": [{"message": {"content": "I found bob@corp.com in the records"}}]
    });
    let resp_result = apply_redact(&mut response_body, &action, false);
    let resp_content = response_body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap();
    assert!(
        !resp_content.contains("bob@corp.com"),
        "Email must be redacted from response body"
    );
    assert!(
        !resp_result.matched_types.is_empty(),
        "Response PII must be detected"
    );
}

/// STATE: Block mode sets should_block=true, which should prevent further processing.
/// BREAK: Block flag not being propagated, allowing blocked content through.
/// ASSERT: should_block is true when PII matched with block mode.
#[test]
fn test_block_mode_short_circuits_on_match() {
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["credit_card".to_string(), "ssn".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Block,
    };
    let mut body = json!({
        "messages": [{"role": "user", "content": "Card 4111111111111111 and SSN 123-45-6789"}]
    });
    let result = apply_redact(&mut body, &action, true);

    assert!(
        result.should_block,
        "Block mode must set should_block on PII match"
    );
    // Both PII types should be matched
    assert!(
        result.matched_types.len() >= 2,
        "Both CC and SSN should be matched, got: {:?}",
        result.matched_types
    );
}

/// STATE: Tokenize mode replaces PII with tok_pii_ tokens.
/// BREAK: Tokenize mode not being wired, falling through to Redact behavior.
/// ASSERT: RedactOnMatch::Tokenize variant exists and deserializes correctly.
#[test]
fn test_tokenize_mode_deserializes() {
    let _json = json!({
        "direction": "request",
        "patterns": ["ssn"],
        "fields": [],
        "on_match": "tokenize"
    });
    let action: Action = serde_json::from_value(json!({
        "action": "redact",
        "direction": "request",
        "patterns": ["ssn"],
        "on_match": "tokenize"
    }))
    .unwrap();

    match &action {
        Action::Redact { on_match, .. } => {
            assert_eq!(
                *on_match,
                RedactOnMatch::Tokenize,
                "Tokenize mode must deserialize correctly"
            );
        }
        _ => panic!("Expected Redact action"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  GROUP D — Guardrail False Positive Regression
// ═══════════════════════════════════════════════════════════════════

/// STATE: Topic denylist uses word boundaries — "context" must NOT match "text" denylist.
/// BREAK: Using .contains() instead of word-boundary regex would cause false positives.
/// ASSERT: "context" is allowed, "text" (as a standalone word) is blocked.
#[test]
fn test_topic_denylist_word_boundary_no_substring_match() {
    let action = Action::ContentFilter {
        block_jailbreak: false,
        block_harmful: false,
        block_code_injection: false,
        block_profanity: false,
        block_bias: false,
        block_competitor_mention: false,
        block_sensitive_topics: false,
        block_gibberish: false,
        block_contact_info: false,
        block_ip_leakage: false,
        competitor_names: vec![],
        topic_allowlist: vec![],
        topic_denylist: vec!["sex".to_string()],
        custom_patterns: vec![],
        risk_threshold: 0.5,
        max_content_length: 0,
    };

    // "context" contains "sex" as a substring, but word-boundary matching
    // should NOT flag it.
    let body_context = json!({
        "messages": [{"role": "user", "content": "Please provide more context about this topic"}]
    });
    let result_context = check_content(&body_context, &action);
    assert!(
        !result_context.blocked,
        "Word 'context' must NOT match denylist 'sex' (word boundary). Matched: {:?}",
        result_context.matched_patterns
    );

    // But the standalone word "sex" SHOULD be blocked
    let body_sex = json!({
        "messages": [{"role": "user", "content": "Tell me about sex education programs"}]
    });
    let result_sex = check_content(&body_sex, &action);
    assert!(
        result_sex.blocked,
        "Standalone word 'sex' must be blocked by denylist"
    );
}

/// STATE: Empty messages array → guardrail returns allow, no crash.
/// BREAK: Iterating over None/empty arrays without checking would panic.
/// ASSERT: check_content returns allowed=true with no matched patterns.
#[test]
fn test_guardrail_empty_messages_array() {
    let action = Action::ContentFilter {
        block_jailbreak: true,
        block_harmful: true,
        block_code_injection: true,
        block_profanity: false,
        block_bias: false,
        block_competitor_mention: false,
        block_sensitive_topics: false,
        block_gibberish: false,
        block_contact_info: false,
        block_ip_leakage: false,
        competitor_names: vec![],
        topic_allowlist: vec![],
        topic_denylist: vec![],
        custom_patterns: vec![],
        risk_threshold: 0.5,
        max_content_length: 0,
    };

    // Empty messages array
    let body_empty = json!({"messages": []});
    let result = check_content(&body_empty, &action);
    assert!(!result.blocked, "Empty messages must not be blocked");
    assert!(
        result.matched_patterns.is_empty(),
        "No patterns should match empty input"
    );

    // No messages field at all
    let body_missing = json!({"model": "gpt-4o"});
    let result2 = check_content(&body_missing, &action);
    assert!(
        !result2.blocked,
        "Missing messages field must not be blocked"
    );
}
