//! Policy evaluation facade.
//!
//! This module bridges the legacy handler interface with the new condition→action
//! engine. The proxy handler calls `evaluate_pre_flight()` and `evaluate_post_flight()`
//! and executes the returned actions.

// use crate::cache::TieredCache;
// use crate::errors::AppError;
use crate::models::policy::{EvalOutcome, Phase, Policy};

use super::engine;
use super::fields::RequestContext;

// ── Pre-flight evaluation (before upstream) ──────────────────

/// Evaluate all pre-flight policies and return the outcome.
///
/// This replaces the old `evaluate_rules()` function. The caller must
/// iterate over `outcome.actions` and execute each one.
pub fn evaluate_pre_flight(policies: &[Policy], ctx: &RequestContext<'_>) -> EvalOutcome {
    engine::evaluate_policies(policies, ctx, &Phase::Pre)
}

// ── Post-flight evaluation (after upstream response) ─────────

/// Evaluate all post-flight policies against the response.
pub fn evaluate_post_flight(policies: &[Policy], ctx: &RequestContext<'_>) -> EvalOutcome {
    engine::evaluate_policies(policies, ctx, &Phase::Post)
}

// ── Action Execution Helpers ─────────────────────────────────

/// Parse a duration string like "1m", "30s", "1h" into seconds.
pub fn parse_window_secs(window: &str) -> Option<u64> {
    let window = window.trim();
    if window.is_empty() {
        return None;
    }

    // FIX H-6: Use char boundary instead of byte-offset split_at to prevent
    // panic on multi-byte UTF-8 last characters (e.g., "5秒").
    let last_char = window.chars().next_back()?;
    let num_str = &window[..window.len() - last_char.len_utf8()];
    let num: u64 = num_str.parse().ok()?;

    match last_char {
        's' => Some(num),
        'm' => Some(num * 60),
        'h' => Some(num * 3600),
        'd' => Some(num * 86400),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_window_secs() {
        assert_eq!(parse_window_secs("1s"), Some(1));
        assert_eq!(parse_window_secs("5m"), Some(300));
        assert_eq!(parse_window_secs("2h"), Some(7200));
        assert_eq!(parse_window_secs("1d"), Some(86400));
        assert_eq!(parse_window_secs(""), None);
        assert_eq!(parse_window_secs("abc"), None);
    }
}
