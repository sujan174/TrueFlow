use serde_json::Value;

use super::super::fields::{self, RequestContext};
use super::operators::{
    check_contains, check_ends_with, check_glob, check_in, check_regex, check_starts_with,
    compare_numeric, values_equal,
};
use crate::models::policy::{Condition, Operator};

/// Maximum recursion depth for nested conditions (All/Any/Not).
/// Prevents stack overflow from deeply nested or malicious policy definitions.
const MAX_RECURSION_DEPTH: u32 = 100;

/// Evaluate a condition against the request context.
/// Public entry point that starts with depth 0.
pub fn evaluate_condition(condition: &Condition, ctx: &RequestContext<'_>) -> bool {
    evaluate_condition_recursive(condition, ctx, 0)
}

/// Internal recursive evaluation with depth tracking.
fn evaluate_condition_recursive(
    condition: &Condition,
    ctx: &RequestContext<'_>,
    depth: u32,
) -> bool {
    // Check recursion depth to prevent stack overflow
    if depth > MAX_RECURSION_DEPTH {
        tracing::warn!(
            depth = depth,
            max_depth = MAX_RECURSION_DEPTH,
            "Policy condition recursion depth exceeded, returning false"
        );
        return false;
    }

    match condition {
        Condition::Always { always } => *always,

        Condition::Check { field, op, value } => {
            let resolved = fields::resolve_field(field, ctx);
            evaluate_operator(op, resolved.as_ref(), value)
        }

        // MED-5: Empty 'all' array is a configuration error that should deny (return false).
        // Previously this used vacuous truth (empty all = true) which is mathematically
        // correct but dangerous in a security context - an empty condition could allow
        // all requests. We now treat empty conditions as a denial for safety.
        // Empty 'any' also returns false, which is the safer default.
        Condition::All { all } => {
            if all.is_empty() {
                tracing::warn!(
                    "MED-5: Policy condition has empty 'all' array — treating as denial (false) for safety. \
                     Empty conditions are likely a configuration error. Please add at least one condition."
                );
                return false; // Safe default: deny on empty condition
            }
            all.iter().all(|c| evaluate_condition_recursive(c, ctx, depth + 1))
        }

        Condition::Any { any } => {
            if any.is_empty() {
                tracing::warn!(
                    "MED-5: Policy condition has empty 'any' array — treating as denial (false) for safety. \
                     Empty conditions are likely a configuration error. Please add at least one condition."
                );
                return false; // Safe default: deny on empty condition
            }
            any.iter().any(|c| evaluate_condition_recursive(c, ctx, depth + 1))
        }

        Condition::Not { not } => !evaluate_condition_recursive(not, ctx, depth + 1),
    }
}

// ── Operator Evaluation ──────────────────────────────────────

/// Compare a resolved field value against an expected value using the given operator.
fn evaluate_operator(op: &Operator, resolved: Option<&Value>, expected: &Value) -> bool {
    match op {
        Operator::Exists => resolved.is_some(),

        _ => {
            let Some(actual) = resolved else {
                return false;
            };
            match op {
                Operator::Eq => values_equal(actual, expected),
                Operator::Neq => !values_equal(actual, expected),
                Operator::Gt => compare_numeric(actual, expected, |a, b| a > b),
                Operator::Gte => compare_numeric(actual, expected, |a, b| a >= b),
                Operator::Lt => compare_numeric(actual, expected, |a, b| a < b),
                Operator::Lte => compare_numeric(actual, expected, |a, b| a <= b),
                Operator::In => check_in(actual, expected),
                Operator::Glob => check_glob(actual, expected),
                Operator::Regex => check_regex(actual, expected),
                Operator::Contains => check_contains(actual, expected),
                Operator::StartsWith => check_starts_with(actual, expected),
                Operator::EndsWith => check_ends_with(actual, expected),
                // This branch should be unreachable (handled in outer match),
                // but provide graceful fallback instead of panic for robustness.
                Operator::Exists => {
                    tracing::error!("Operator::Exists should have been handled in outer match");
                    resolved.is_some()
                }
            }
        }
    }
}
