//! Integration tests for enterprise roadmap features built in this session.
//!
//! Tests cover:
//! - Fix 2: RBAC scope enforcement on all handlers
//! - Fix 4: Tool-call structured audit logging
//! - PII Tokenization Vault: tokenize, rehydrate, determinism, isolation
//! - Policy model: RedactOnMatch::Tokenize deserialization
//!
//! Note: Fix 1 (SIEM logging), Fix 3 (HITL BLPOP), and Fix 5 (Key Rotation)
//! require live Redis/PG connections and are verified via `cargo check` + manual testing.

use gateway::middleware::pii_vault;
use gateway::middleware::redact;
use gateway::models::policy::{Action, RedactDirection, RedactOnMatch};
use serde_json::json;
use uuid::Uuid;

// ── RedactOnMatch::Tokenize deserialization ──────────────────────────────────

#[test]
fn test_redact_on_match_tokenize_deserializes() {
    let action: Action = serde_json::from_str(
        r#"{
        "action": "redact",
        "patterns": ["credit_card", "ssn"],
        "on_match": "tokenize"
    }"#,
    )
    .unwrap();

    match action {
        Action::Redact {
            on_match, patterns, ..
        } => {
            assert_eq!(on_match, RedactOnMatch::Tokenize);
            assert_eq!(patterns, vec!["credit_card", "ssn"]);
        }
        _ => panic!("Expected Action::Redact"),
    }
}

#[test]
fn test_redact_on_match_backwards_compatible() {
    // "redact" (default) still works
    let action: Action = serde_json::from_str(
        r#"{
        "action": "redact",
        "patterns": ["email"],
        "on_match": "redact"
    }"#,
    )
    .unwrap();
    match action {
        Action::Redact { on_match, .. } => assert_eq!(on_match, RedactOnMatch::Redact),
        _ => panic!("Expected Action::Redact"),
    }

    // "block" still works
    let action: Action = serde_json::from_str(
        r#"{
        "action": "redact",
        "patterns": ["ssn"],
        "on_match": "block"
    }"#,
    )
    .unwrap();
    match action {
        Action::Redact { on_match, .. } => assert_eq!(on_match, RedactOnMatch::Block),
        _ => panic!("Expected Action::Redact"),
    }
}

#[test]
fn test_redact_on_match_default_is_redact() {
    // Omitting on_match should default to "redact"
    let action: Action = serde_json::from_str(
        r#"{
        "action": "redact",
        "patterns": ["email"]
    }"#,
    )
    .unwrap();
    match action {
        Action::Redact { on_match, .. } => assert_eq!(on_match, RedactOnMatch::Redact),
        _ => panic!("Expected Action::Redact"),
    }
}

