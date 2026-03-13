use serde_json::Value;

use super::super::fields::{self, RequestContext};
use super::operators::{
    check_contains, check_ends_with, check_glob, check_in, check_regex, check_starts_with,
    compare_numeric, values_equal,
};
use crate::models::policy::{Condition, Operator};

pub fn evaluate_condition(condition: &Condition, ctx: &RequestContext<'_>) -> bool {
    match condition {
        Condition::Always { always } => *always,

        Condition::Check { field, op, value } => {
            let resolved = fields::resolve_field(field, ctx);
            evaluate_operator(op, resolved.as_ref(), value)
        }

        // FIX: Empty 'all' array evaluates to true (vacuous truth in Rust's .all()).
        // This is mathematically correct but almost certainly a configuration error.
        // Log a warning to alert operators. Empty 'any' evaluates to false, also warn.
        Condition::All { all } => {
            if all.is_empty() {
                tracing::warn!(
                    "Policy condition has empty 'all' array — this always matches (likely a configuration error)"
                );
            }
            all.iter().all(|c| evaluate_condition(c, ctx))
        }

        Condition::Any { any } => {
            if any.is_empty() {
                tracing::warn!(
                    "Policy condition has empty 'any' array — this never matches (likely a configuration error)"
                );
            }
            any.iter().any(|c| evaluate_condition(c, ctx))
        }

        Condition::Not { not } => !evaluate_condition(not, ctx),
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
                Operator::Exists => unreachable!(),
            }
        }
    }
}
