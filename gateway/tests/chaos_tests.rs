//! Chaos Coverage: PII Redaction against obfuscated & adversarial inputs.
//!
//! These integration tests verify that the PII regex patterns catch credit cards
//! formatted with real-world separators (spaces, dashes, tabs) — not just clean
//! 16-digit strings that no user actually types.

use gateway::middleware::redact::apply_redact;
use gateway::models::policy::{Action, RedactDirection, RedactOnMatch};
use serde_json::json;

/// Build a standard credit_card redact action.
fn cc_redact_action() -> Action {
    Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["credit_card".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Redact,
        nlp_backend: None,
    }
}

// ═══════════════════════════════════════════════════════════════════
//  PII Obfuscation Tests
// ═══════════════════════════════════════════════════════════════════

/// Baseline: clean 16-digit credit card.
#[test]
fn test_cc_redaction_clean_number() {
    let action = cc_redact_action();
    let mut body =
        json!({"messages": [{"role": "user", "content": "My card is 4111111111111111"}]});
    let result = apply_redact(&mut body, &action, true);
    let content = body["messages"][0]["content"].as_str().unwrap();
    assert!(
        content.contains("[REDACTED_CC]"),
        "Clean CC not redacted: '{}'",
        content
    );
    assert!(result.matched_types.contains(&"credit_card".to_string()));
}

/// Credit card with space separators: `4111 1111 1111 1111`
#[test]
fn test_cc_redaction_with_spaces() {
    let action = cc_redact_action();
    let mut body = json!({
        "messages": [{"role": "user", "content": "My Visa is 4111 1111 1111 1111 please process it"}]
    });
    let result = apply_redact(&mut body, &action, true);
    let content = body["messages"][0]["content"].as_str().unwrap();
    assert!(
        content.contains("[REDACTED_CC]"),
        "Space-separated CC not redacted: '{}'",
        content
    );
    assert!(result.matched_types.contains(&"credit_card".to_string()));
}

/// Credit card with dash separators: `4111-1111-1111-1111`
#[test]
fn test_cc_redaction_with_dashes() {
    let action = cc_redact_action();
    let mut body = json!({
        "messages": [{"role": "user", "content": "Card number: 4111-1111-1111-1111"}]
    });
    let result = apply_redact(&mut body, &action, true);
    let content = body["messages"][0]["content"].as_str().unwrap();
    assert!(
        content.contains("[REDACTED_CC]"),
        "Dash-separated CC not redacted: '{}'",
        content
    );
    assert!(result.matched_types.contains(&"credit_card".to_string()));
}

/// Multiple PII types in a single message — all must be caught.
#[test]
fn test_multiple_pii_types_in_one_message() {
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec![
            "credit_card".to_string(),
            "email".to_string(),
            "ssn".to_string(),
        ],
        fields: vec![],
        on_match: RedactOnMatch::Redact,
        nlp_backend: None,
    };
    let mut body = json!({
        "messages": [{"role": "user",
            "content": "Send $500 to alice@example.com, card 4111111111111111, SSN 123-45-6789"}]
    });
    let result = apply_redact(&mut body, &action, true);
    let content = body["messages"][0]["content"].as_str().unwrap();
    assert!(content.contains("[REDACTED_CC]"), "CC not redacted");
    assert!(content.contains("[REDACTED_EMAIL]"), "Email not redacted");
    assert!(content.contains("[REDACTED_SSN]"), "SSN not redacted");
    assert_eq!(
        result.matched_types.len(),
        3,
        "Should match 3 PII types, got: {:?}",
        result.matched_types
    );
}

/// PII with `on_match=block` MUST set `should_block = true`.
#[test]
fn test_pii_block_mode_triggers_on_match() {
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["credit_card".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Block,
        nlp_backend: None,
    };
    let mut body = json!({"messages": [{"role": "user", "content": "Card: 4111111111111111"}]});
    let result = apply_redact(&mut body, &action, true);
    assert!(
        result.should_block,
        "on_match=block should set should_block when PII is found"
    );
}

/// No PII in content — `should_block` must be false even with block mode.
#[test]
fn test_pii_block_mode_no_false_positive() {
    let action = Action::Redact {
        direction: RedactDirection::Request,
        patterns: vec!["credit_card".to_string()],
        fields: vec![],
        on_match: RedactOnMatch::Block,
        nlp_backend: None,
    };
    let mut body = json!({"messages": [{"role": "user", "content": "What is the weather today?"}]});
    let result = apply_redact(&mut body, &action, true);
    assert!(
        !result.should_block,
        "should_block should be false when no PII"
    );
    assert!(result.matched_types.is_empty());
}

/// Deeply nested PII in tool_call arguments — recursive walk must find it.
#[test]
fn test_pii_deeply_nested_in_tool_calls() {
    let action = cc_redact_action();
    let mut body = json!({
        "messages": [{"role": "assistant", "tool_calls": [{
            "function": {"arguments": "{\"card_number\": \"4111111111111111\"}"}
        }]}]
    });
    let result = apply_redact(&mut body, &action, true);
    let args = body["messages"][0]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap();
    assert!(
        args.contains("[REDACTED_CC]"),
        "Nested CC in tool_calls not redacted: '{}'",
        args
    );
    assert!(result.matched_types.contains(&"credit_card".to_string()));
}