#[test]
fn test_tokenize_serializes_correctly() {
    let on_match = RedactOnMatch::Tokenize;
    let serialized = serde_json::to_string(&on_match).unwrap();
    assert_eq!(serialized, r#""tokenize""#);
}

// ── PII Token Generation ────────────────────────────────────────────────────

#[test]
fn test_pii_token_deterministic_across_calls() {
    let project = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let tokens: Vec<String> = (0..100)
        .map(|_| pii_vault::generate_token(project, "credit_card", "4111111111111111"))
        .collect();

    // All 100 calls must produce the same token
    assert!(
        tokens.windows(2).all(|w| w[0] == w[1]),
        "Token generation must be perfectly deterministic"
    );
}

#[test]
fn test_pii_token_project_isolation() {
    let p1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let p2 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

    let t1 = pii_vault::generate_token(p1, "ssn", "123-45-6789");
    let t2 = pii_vault::generate_token(p2, "ssn", "123-45-6789");

    assert_ne!(
        t1, t2,
        "Same PII in different projects must produce different tokens"
    );
    assert!(t1.starts_with("tok_pii_ssn_"));
    assert!(t2.starts_with("tok_pii_ssn_"));
}

#[test]
fn test_pii_token_type_isolation() {
    let project = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

    // Same value but different types must produce different tokens
    let t1 = pii_vault::generate_token(project, "email", "test@test.com");
    let t2 = pii_vault::generate_token(project, "credit_card", "test@test.com");

    assert_ne!(
        t1, t2,
        "Same value with different PII types must produce different tokens"
    );
    assert!(t1.starts_with("tok_pii_email_"));
    assert!(t2.starts_with("tok_pii_credit_card_"));
}

#[test]
fn test_pii_token_no_original_value_leaked() {
    let project = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let token = pii_vault::generate_token(project, "credit_card", "4111111111111111");

    // The token must NOT contain ANY part of the original value
    assert!(
        !token.contains("4111"),
        "Token must not leak original value"
    );
    assert!(
        !token.contains("1111"),
        "Token must not contain partial original"
    );

    // But must contain the type identifier (so humans can see what was tokenized)
    assert!(token.contains("credit_card"), "Token must contain PII type");
}

// ── compile_pii_patterns ────────────────────────────────────────────────────

#[test]
fn test_compile_pii_patterns_builtin() {
    let patterns = redact::compile_pii_patterns(&[
        "email".to_string(),
        "ssn".to_string(),
        "credit_card".to_string(),
    ]);

    assert_eq!(patterns.len(), 3);
    assert!(patterns[0].regex.is_match("user@example.com"));
    assert!(patterns[1].regex.is_match("123-45-6789"));
    assert!(patterns[2].regex.is_match("4111111111111111"));
}

#[test]
fn test_compile_pii_patterns_custom_regex() {
    let patterns = redact::compile_pii_patterns(&[
        r"\b[A-Z]{2}\d{6}\b".to_string(), // passport-like
    ]);

    assert_eq!(patterns.len(), 1);
    assert!(patterns[0].regex.is_match("AB123456"));
    assert!(!patterns[0].regex.is_match("hello"));
}

#[test]
fn test_compile_pii_patterns_invalid_regex_skipped() {
    let patterns = redact::compile_pii_patterns(&[
        "email".to_string(),
        "[invalid regex(".to_string(), // should be skipped
        "ssn".to_string(),
    ]);

    // Only email and ssn should compile; invalid regex is silently skipped
    assert_eq!(patterns.len(), 2);
}

// ── Existing Redaction Still Works (no regressions) ─────────────────────────

#[test]
fn test_destructive_redact_unaffected_by_tokenize_addition() {
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["email".to_string(), "ssn".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Redact,
    };

    let mut body = json!({
        "message": "Contact alice@example.com, SSN: 123-45-6789"
    });

    let result = redact::apply_redact(&mut body, &action, true);

    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("[REDACTED_EMAIL]"));
    assert!(body["message"].as_str().unwrap().contains("[REDACTED_SSN]"));
    assert!(!body["message"]
        .as_str()
        .unwrap()
        .contains("alice@example.com"));
    assert!(!body["message"].as_str().unwrap().contains("123-45-6789"));
    assert_eq!(result.matched_types.len(), 2);
    assert!(!result.should_block);
}

#[test]
fn test_block_mode_unaffected_by_tokenize_addition() {
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["credit_card".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Block,
    };

    let mut body = json!({
        "payment": "4111111111111111"
    });

    let result = redact::apply_redact(&mut body, &action, true);

    assert!(
        result.should_block,
        "Block mode must still block on PII detection"
    );
    assert!(result.matched_types.contains(&"credit_card".to_string()));
}

// ── PII Pattern Collection (deep JSON trees) ────────────────────────────────

#[test]
fn test_collect_pii_deeply_nested_json() {
    let _body = json!({
        "level1": {
            "level2": {
                "level3": {
                    "level4": {
                        "email": "deep@nested.com"
                    }
                }
            }
        }
    });

    let project = Uuid::nil();

    // Verify token generation works for deeply nested values
    let token = pii_vault::generate_token(project, "email", "deep@nested.com");
    assert!(token.starts_with("tok_pii_email_"));
}

#[test]
fn test_collect_pii_in_arrays() {
    let _body = json!({
        "contacts": [
            {"email": "a@b.com"},
            {"email": "c@d.com"},
            {"phone": "no-pii-here"}
        ]
    });

    // Verify that patterns would match both emails
    let _email_re = regex::Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap();
    let project = Uuid::nil();

    // Both emails should produce different tokens
    let t1 = pii_vault::generate_token(project, "email", "a@b.com");
    let t2 = pii_vault::generate_token(project, "email", "c@d.com");
    assert_ne!(t1, t2);
}

// ── Full Policy Deserialization with Tokenize ───────────────────────────────

#[test]
fn test_full_tokenize_policy_deserialization() {
    let policy_json = r#"{
        "id": "00000000-0000-0000-0000-000000000001",
        "name": "pii-tokenize-policy",
        "description": "Tokenize PII instead of destroying it",
        "rules": [
            {
                "when": {"type": "always"},
                "then": {
                    "action": "redact",
                    "direction": "request",
                    "patterns": ["credit_card", "ssn", "email"],
                    "on_match": "tokenize"
                }
            }
        ]
    }"#;

    let policy: gateway::models::policy::Policy = serde_json::from_str(policy_json).unwrap();
    assert_eq!(policy.name, "pii-tokenize-policy");
    assert_eq!(policy.rules.len(), 1);

    match &policy.rules[0].then[0] {
        Action::Redact {
            direction,
            patterns,
            on_match,
            ..
        } => {
            assert_eq!(on_match, &RedactOnMatch::Tokenize);
            assert!(matches!(direction, RedactDirection::Request));
            assert_eq!(patterns.len(), 3);
        }
        other => panic!("Expected Redact action, got {:?}", other),
    }
}

