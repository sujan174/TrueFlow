//! Integration tests for policy scope validation at binding time.
//!
//! These tests verify that policies attached to tokens are validated
//! against the token's allowed_providers and allowed_models scope.
//! This prevents misconfiguration where a policy routes to models
//! outside the token's permitted scope.
//!
//! **Test Categories:**
//! - Provider-level restrictions (allowed_providers)
//! - Model-level restrictions (allowed_models with glob patterns)
//! - Combined provider + model restrictions
//! - No restrictions (all models allowed)
//!
//! Run with: `cargo test policy_scope_validation --test policy_scope_validation`

use gateway::middleware::policy_scope::{validate_policy_scope_detailed, DetailedViolationType};
use serde_json::json;

// ── Provider-Level Restriction Tests ─────────────────────────────────────

/// Test: Policy with OpenAI model bound to OpenAI-only token should pass.
/// This validates that models matching the token's allowed_providers are accepted.
#[test]
fn test_scope_validation_openai_model_openai_token() {
    let routing_models = vec![
        ("gpt-4o-mini".to_string(), "dynamic_route".to_string()),
        ("gpt-3.5-turbo".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(
        result.is_ok(),
        "OpenAI models should be allowed for OpenAI-only token"
    );
}

/// Test: Policy with Anthropic model bound to OpenAI-only token should fail.
/// This validates that provider mismatch is detected and reported.
#[test]
fn test_scope_validation_anthropic_model_openai_token() {
    let routing_models = vec![
        ("gpt-4o".to_string(), "dynamic_route".to_string()),
        ("claude-3-opus".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_err(), "Anthropic model should be rejected for OpenAI-only token");

    let violations = result.unwrap_err();
    assert_eq!(violations.len(), 1, "Should have exactly one violation");
    assert_eq!(violations[0].model, "claude-3-opus");
    assert_eq!(violations[0].detected_provider, "anthropic");
    assert!(matches!(
        violations[0].violation_type,
        DetailedViolationType::ProviderNotAllowed { .. }
    ));
}

/// Test: Policy with unknown model prefix should fail if provider not explicitly allowed.
/// Unknown models (no recognizable prefix) should be rejected when provider restrictions exist.
#[test]
fn test_scope_validation_unknown_model_restricted_provider() {
    let routing_models = vec![("custom-model-v1".to_string(), "dynamic_route".to_string())];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_err(), "Unknown model should be rejected when provider is restricted");

    let violations = result.unwrap_err();
    assert!(matches!(
        violations[0].violation_type,
        DetailedViolationType::ProviderNotAllowed { .. }
    ));
}

// ── No Restrictions Tests ────────────────────────────────────────────────

/// Test: No restrictions means all models allowed.
/// When both allowed_providers and allowed_models are None, any model should pass.
#[test]
fn test_scope_validation_no_restrictions() {
    let routing_models = vec![
        ("gpt-4o".to_string(), "dynamic_route".to_string()),
        ("claude-3-opus".to_string(), "dynamic_route".to_string()),
        ("gemini-2.0-flash".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers: Option<Vec<String>> = None;
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "All models should be allowed with no restrictions");
}

// ── Model Pattern Matching Tests ──────────────────────────────────────────

/// Test: Model pattern matching with wildcard.
/// Glob patterns like "gpt-4*" should match models like "gpt-4o-mini".
#[test]
fn test_scope_validation_model_pattern() {
    let routing_models = vec![
        ("gpt-4o-mini".to_string(), "dynamic_route".to_string()),
        ("gpt-4-turbo".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models = Some(json!(["gpt-4*"]));

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(
        result.is_ok(),
        "Models matching pattern 'gpt-4*' should be allowed"
    );
}

/// Test: Model pattern rejection.
/// Models not matching the allowed patterns should be rejected.
#[test]
fn test_scope_validation_model_pattern_rejection() {
    let routing_models = vec![("gpt-3.5-turbo".to_string(), "dynamic_route".to_string())];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models = Some(json!(["gpt-4*"]));

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_err(), "Model not matching pattern should be rejected");

    let violations = result.unwrap_err();
    assert!(matches!(
        violations[0].violation_type,
        DetailedViolationType::ModelNotAllowed { .. }
    ));
}

/// Test: Multiple model patterns.
/// Token can allow multiple model patterns simultaneously.
#[test]
fn test_scope_validation_multiple_model_patterns() {
    let routing_models = vec![
        ("gpt-4o".to_string(), "dynamic_route".to_string()),
        ("claude-3-opus".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["openai".to_string(), "anthropic".to_string()]);
    let allowed_models = Some(json!(["gpt-*", "claude-*"]));

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "Models matching any pattern should be allowed");
}

// ── Multi-Provider Tests ─────────────────────────────────────────────────

/// Test: Multi-provider token with multi-provider policy.
/// Tokens can allow multiple providers, and policies can route to any of them.
#[test]
fn test_scope_validation_multi_provider() {
    let routing_models = vec![
        ("gpt-4o".to_string(), "dynamic_route".to_string()),
        ("claude-3-opus".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["openai".to_string(), "anthropic".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "Multi-provider token should allow models from all allowed providers");
}

/// Test: Partial violation in multi-provider scenario.
/// When some models are allowed and some are not, only violations are reported.
#[test]
fn test_scope_validation_partial_violation() {
    let routing_models = vec![
        ("gpt-4o".to_string(), "dynamic_route".to_string()),
        ("claude-3-opus".to_string(), "dynamic_route".to_string()),
        ("gemini-2.0-flash".to_string(), "dynamic_route".to_string()), // Not allowed
    ];

    let allowed_providers = Some(vec!["openai".to_string(), "anthropic".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_err(), "Should fail due to gemini model");
    let violations = result.unwrap_err();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].model, "gemini-2.0-flash");
    assert_eq!(violations[0].detected_provider, "google");
}

// ── Case Sensitivity Tests ───────────────────────────────────────────────

/// Test: Provider matching is case-insensitive.
/// Provider names should be compared case-insensitively.
#[test]
fn test_scope_validation_provider_case_insensitive() {
    let routing_models = vec![("gpt-4o".to_string(), "dynamic_route".to_string())];

    // Use uppercase provider name
    let allowed_providers = Some(vec!["OpenAI".to_string(), "ANTHROPIC".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "Provider matching should be case-insensitive");
}

// ── Edge Cases ───────────────────────────────────────────────────────────

/// Test: Empty routing models list should always pass.
#[test]
fn test_scope_validation_empty_routing_models() {
    let routing_models: Vec<(String, String)> = vec![];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "Empty routing models should always pass");
}

/// Test: Empty allowed_providers list should be treated as no restriction.
#[test]
fn test_scope_validation_empty_allowed_providers() {
    let routing_models = vec![("gpt-4o".to_string(), "dynamic_route".to_string())];

    let allowed_providers = Some(vec![]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "Empty allowed_providers should mean no restriction");
}

/// Test: Empty allowed_models array should be treated as no restriction.
#[test]
fn test_scope_validation_empty_allowed_models() {
    let routing_models = vec![("gpt-4o".to_string(), "dynamic_route".to_string())];

    let allowed_providers: Option<Vec<String>> = None;
    let allowed_models = Some(json!([]));

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "Empty allowed_models should mean no restriction");
}

/// Test: Conditional route models are validated the same as dynamic routes.
#[test]
fn test_scope_validation_conditional_route() {
    let routing_models = vec![("claude-3-opus".to_string(), "conditional_route".to_string())];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_err(), "Conditional route models should be validated");
    let violations = result.unwrap_err();
    assert_eq!(violations[0].model, "claude-3-opus");
}

// ── Detailed Violation Structure Tests ────────────────────────────────────

/// Test: DetailedScopeViolation contains correct allowed providers in error.
#[test]
fn test_scope_validation_violation_contains_allowed_providers() {
    let routing_models = vec![("claude-3-opus".to_string(), "dynamic_route".to_string())];

    let allowed_providers = Some(vec!["openai".to_string(), "azure".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    let violations = result.unwrap_err();

    if let DetailedViolationType::ProviderNotAllowed { allowed } = &violations[0].violation_type {
        assert_eq!(allowed.len(), 2);
        assert!(allowed.contains(&"openai".to_string()));
        assert!(allowed.contains(&"azure".to_string()));
    } else {
        panic!("Expected ProviderNotAllowed violation type");
    }
}

/// Test: DetailedScopeViolation contains correct allowed patterns in error.
#[test]
fn test_scope_validation_violation_contains_allowed_patterns() {
    let routing_models = vec![("gpt-3.5-turbo".to_string(), "dynamic_route".to_string())];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models = Some(json!(["gpt-4*", "o1-*"]));

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    let violations = result.unwrap_err();

    if let DetailedViolationType::ModelNotAllowed { allowed_patterns } =
        &violations[0].violation_type
    {
        assert_eq!(allowed_patterns.len(), 2);
        assert!(allowed_patterns.contains(&"gpt-4*".to_string()));
        assert!(allowed_patterns.contains(&"o1-*".to_string()));
    } else {
        panic!("Expected ModelNotAllowed violation type");
    }
}

// ── Provider Detection Tests ─────────────────────────────────────────────

/// Test: Various OpenAI model prefixes are correctly detected.
#[test]
fn test_scope_validation_openai_model_variants() {
    let routing_models = vec![
        ("gpt-4o".to_string(), "dynamic_route".to_string()),
        ("o1-preview".to_string(), "dynamic_route".to_string()),
        ("o3-mini".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["openai".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "All OpenAI model variants should be detected");
}

/// Test: Various Anthropic model prefixes are correctly detected.
#[test]
fn test_scope_validation_anthropic_model_variants() {
    let routing_models = vec![
        ("claude-3-opus".to_string(), "dynamic_route".to_string()),
        ("claude-sonnet-4".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["anthropic".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "All Anthropic model variants should be detected");
}

/// Test: Google Gemini models are correctly detected.
#[test]
fn test_scope_validation_google_model_variants() {
    let routing_models = vec![
        ("gemini-2.0-flash".to_string(), "dynamic_route".to_string()),
        ("gemini-pro".to_string(), "dynamic_route".to_string()),
    ];

    let allowed_providers = Some(vec!["google".to_string()]);
    let allowed_models: Option<serde_json::Value> = None;

    let result = validate_policy_scope_detailed(
        &routing_models,
        allowed_providers.as_deref(),
        allowed_models.as_ref(),
    );

    assert!(result.is_ok(), "All Google model variants should be detected");
}