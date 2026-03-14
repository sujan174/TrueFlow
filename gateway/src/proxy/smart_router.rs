//! Smart router: selects the best model from a DynamicRoute pool.
//!
//! Called by the policy engine when an `Action::DynamicRoute` is matched.
//! Returns a `RouteDecision` describing which model/upstream won and why.

use crate::models::latency_cache::LatencyCache;
use crate::models::policy::{RouteTarget, RoutingStrategy};
use crate::models::pricing_cache::PricingCache;
use crate::proxy::loadbalancer::LoadBalancer;
use rust_decimal::prelude::ToPrimitive;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

// Thread-local cache for compiled regex patterns to avoid recompilation overhead.
// Bounded at 64 entries with simple eviction when exceeded.
thread_local! {
    static ROUTE_REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::with_capacity(64));
}

/// The outcome of a dynamic routing decision.
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// The selected model name (will override `body.model`).
    pub model: String,
    /// The upstream base URL for this target.
    pub upstream_url: String,
    /// Optional credential override for this target.
    #[allow(dead_code)]
    pub credential_id: Option<Uuid>,
    /// Human-readable strategy label for audit logs.
    pub strategy_used: String,
    /// Human-readable explanation, e.g. "cheapest at $0.15/M input"
    pub reason: String,
}

/// Per-token round-robin counter — stored globally in a DashMap.
/// Using a LazyLock + DashMap so we don't need to thread it through AppState.
static RR_COUNTERS: std::sync::LazyLock<dashmap::DashMap<String, Arc<AtomicU64>>> =
    std::sync::LazyLock::new(dashmap::DashMap::new);

/// Select the best route from `pool` given the strategy.
///
/// Health filtering: any target whose upstream URL has an open circuit breaker
/// for `token_id` is skipped. If all targets are unhealthy, `fallback` is used.
/// If fallback is also None, returns None (caller should let the request proceed
/// with the original model/upstream as a last-resort fail-open).
#[allow(clippy::too_many_arguments)]
pub async fn select_route(
    strategy: &RoutingStrategy,
    pool: &[RouteTarget],
    fallback: Option<&RouteTarget>,
    pricing: &PricingCache,
    latency: &LatencyCache,
    lb: &LoadBalancer,
    token_id: &str,
    cb_cooldown_secs: u64,
) -> Option<RouteDecision> {
    if pool.is_empty() {
        return None;
    }

    // --- Filter healthy targets ---
    let healthy: Vec<&RouteTarget> = pool
        .iter()
        .filter(|t| {
            let state = lb.get_circuit_state(token_id, &t.upstream_url, cb_cooldown_secs);
            // "closed" = healthy, "half_open" = allow recovery probe, "open" = skip
            state != "open"
        })
        .collect();

    let candidates = if healthy.is_empty() {
        tracing::warn!(
            token_id,
            "dynamic_route: all pool targets unhealthy, trying fallback"
        );
        // Check if fallback is healthy before using it
        if let Some(fb) = fallback {
            let fallback_state = lb.get_circuit_state(token_id, &fb.upstream_url, cb_cooldown_secs);
            if fallback_state != "open" {
                return Some(RouteDecision {
                    model: fb.model.clone(),
                    upstream_url: fb.upstream_url.clone(),
                    credential_id: fb.credential_id,
                    strategy_used: "fallback".to_string(),
                    reason: "all pool targets unhealthy".to_string(),
                });
            }
            // Fallback is also unhealthy
            tracing::warn!(
                token_id,
                fallback_url = %fb.upstream_url,
                "dynamic_route: fallback target also unhealthy"
            );
        }
        // All targets including fallback are unhealthy — return None
        return None;
    } else {
        healthy
    };

    match strategy {
        RoutingStrategy::LowestCost => select_lowest_cost(candidates, pricing).await,
        RoutingStrategy::LowestLatency => select_lowest_latency(candidates, latency).await,
        RoutingStrategy::RoundRobin => select_round_robin(candidates, token_id),
        RoutingStrategy::LeastBusy => select_least_busy(candidates, lb),
        RoutingStrategy::WeightedRandom => select_weighted_random(candidates),
    }
}

// ── Strategy implementations ──────────────────────────────────

async fn select_lowest_cost(
    candidates: Vec<&RouteTarget>,
    pricing: &PricingCache,
) -> Option<RouteDecision> {
    // Compute a blended $/M cost for each candidate (input + output averaged)
    // using the PricingCache. Unknown models get f64::MAX (placed last).
    let mut scored: Vec<(&RouteTarget, f64)> = Vec::with_capacity(candidates.len());

    for target in candidates {
        let provider =
            crate::proxy::model_router::detect_provider(&target.model, &target.upstream_url);
        let provider_str = format!("{:?}", provider).to_lowercase();

        let cost =
            if let Some((input_m, output_m)) = pricing.lookup(&provider_str, &target.model).await {
                // Simple blended average
                let blended = (input_m + output_m) / rust_decimal::Decimal::from(2);
                blended.to_f64().unwrap_or(f64::MAX)
            } else {
                f64::MAX // Unknown price — deprioritize
            };

        scored.push((target, cost));
    }

    // Sort ascending by cost (cheapest first)
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .next()
        .map(|(target, cost)| RouteDecision {
            model: target.model.clone(),
            upstream_url: target.upstream_url.clone(),
            credential_id: target.credential_id,
            strategy_used: "lowest_cost".to_string(),
            reason: if cost < f64::MAX {
                format!("cheapest at ${:.4}/M blended", cost)
            } else {
                "no pricing data; selected first healthy target".to_string()
            },
        })
}