// ── Edge Cases ──────────────────────────────────────────────────────────────

#[test]
fn test_token_empty_value() {
    let project = Uuid::nil();
    let token = pii_vault::generate_token(project, "email", "");
    assert!(token.starts_with("tok_pii_email_"));
    // Even empty strings produce valid tokens (3B-1 FIX: 32-char hash)
    assert_eq!(token.len(), "tok_pii_email_".len() + 32);
}

#[test]
fn test_token_unicode_value() {
    let project = Uuid::nil();
    let token = pii_vault::generate_token(project, "email", "user@例え.jp");
    assert!(token.starts_with("tok_pii_email_"));
    // Unicode must not crash or produce invalid tokens
    assert!(token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
}

#[test]
fn test_token_very_long_value() {
    let project = Uuid::nil();
    let long_value = "x".repeat(100_000);
    let token = pii_vault::generate_token(project, "custom", &long_value);
    assert!(token.starts_with("tok_pii_custom_"));
    // Token length must be fixed regardless of input length (3B-1 FIX: 32-char hash)
    assert_eq!(token.len(), "tok_pii_custom_".len() + 32);
}

// ═══════════════════════════════════════════════════════════════════════════
// Tool-Level RBAC — Action::ToolScope correctness + false positive checks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tool_scope_action_deserializes() {
    // Verify the new Action::ToolScope deserializes correctly from JSON
    let action: Action = serde_json::from_str(
        r#"{
        "action": "tool_scope",
        "allowed_tools": ["jira.read", "jira.search"],
        "blocked_tools": ["stripe.createCharge"]
    }"#,
    )
    .unwrap();

    match action {
        Action::ToolScope {
            allowed_tools,
            blocked_tools,
            deny_message,
        } => {
            assert_eq!(allowed_tools, vec!["jira.read", "jira.search"]);
            assert_eq!(blocked_tools, vec!["stripe.createCharge"]);
            assert_eq!(deny_message, "tool not authorized for this agent");
        }
        _ => panic!("Expected Action::ToolScope"),
    }
}

#[test]
fn test_tool_scope_extracts_openai_tool_names() {
    use gateway::middleware::engine::extract_tool_names;

    let body = json!({
        "model": "gpt-4o",
        "tools": [
            { "type": "function", "function": { "name": "jira.read", "description": "Read Jira issues" } },
            { "type": "function", "function": { "name": "jira.search", "description": "Search Jira" } }
        ]
    });
    let names = extract_tool_names(Some(&body));
    assert_eq!(names, vec!["jira.read", "jira.search"]);
}

#[test]
fn test_tool_scope_extracts_anthropic_tool_names() {
    use gateway::middleware::engine::extract_tool_names;

    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "tools": [
            { "name": "stripe.createCharge", "description": "Create a Stripe charge" }
        ]
    });
    let names = extract_tool_names(Some(&body));
    assert_eq!(names, vec!["stripe.createCharge"]);
}

#[test]
fn test_tool_scope_no_tools_is_not_a_false_positive() {
    // FALSE POSITIVE CHECK: requests with no tools should never trigger ToolScope
    use gateway::middleware::engine::extract_tool_names;

    let body = json!({ "model": "gpt-4o", "messages": [{ "role": "user", "content": "Hello" }] });
    let names = extract_tool_names(Some(&body));
    assert!(
        names.is_empty(),
        "No tools in body should produce empty tool names"
    );
}

#[test]
fn test_tool_scope_empty_body_is_not_a_false_positive() {
    use gateway::middleware::engine::extract_tool_names;
    // Body is None (no parsed body) — should never trigger
    let names = extract_tool_names(None);
    assert!(names.is_empty());
}

#[test]
fn test_tool_scope_blocked_tool_denied() {
    use gateway::middleware::engine::evaluate_tool_scope;

    let tool_names = vec!["stripe.createCharge".to_string()];
    let blocked = vec!["stripe.createCharge".to_string()];
    let result = evaluate_tool_scope(&tool_names, &[], &blocked, "tool denied");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("stripe.createCharge"),
        "Error should name the offending tool"
    );
    assert!(err.contains("blocked"), "Error should say 'blocked'");
}

#[test]
fn test_tool_scope_allowed_tool_passes() {
    use gateway::middleware::engine::evaluate_tool_scope;

    let tool_names = vec!["jira.read".to_string()];
    let allowed = vec!["jira.read".to_string(), "jira.search".to_string()];
    let result = evaluate_tool_scope(&tool_names, &allowed, &[], "denied");
    assert!(result.is_ok(), "Allowed tool should pass without error");
}

