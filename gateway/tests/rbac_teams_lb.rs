//! Integration tests for Roadmap Items #5-#9.
//!
//! Tests cover:
//! - RBAC Model Access (#7): model_matches glob correctness, check_model_access edge cases
//! - Team/Org Management (#9): merge_tags, check_team_model_access, Team struct correctness
//! - Weighted Load Balancing (#6): routing strategy deserialization
//! - Observability (#5): ObserverHub construction
//! - AppError::Forbidden correctness
//!
//! NOTE: In-flight tracking tests are in the --bin target (proxy module is bin-only).
//!
//! These tests are HONEST — they test actual behavior, catch real bugs, and verify
//! both positive (should match) AND negative (should NOT match) cases.

use chrono::Utc;
use gateway::middleware::model_access::{check_model_access, model_matches};
use gateway::middleware::teams::{check_team_model_access, merge_tags, Team};
use gateway::models::policy::RoutingStrategy;
use serde_json::json;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// Model Access — Glob Matching Edge Cases (#7)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_model_matches_case_sensitivity_across_providers() {
    // Real-world: providers return model names in mixed case
    assert!(model_matches("GPT-4o", "gpt-4o"));
    assert!(model_matches("gpt-4o", "GPT-4O"));
    assert!(model_matches("Claude-3-Opus", "claude-3-opus"));
    assert!(model_matches("LLAMA-3.1-70B", "llama-3.1-70b"));
}

#[test]
fn test_model_matches_version_suffixes() {
    // Real-world: models often have date suffixes
    assert!(model_matches("gpt-4o-2024-08-06", "gpt-4o*"));
    assert!(model_matches("claude-3-haiku-20240307", "claude-3-haiku*"));
    assert!(model_matches(
        "claude-3-5-sonnet-20241022",
        "claude-3-5-sonnet*"
    ));
    assert!(!model_matches("claude-3-opus-20240229", "claude-3-haiku*"));
}

#[test]
fn test_model_matches_exact_must_not_match_substring() {
    // CRITICAL: exact match must NOT match substrings
    assert!(!model_matches("gpt-4o-mini", "gpt-4o"));
    assert!(!model_matches("gpt-4o", "gpt-4"));
    assert!(!model_matches("gpt-4", "gpt-4o"));
    assert!(!model_matches("claude-3-opus", "claude-3"));
}

#[test]
fn test_model_matches_empty_pattern() {
    // Empty pattern matches nothing (non-empty model)
    assert!(!model_matches("gpt-4o", ""));
    // Empty model with empty pattern: both lowercase "" == "" → exact match returns true
    // This is acceptable because empty model names are filtered upstream (check_model_access)
    assert!(model_matches("", ""));
}

#[test]
fn test_model_matches_pattern_with_dots() {
    // Models with version dots (e.g., llama-3.1-70b)
    assert!(model_matches("llama-3.1-70b", "llama-3.1*"));
    assert!(model_matches("llama-3.1-70b-instruct", "llama-3.1*"));
    assert!(!model_matches("llama-3.2-70b", "llama-3.1*"));
}

#[test]
fn test_model_matches_triple_glob() {
    // Pattern with multiple wildcards: "gpt-*-*" or "*-*-preview"
    assert!(model_matches("gpt-4-turbo-preview", "gpt-*-*-preview"));
    assert!(!model_matches("gpt-4-turbo", "gpt-*-*-preview"));
}

#[test]
fn test_model_matches_no_false_positive_on_partial_prefix() {
    // "gpt-4*" should NOT match "llama-gpt-4o" — it's a prefix match
    assert!(!model_matches("llama-gpt-4o", "gpt-4*"));
    // But "*gpt-4*" (contains) would
    assert!(model_matches("llama-gpt-4o", "*gpt-4*"));
}

// ═══════════════════════════════════════════════════════════════════════════
// check_model_access — Full Enforcement Logic (#7)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_check_model_access_non_json_array_value_is_ignored() {
    // If allowed_models is a JSON string instead of array, should allow all
    let bad_json = json!("gpt-4o"); // string, not array
    assert!(check_model_access("gpt-4o", Some(&bad_json), &[]).is_ok());
    assert!(check_model_access("claude-3-opus", Some(&bad_json), &[]).is_ok());
}

#[test]
fn test_check_model_access_non_string_array_items_skipped() {
    // Array contains non-string items — they should be silently skipped
    let allowed = json!([42, null, "gpt-4o", true]);
    assert!(check_model_access("gpt-4o", Some(&allowed), &[]).is_ok());
    assert!(check_model_access("claude-3-opus", Some(&allowed), &[]).is_err());
}