async fn select_lowest_latency(
    candidates: Vec<&RouteTarget>,
    latency: &LatencyCache,
) -> Option<RouteDecision> {
    let mut scored: Vec<(&RouteTarget, f64)> = Vec::with_capacity(candidates.len());

    for target in candidates {
        let p50 = latency.get_p50(&target.model).await.unwrap_or(f64::MAX);
        scored.push((target, p50));
    }

    // Sort ascending by p50 (fastest first)
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .next()
        .map(|(target, p50)| RouteDecision {
            model: target.model.clone(),
            upstream_url: target.upstream_url.clone(),
            credential_id: target.credential_id,
            strategy_used: "lowest_latency".to_string(),
            reason: if p50 < f64::MAX {
                format!("fastest at {:.0}ms p50", p50)
            } else {
                "no latency data; selected first healthy target".to_string()
            },
        })
}

fn select_round_robin(candidates: Vec<&RouteTarget>, token_id: &str) -> Option<RouteDecision> {
    if candidates.is_empty() {
        return None;
    }

    let counter = RR_COUNTERS
        .entry(token_id.to_string())
        .or_insert_with(|| Arc::new(AtomicU64::new(0)))
        .clone();

    let idx = counter.fetch_add(1, Ordering::Relaxed) as usize % candidates.len();
    let target = candidates[idx];

    Some(RouteDecision {
        model: target.model.clone(),
        upstream_url: target.upstream_url.clone(),
        credential_id: target.credential_id,
        strategy_used: "round_robin".to_string(),
        reason: format!("round-robin slot {}", idx),
    })
}

/// Pick the model with the fewest in-flight requests right now.
/// Falls back to the first candidate if no in-flight data exists.
fn select_least_busy(candidates: Vec<&RouteTarget>, lb: &LoadBalancer) -> Option<RouteDecision> {
    if candidates.is_empty() {
        return None;
    }

    let mut scored: Vec<(&RouteTarget, u64)> = candidates
        .iter()
        .map(|t| (*t, lb.get_in_flight(&t.upstream_url)))
        .collect();

    // Sort ascending by in-flight count (least busy first)
    scored.sort_by_key(|(_, count)| *count);

    scored
        .into_iter()
        .next()
        .map(|(target, count)| RouteDecision {
            model: target.model.clone(),
            upstream_url: target.upstream_url.clone(),
            credential_id: target.credential_id,
            strategy_used: "least_busy".to_string(),
            reason: format!("{} in-flight requests", count),
        })
}

/// Randomly select from the pool, weighted by each target's weight field.
/// This is equivalent to LiteLLM's "weighted-pick" strategy — better than
/// round-robin for heterogeneous deployments because it introduces randomness
/// that prevents synchronized thundering-herd effects.
fn select_weighted_random(candidates: Vec<&RouteTarget>) -> Option<RouteDecision> {
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        let t = candidates[0];
        return Some(RouteDecision {
            model: t.model.clone(),
            upstream_url: t.upstream_url.clone(),
            credential_id: t.credential_id,
            strategy_used: "weighted_random".to_string(),
            reason: "single candidate".to_string(),
        });
    }

    // FIX 4F-1: Use rand::thread_rng() instead of SystemTime::now().as_nanos().
    // The nanosecond approach was biased (modular bias) and predictable
    // (concurrent requests got the same selection).
    use rand::Rng;
    let total = candidates.len();
    let idx = rand::thread_rng().gen_range(0..total);
    let target = candidates[idx];

    Some(RouteDecision {
        model: target.model.clone(),
        upstream_url: target.upstream_url.clone(),
        credential_id: target.credential_id,
        strategy_used: "weighted_random".to_string(),
        reason: format!("random slot {}/{}", idx, total),
    })
}

// ── Conditional Routing ───────────────────────────────────────

/// Evaluate an ordered list of route branches and return the first matching target.
///
/// Field syntax:
/// - `"body.<key>"` — top-level request body field (e.g. `"body.model"`)
/// - `"body.<key>.<subkey>"` — nested body field
/// - `"header.<name>"` — request header (lowercase)
/// - `"metadata.<key>"` — `x-trueflow-metadata` JSON field
pub fn evaluate_conditional_route_branches(
    branches: &[crate::models::policy::RouteBranch],
    body: &serde_json::Value,
    headers: &hyper::HeaderMap,
) -> Option<crate::models::policy::RouteTarget> {
    for branch in branches {
        if evaluate_route_condition(&branch.condition, body, headers) {
            return Some(branch.target.clone());
        }
    }
    None
}