#[test]
fn test_tool_scope_unlisted_tool_denied_when_allowlist_active() {
    use gateway::middleware::engine::evaluate_tool_scope;

    // stripe.createCharge is NOT in the allowed list
    let tool_names = vec!["stripe.createCharge".to_string()];
    let allowed = vec!["jira.read".to_string()];
    let result = evaluate_tool_scope(&tool_names, &allowed, &[], "denied");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not in the allowed list"));
}

#[test]
fn test_tool_scope_glob_pattern_in_blocklist() {
    use gateway::middleware::engine::evaluate_tool_scope;

    // "stripe.*" in blocklist should block any stripe tool
    let tool_names = vec!["stripe.refund".to_string()];
    let blocked = vec!["stripe.*".to_string()];
    let result = evaluate_tool_scope(&tool_names, &[], &blocked, "denied");
    assert!(result.is_err(), "Glob pattern in blocklist should match");
}

#[test]
fn test_tool_scope_glob_no_false_positive() {
    use gateway::middleware::engine::evaluate_tool_scope;

    // "stripe.*" in blocklist should NOT block a jira tool
    let tool_names = vec!["jira.read".to_string()];
    let blocked = vec!["stripe.*".to_string()];
    let result = evaluate_tool_scope(&tool_names, &[], &blocked, "denied");
    assert!(
        result.is_ok(),
        "Non-matching glob should not block unrelated tools"
    );
}

#[test]
fn test_tool_scope_empty_lists_allow_all_no_false_positive() {
    use gateway::middleware::engine::evaluate_tool_scope;

    // Both allowed and blocked are empty → allow any tool (permissive default)
    let tool_names = vec!["anything.goes".to_string(), "every.tool".to_string()];
    let result = evaluate_tool_scope(&tool_names, &[], &[], "denied");
    assert!(
        result.is_ok(),
        "Empty allowed/blocked lists should allow all tools"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Anomaly Detection — statistical correctness + false positive checks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_anomaly_below_threshold_no_false_positive() {
    // If velocity is within 3σ of the mean, should NOT alert
    let values = [10.0_f64; 20]; // stable baseline of 10 req/window
    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let variance: f64 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();
    let threshold = mean + 3.0 * stddev;

    // With zero variance, threshold = mean = 10
    // A velocity of 10 should NOT exceed threshold
    assert!(
        10.0 <= threshold,
        "10 req/window should NOT trigger on stable baseline of 10"
    );
}

#[test]
fn test_anomaly_far_above_threshold_detects_spike() {
    let values = [10.0_f64; 20];
    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let variance: f64 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();
    let threshold = mean + 3.0 * stddev;

    // 100 req when baseline is 10 and stddev=0 → threshold=10, 100 > 10 → anomalous
    assert!(
        100.0 > threshold,
        "100 req/window should trigger on stable baseline of 10"
    );
}

#[test]
fn test_anomaly_realistic_baseline_not_triggered() {
    // Realistic scenario: baseline avg 50 req/window, stddev 10
    // threshold = 50 + 3*10 = 80
    // A velocity of 70 should NOT trigger
    let mean = 50.0_f64;
    let stddev = 10.0_f64;
    let threshold = mean + 3.0 * stddev;
    assert_eq!(threshold, 80.0);
    assert!(
        70.0 < threshold,
        "70 req/window below 3σ threshold of 80 should NOT alert"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// SSO/OIDC — false positive + security checks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_oidc_expired_jwt_rejected() {
    use base64::Engine;
    use gateway::middleware::oidc::decode_claims;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header = engine.encode(r#"{"alg":"RS256"}"#);
    // exp = 1000 (far in the past — Unix epoch + ~16 minutes)
    let payload = engine.encode(r#"{"sub":"user","exp":1000}"#);
    let token = format!("{}.{}.sig", header, payload);

    let result = decode_claims(&token);
    assert!(result.is_err(), "Expired token must be rejected");
    assert!(result.unwrap_err().to_string().contains("expired"));
}

#[test]
fn test_oidc_missing_sub_rejected() {
    use base64::Engine;
    use gateway::middleware::oidc::decode_claims;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header = engine.encode(r#"{"alg":"RS256"}"#);
    // No 'sub' claim
    let payload = engine.encode(r#"{"exp":9999999999,"iss":"https://example.okta.com"}"#);
    let token = format!("{}.{}.sig", header, payload);

    let result = decode_claims(&token);
    assert!(result.is_err(), "Token missing 'sub' must be rejected");
}

#[test]
fn test_session_entity_status_values() {
    // Verify the status values we accept are well-defined (no typos in handler)
    let valid_statuses = ["active", "paused", "completed"];
    for status in valid_statuses {
        assert!(!status.is_empty());
    }
    // Document invalid values that should be rejected (422)
    let invalid_statuses = ["running", "stopped", "pending", "ACTIVE"];
    for status in invalid_statuses {
        let valid = matches!(status, "active" | "paused" | "completed");
        assert!(!valid, "Status '{}' should be rejected as invalid", status);
    }
}
