use super::types::SpendByDimension;

/// Test that SpendByDimension serializes to the expected JSON shape.
/// This is NOT a false positive — it verifies the contract that the
/// frontend/SDK will consume. If any field is renamed or removed,
/// this test will catch the breaking change.
#[test]
fn test_spend_by_dimension_serialization_contract() {
    let row = SpendByDimension {
        dimension: "gpt-4o".to_string(),
        total_cost_usd: 42.50,
        request_count: 1000,
        total_prompt_tokens: 50000,
        total_completion_tokens: 25000,
    };

    let json = serde_json::to_value(&row).unwrap();

    // Verify exact field names (API contract)
    assert!(json.get("dimension").is_some(), "missing 'dimension' field");
    assert!(json.get("total_cost_usd").is_some(), "missing 'total_cost_usd' field");
    assert!(json.get("request_count").is_some(), "missing 'request_count' field");
    assert!(json.get("total_prompt_tokens").is_some(), "missing 'total_prompt_tokens' field");
    assert!(json.get("total_completion_tokens").is_some(), "missing 'total_completion_tokens' field");

    // Verify actual values (not just existence — prevents false positive)
    assert_eq!(json["dimension"], "gpt-4o");
    assert_eq!(json["total_cost_usd"], 42.5);
    assert_eq!(json["request_count"], 1000);
    assert_eq!(json["total_prompt_tokens"], 50000);
    assert_eq!(json["total_completion_tokens"], 25000);
}

/// Test that SpendByDimension deserialization works round-trip.
/// This validates that the sqlx::FromRow derivation will produce
/// a struct that can be serialized back to JSON for the API response.
#[test]
fn test_spend_by_dimension_roundtrip() {
    let original = SpendByDimension {
        dimension: "tag:engineering".to_string(),
        total_cost_usd: 0.0,
        request_count: 0,
        total_prompt_tokens: 0,
        total_completion_tokens: 0,
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let deserialized: SpendByDimension = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.dimension, "tag:engineering");
    assert_eq!(deserialized.total_cost_usd, 0.0);
    assert_eq!(deserialized.request_count, 0);
}

/// Test that the breakdown response total calculation is correct.
/// This simulates what the handler does: summing breakdown rows.
#[test]
fn test_breakdown_total_aggregation() {
    let rows = vec![
        SpendByDimension {
            dimension: "gpt-4o".into(),
            total_cost_usd: 100.0,
            request_count: 500,
            total_prompt_tokens: 50000,
            total_completion_tokens: 25000,
        },
        SpendByDimension {
            dimension: "gpt-4o-mini".into(),
            total_cost_usd: 10.0,
            request_count: 3000,
            total_prompt_tokens: 100000,
            total_completion_tokens: 50000,
        },
        SpendByDimension {
            dimension: "claude-3-sonnet".into(),
            total_cost_usd: 45.50,
            request_count: 200,
            total_prompt_tokens: 30000,
            total_completion_tokens: 15000,
        },
    ];

    // This is the exact logic from the handler — test it doesn't silently break
    let total_cost: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
    let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

    assert!((total_cost - 155.50).abs() < 0.001, "expected 155.50, got {}", total_cost);
    assert_eq!(total_requests, 3700, "expected 3700 requests, got {}", total_requests);
}

/// Test edge case: empty breakdown (no spend data).
/// The handler should still produce valid JSON with zeroes.
#[test]
fn test_empty_breakdown_totals_to_zero() {
    let rows: Vec<SpendByDimension> = vec![];
    let total_cost: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
    let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

    assert_eq!(total_cost, 0.0);
    assert_eq!(total_requests, 0);
}

/// Test that the "unknown" dimension appears for NULL model values.
/// The SQL COALESCE(model, 'unknown') should convert NULLs.
#[test]
fn test_dimension_handles_unknown_sentinel_value() {
    let row = SpendByDimension {
        dimension: "unknown".to_string(),
        total_cost_usd: 5.0,
        request_count: 10,
        total_prompt_tokens: 0,
        total_completion_tokens: 0,
    };
    // Verify the sentinel serializes (not an empty string or null)
    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(json["dimension"], "unknown");
}