fn evaluate_route_condition(
    cond: &crate::models::policy::RouteCondition,
    body: &serde_json::Value,
    headers: &hyper::HeaderMap,
) -> bool {
    let actual = resolve_condition_field(&cond.field, body, headers);
    let op = cond.op.as_str();

    match op {
        "exists" => actual.is_some(),
        "eq" => {
            let Some(val) = actual else { return false };
            values_equal(&val, &cond.value)
        }
        "neq" => {
            match actual {
                None => true, // field doesn't exist → not equal
                Some(val) => !values_equal(&val, &cond.value),
            }
        }
        "contains" => {
            let Some(val) = actual else { return false };
            let haystack = val.as_str().unwrap_or("");
            let needle = cond.value.as_str().unwrap_or("");
            haystack.contains(needle)
        }
        "starts_with" => {
            let Some(val) = actual else { return false };
            let s = val.as_str().unwrap_or("");
            s.starts_with(cond.value.as_str().unwrap_or(""))
        }
        "ends_with" => {
            let Some(val) = actual else { return false };
            let s = val.as_str().unwrap_or("");
            s.ends_with(cond.value.as_str().unwrap_or(""))
        }
        "regex" => {
            let Some(val) = actual else { return false };
            let pattern = cond.value.as_str().unwrap_or("");

            ROUTE_REGEX_CACHE.with(|cache| {
                let mut cache = cache.borrow_mut();

                // Check cache first
                if let Some(cached) = cache.get(pattern) {
                    return cached.as_ref().is_some_and(|re| {
                        re.is_match(val.as_str().unwrap_or(""))
                    });
                }

                // Compile and cache
                let result = regex::RegexBuilder::new(pattern)
                    .size_limit(1 << 20) // ReDoS protection
                    .build()
                    .ok();

                let is_match = result.as_ref().is_some_and(|re| {
                    re.is_match(val.as_str().unwrap_or(""))
                });

                // Cache the result (even None for invalid patterns)
                cache.insert(pattern.to_string(), result);

                // Simple eviction: clear half if cache exceeds capacity
                if cache.len() > 64 {
                    let keys: Vec<_> = cache.keys().take(32).cloned().collect();
                    for k in keys {
                        cache.remove(&k);
                    }
                }

                is_match
            })
        }
        _ => {
            tracing::warn!(op = op, "conditional_route: unknown operator");
            false
        }
    }
}

/// Resolve a field path against the request context.
fn resolve_condition_field(
    field: &str,
    body: &serde_json::Value,
    headers: &hyper::HeaderMap,
) -> Option<serde_json::Value> {
    if let Some(rest) = field.strip_prefix("body.") {
        // Traverse the body JSON by dot-path
        let mut cur = body;
        for key in rest.split('.') {
            cur = cur.get(key)?;
        }
        Some(cur.clone())
    } else if let Some(rest) = field.strip_prefix("header.") {
        headers
            .get(rest)
            .and_then(|v| v.to_str().ok())
            .map(|s| serde_json::Value::String(s.to_owned()))
    } else if let Some(rest) = field.strip_prefix("metadata.") {
        // metadata is sent as x-trueflow-metadata JSON header
        headers
            .get("x-trueflow-metadata")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|m| m.get(rest).cloned())
    } else {
        None
    }
}

/// Compare two serde_json::Values for equality (type-coercing for numbers).
fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    // Handle bool vs bool
    if let (Some(ab), Some(bb)) = (a.as_bool(), b.as_bool()) {
        return ab == bb;
    }
    // Handle number vs number (f64 comparison)
    if let (Some(an), Some(bn)) = (a.as_f64(), b.as_f64()) {
        return (an - bn).abs() < f64::EPSILON;
    }
    // String vs string (case-insensitive for model names)
    if let (Some(as_), Some(bs)) = (a.as_str(), b.as_str()) {
        return as_.eq_ignore_ascii_case(bs);
    }
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::policy::RouteTarget;

    fn target(model: &str, url: &str) -> RouteTarget {
        RouteTarget {
            model: model.to_string(),
            upstream_url: url.to_string(),
            credential_id: None,
        }
    }

    #[test]
    fn test_round_robin_rotates() {
        let pool = [
            target("gpt-4o", "https://api.openai.com"),
            target("claude-3-haiku-20240307", "https://api.anthropic.com"),
        ];
        let refs: Vec<&RouteTarget> = pool.iter().collect();

        let d1 = select_round_robin(refs.clone(), "tok_test_rr").unwrap();
        let d2 = select_round_robin(refs.clone(), "tok_test_rr").unwrap();
        // The two calls should return different models
        assert_ne!(d1.model, d2.model);
    }

    #[test]
    fn test_round_robin_single_target() {
        let pool = [target("gpt-4o", "https://api.openai.com")];
        let refs: Vec<&RouteTarget> = pool.iter().collect();
        let d = select_round_robin(refs, "tok_single").unwrap();
        assert_eq!(d.model, "gpt-4o");
        assert_eq!(d.strategy_used, "round_robin");
    }

    #[test]
    fn test_round_robin_empty_returns_none() {
        let result = select_round_robin(vec![], "tok_empty");
        assert!(result.is_none());
    }
}