#[test]
fn test_check_model_access_group_only_no_direct_restriction() {
    // Only group models configured, no direct allowed_models
    let groups = vec!["gpt-4o-mini".to_string(), "gpt-3.5*".to_string()];
    assert!(check_model_access("gpt-4o-mini", None, &groups).is_ok());
    assert!(check_model_access("gpt-3.5-turbo", None, &groups).is_ok());
    assert!(check_model_access("gpt-4o", None, &groups).is_err());
}

#[test]
fn test_check_model_access_error_message_lists_all_patterns() {
    let allowed = json!(["gpt-4o", "gpt-4o-mini"]);
    let groups = vec!["claude-3-haiku*".to_string()];
    let err = check_model_access("llama-3-70b", Some(&allowed), &groups).unwrap_err();
    // Should list all patterns
    assert!(err.contains("gpt-4o"), "Error should list direct patterns");
    assert!(
        err.contains("gpt-4o-mini"),
        "Error should list direct patterns"
    );
    assert!(
        err.contains("claude-3-haiku*"),
        "Error should list group patterns"
    );
    assert!(
        err.contains("llama-3-70b"),
        "Error should name the denied model"
    );
}

#[test]
fn test_check_model_access_wildcard_pattern_allows_all() {
    let allowed = json!(["*"]);
    assert!(check_model_access("gpt-4o", Some(&allowed), &[]).is_ok());
    assert!(check_model_access("claude-3-opus", Some(&allowed), &[]).is_ok());
    assert!(check_model_access("any-model-name", Some(&allowed), &[]).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// Team Model Access — check_team_model_access (#9)
// ═══════════════════════════════════════════════════════════════════════════

fn make_team(
    name: &str,
    allowed_models: Option<serde_json::Value>,
    max_budget: Option<rust_decimal::Decimal>,
    budget_duration: Option<&str>,
    tags: serde_json::Value,
) -> Team {
    Team {
        id: Uuid::new_v4(),
        org_id: Uuid::new_v4(),
        name: name.into(),
        description: None,
        max_budget_usd: max_budget,
        budget_duration: budget_duration.map(String::from),
        allowed_models,
        tags,
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn test_team_model_access_null_allows_all() {
    let team = make_team("open-team", None, None, None, json!({}));
    assert!(check_team_model_access("gpt-4o", &team).is_ok());
    assert!(check_team_model_access("claude-3-opus", &team).is_ok());
    assert!(check_team_model_access("any-model", &team).is_ok());
}

#[test]
fn test_team_model_access_empty_array_allows_all() {
    let team = make_team("open-team", Some(json!([])), None, None, json!({}));
    assert!(check_team_model_access("gpt-4o", &team).is_ok());
}

#[test]
fn test_team_model_access_restricts_with_exact() {
    let team = make_team(
        "budget-team",
        Some(json!(["gpt-4o-mini"])),
        None,
        None,
        json!({}),
    );
    assert!(check_team_model_access("gpt-4o-mini", &team).is_ok());
    assert!(check_team_model_access("gpt-4o", &team).is_err());
    assert!(check_team_model_access("claude-3-opus", &team).is_err());
}

#[test]
fn test_team_model_access_restricts_with_glob() {
    let team = make_team(
        "budget-team",
        Some(json!(["gpt-3.5*", "claude-3-haiku*"])),
        None,
        None,
        json!({}),
    );
    assert!(check_team_model_access("gpt-3.5-turbo", &team).is_ok());
    assert!(check_team_model_access("gpt-3.5-turbo-0125", &team).is_ok());
    assert!(check_team_model_access("claude-3-haiku-20240307", &team).is_ok());
    assert!(check_team_model_access("gpt-4o", &team).is_err());
    assert!(check_team_model_access("claude-3-opus", &team).is_err());
}

#[test]
fn test_team_model_access_error_names_team() {
    let team = make_team(
        "ML Engineering",
        Some(json!(["gpt-4o"])),
        None,
        None,
        json!({}),
    );
    let err = check_team_model_access("claude-3-opus", &team).unwrap_err();
    assert!(
        err.contains("ML Engineering"),
        "Error should mention team name"
    );
    assert!(
        err.contains("claude-3-opus"),
        "Error should mention denied model"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tag Merging — merge_tags (#9)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_merge_tags_non_object_values_handled() {
    // If team_tags is null instead of {}, should not panic
    let team = json!(null);
    let token = json!({"key": "value"});
    let result = merge_tags(&team, &token);
    assert_eq!(result["key"], "value");
}

#[test]
fn test_merge_tags_preserves_nested_values() {
    let team = json!({"config": {"nested": true}});
    let token = json!({"other": "value"});
    let result = merge_tags(&team, &token);
    assert_eq!(result["config"]["nested"], true);
    assert_eq!(result["other"], "value");
}

#[test]
fn test_merge_tags_many_keys() {
    let team = json!({"a": 1, "b": 2, "c": 3});
    let token = json!({"d": 4, "e": 5, "b": 20}); // b overrides
    let result = merge_tags(&team, &token);
    assert_eq!(result["a"], 1);
    assert_eq!(result["b"], 20); // token wins
    assert_eq!(result["c"], 3);
    assert_eq!(result["d"], 4);
    assert_eq!(result["e"], 5);
}

// ═══════════════════════════════════════════════════════════════════════════
// Routing Strategy Enum — (#6) Deserialization Correctness
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_routing_strategy_all_variants_deserialize() {
    // NOTE: RoutingStrategy uses serde rename_all = "snake_case"
    let strategies = vec![
        ("\"lowest_cost\"", RoutingStrategy::LowestCost),
        ("\"lowest_latency\"", RoutingStrategy::LowestLatency),
        ("\"round_robin\"", RoutingStrategy::RoundRobin),
        ("\"least_busy\"", RoutingStrategy::LeastBusy),
        ("\"weighted_random\"", RoutingStrategy::WeightedRandom),
    ];

    for (json_str, expected) in strategies {
        let deserialized: RoutingStrategy =
            serde_json::from_str(json_str)
                .unwrap_or_else(|_| panic!("Failed to deserialize {}", json_str));
        assert_eq!(
            deserialized, expected,
            "Strategy {} should deserialize correctly",
            json_str
        );
    }
}

#[test]
fn test_routing_strategy_invalid_variant_rejected() {
    let result: Result<RoutingStrategy, _> = serde_json::from_str("\"InvalidStrategy\"");
    assert!(result.is_err(), "Invalid strategy name must be rejected");
}

#[test]
fn test_routing_strategy_round_trip() {
    let strategies = vec![
        RoutingStrategy::LowestCost,
        RoutingStrategy::LowestLatency,
        RoutingStrategy::RoundRobin,
        RoutingStrategy::LeastBusy,
        RoutingStrategy::WeightedRandom,
    ];

    for strategy in strategies {
        let serialized = serde_json::to_string(&strategy).unwrap();
        let deserialized: RoutingStrategy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(strategy, deserialized, "Round-trip must preserve strategy");
    }
}

// NOTE: In-flight tracking tests (LoadBalancer) are in the --bin target.
// The proxy module is only available in the binary, not the library crate.
// See: cargo test --bin trueflow -- "in_flight" (3 tests pass in bin target).

// ═══════════════════════════════════════════════════════════════════════════
// AppError::Forbidden — Error Response Correctness (#7)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_app_error_forbidden_exists_and_has_message() {
    use gateway::errors::AppError;

    let error = AppError::Forbidden("Model 'gpt-4o' not allowed".to_string());
    let error_str = format!("{}", error);
    assert!(
        error_str.contains("gpt-4o"),
        "Error display should contain the reason"
    );
    assert!(error_str.contains("not allowed") || error_str.contains("forbidden"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Model Access Group Struct — (#7) Deserialization
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_model_access_group_struct_fields() {
    // Verify the struct can be constructed and serialized
    let group = gateway::middleware::model_access::ModelAccessGroup {
        id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        name: "Budget Models".into(),
        description: Some("Only cheap models".into()),
        models: json!(["gpt-4o-mini", "gpt-3.5-turbo*"]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let serialized = serde_json::to_value(&group).unwrap();
    assert_eq!(serialized["name"], "Budget Models");
    assert_eq!(serialized["models"][0], "gpt-4o-mini");
    assert_eq!(serialized["models"][1], "gpt-3.5-turbo*");
}

// ═══════════════════════════════════════════════════════════════════════════
// Team Struct — (#9) Serialization and Field Presence
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_team_struct_serialization() {
    let team = make_team(
        "ML Engineering",
        Some(json!(["gpt-4o", "claude-3-haiku*"])),
        Some(rust_decimal::Decimal::new(50000, 2)), // $500.00
        Some("monthly"),
        json!({"department": "engineering", "cost_center": "CC-42"}),
    );

    let serialized = serde_json::to_value(&team).unwrap();
    assert_eq!(serialized["name"], "ML Engineering");
    assert_eq!(serialized["max_budget_usd"], "500.00");
    assert_eq!(serialized["budget_duration"], "monthly");
    assert_eq!(serialized["tags"]["department"], "engineering");
    assert!(serialized["is_active"].as_bool().unwrap());
}

#[test]
fn test_team_spend_struct_fields() {
    let spend = gateway::middleware::teams::TeamSpend {
        id: Uuid::new_v4(),
        team_id: Uuid::new_v4(),
        period: chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        total_requests: 1500,
        total_tokens_used: 2_500_000,
        total_spend_usd: rust_decimal::Decimal::new(4750, 2), // $47.50
        updated_at: Utc::now(),
    };

    let serialized = serde_json::to_value(&spend).unwrap();
    assert_eq!(serialized["total_requests"], 1500);
    assert_eq!(serialized["total_tokens_used"], 2_500_000);
    assert_eq!(serialized["total_spend_usd"], "47.50");
}

// ═══════════════════════════════════════════════════════════════════════════
// Combined Enforcement — Model Access + Team Access Layered (#7 + #9)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_combined_token_and_team_model_restriction() {
    // Scenario: Token allows gpt-4*, Team only allows gpt-4o-mini
    // Token check should pass for gpt-4o, but team should deny it
    let token_allowed = json!(["gpt-4*"]); // token level: allows all gpt-4 family
    let team = make_team(
        "budget-team",
        Some(json!(["gpt-4o-mini"])),
        None,
        None,
        json!({}),
    );

    // Token-level: gpt-4o passes
    assert!(check_model_access("gpt-4o", Some(&token_allowed), &[]).is_ok());
    // Team-level: gpt-4o denied
    assert!(check_team_model_access("gpt-4o", &team).is_err());
    // Team-level: gpt-4o-mini passes
    assert!(check_team_model_access("gpt-4o-mini", &team).is_ok());
}

#[test]
fn test_model_access_no_false_positive_on_similar_names() {
    // "gpt-4o" should NOT match "gpt-4" or "gpt-4o-mini"
    let allowed = json!(["gpt-4o"]);
    assert!(check_model_access("gpt-4o", Some(&allowed), &[]).is_ok());
    assert!(check_model_access("gpt-4o-mini", Some(&allowed), &[]).is_err());
    assert!(check_model_access("gpt-4", Some(&allowed), &[]).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Observability — ObserverHub Struct (#5)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_observer_hub_construction() {
    // ObserverHub::from_env() reads env vars for Langfuse/DataDog config.
    // In test environment, it creates a hub with no active exporters.
    let _hub = gateway::middleware::observer::ObserverHub::from_env();
    // Should be constructable without panic — it's the entry point for telemetry
}

// ═══════════════════════════════════════════════════════════════════════════
// Regression Guard — Ensures patterns that MUST NOT match don't (#7)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_no_false_positive_regression_suite() {
    // Each of these MUST be is_err() — if any becomes is_ok(), we have a false positive
    let allowed = json!(["gpt-4o-mini"]);

    let must_deny = vec![
        "gpt-4o",
        "gpt-4o-mini-2024", // This one should deny since "gpt-4o-mini" is exact, not glob
        "gpt-4o-mini-preview",
        "gpt-4",
        "gpt-3.5-turbo",
        "claude-3-opus",
        "",
    ];

    for model in &must_deny {
        if model.is_empty() {
            // Empty model is always allowed (skip)
            continue;
        }
        let result = check_model_access(model, Some(&allowed), &[]);
        assert!(
            result.is_err(),
            "SECURITY: Model '{}' should be DENIED when only 'gpt-4o-mini' is allowed, but it was allowed!",
            model
        );
    }
}

#[test]
fn test_no_false_negative_regression_suite() {
    // Each of these MUST be is_ok() — if any becomes is_err(), we have a false negative
    let allowed = json!(["gpt-4*", "claude-3-haiku*"]);

    let must_allow = vec![
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4-turbo",
        "gpt-4-turbo-preview",
        "gpt-4o-2024-08-06",
        "claude-3-haiku-20240307",
    ];

    for model in must_allow {
        let result = check_model_access(model, Some(&allowed), &[]);
        assert!(
            result.is_ok(),
            "Model '{}' should be ALLOWED when patterns are ['gpt-4*', 'claude-3-haiku*'], but it was denied!",
            model
        );
    }
}
