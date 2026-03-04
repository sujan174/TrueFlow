use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// An upstream target parsed from the token's `upstreams` JSONB array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamTarget {
    pub url: String,
    pub credential_id: Option<Uuid>,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_weight() -> u32 { 100 }
fn default_priority() -> u32 { 1 }

// ── Circuit Breaker Config ─────────────────────────────────────

/// Per-token circuit breaker configuration.
/// Stored as JSONB on the `tokens` table. Missing fields use defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Master toggle — when false, all health checks are skipped (simple round-robin).
    #[serde(default = "default_cb_enabled")]
    pub enabled: bool,
    /// Number of consecutive failures before the circuit opens.
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    /// Seconds to wait before trying an unhealthy upstream again (half-open state).
    #[serde(default = "default_recovery_secs")]
    pub recovery_cooldown_secs: u64,
    /// Max requests to allow through in half-open state before deciding recovery.
    #[serde(default = "default_half_open_max")]
    pub half_open_max_requests: u32,
}

fn default_cb_enabled() -> bool { true }
fn default_failure_threshold() -> u32 { 3 }
fn default_recovery_secs() -> u64 { 30 }
fn default_half_open_max() -> u32 { 1 }

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold: default_failure_threshold(),
            recovery_cooldown_secs: default_recovery_secs(),
            half_open_max_requests: default_half_open_max(),
        }
    }
}

/// Health state for a single upstream endpoint.
#[derive(Debug)]
struct UpstreamHealth {
    url: String,
    is_healthy: bool,
    failure_count: u32,
    last_failure: Option<Instant>,
    /// Number of requests allowed through in the current half-open window.
    /// Reset when the circuit closes (mark_healthy) or re-opens (mark_failed).
    half_open_attempts: u32,
}

/// In-memory loadbalancer with circuit-breaker health tracking.
/// Uses weighted round-robin within priority tiers and automatic failover.
pub struct LoadBalancer {
    /// Per-token health status: token_id → Vec<UpstreamHealth>
    health: DashMap<String, Vec<UpstreamHealth>>,
    /// Per-token round-robin counter
    counters: DashMap<String, Arc<AtomicU64>>,
    /// In-flight request count per upstream URL (for least-busy routing)
    in_flight: DashMap<String, AtomicU64>,
}

impl LoadBalancer {
    pub fn new() -> Self {
        Self {
            health: DashMap::new(),
            counters: DashMap::new(),
            in_flight: DashMap::new(),
        }
    }

    /// Select the best upstream target using weighted round-robin within priority tiers.
    /// Returns the index into the `upstreams` slice, or `None` if all are unhealthy.
    /// When `config.enabled` is false, bypasses health checks and uses simple round-robin.
    pub fn select(&self, token_id: &str, upstreams: &[UpstreamTarget], config: &CircuitBreakerConfig) -> Option<usize> {
        tracing::info!(token_id = token_id, upstream_count = upstreams.len(), cb_enabled = config.enabled, "LoadBalancer::select called");

        // When CB is disabled, skip all health tracking and do simple round-robin.
        if !config.enabled {
            if upstreams.is_empty() { return None; }
            if upstreams.len() == 1 { return Some(0); }
            let counter = self.counters
                .entry(token_id.to_string())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)));
            let idx = counter.fetch_add(1, Ordering::Relaxed) as usize % upstreams.len();
            return Some(idx);
        }
        if upstreams.is_empty() {
            return None;
        }
        if upstreams.len() == 1 {
            // Still track health for single-upstream tokens so get_all_status() works
            self.ensure_health(token_id, upstreams);
            return Some(0);
        }

        // Pass cooldown parameter into health checks
        let cooldown = config.recovery_cooldown_secs;

        // Ensure health entries exist
        self.ensure_health(token_id, upstreams);

        // Get health snapshot
        let health = self.health.get(token_id);
        let health_vec = health.as_ref().map(|h| h.value());

        // Find the highest priority tier (lowest number) that has healthy upstreams
        let mut priorities: Vec<u32> = upstreams.iter().map(|u| u.priority).collect();
        priorities.sort();
        priorities.dedup();

        for priority in priorities {
            let candidates: Vec<(usize, &UpstreamTarget)> = upstreams
                .iter()
                .enumerate()
                .filter(|(i, u)| {
                    u.priority == priority && self.is_healthy_at(health_vec, *i, &u.url, cooldown, config.half_open_max_requests)
                })
                .collect();

            if candidates.is_empty() {
                continue; // all upstreams at this priority are unhealthy, try next tier
            }

            // Weighted round-robin among candidates
            let counter = self
                .counters
                .entry(token_id.to_string())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)));
            let round = counter.fetch_add(1, Ordering::Relaxed);

            // Build weight table
            let total_weight: u32 = candidates.iter().map(|(_, u)| u.weight).sum();
            if total_weight == 0 {
                return candidates.first().map(|(i, _)| *i);
            }

            let target = (round % total_weight as u64) as u32;
            let mut cumulative = 0u32;
            for (idx, upstream) in &candidates {
                cumulative += upstream.weight;
                if target < cumulative {
                    return Some(*idx);
                }
            }

            // Fallback: first candidate
            return candidates.first().map(|(i, _)| *i);
        }

        // All tiers exhausted — try recovery on highest priority
        // Return the first upstream that has cooled down
        for (i, upstream) in upstreams.iter().enumerate() {
            if self.check_recovery(token_id, &upstream.url, cooldown) {
                return Some(i);
            }
        }

        None
    }

    /// Mark an upstream as failed. Opens the circuit once `config.failure_threshold` failures accumulate.
    /// No-op when CB is disabled (`config.enabled == false`).
    pub fn mark_failed(&self, token_id: &str, url: &str, config: &CircuitBreakerConfig) {
        if !config.enabled {
            return;
        }
        if let Some(mut healths) = self.health.get_mut(token_id) {
            if let Some(h) = healths.iter_mut().find(|h| h.url == url) {
                h.failure_count += 1;
                h.last_failure = Some(Instant::now());
                if h.failure_count >= config.failure_threshold {
                    h.is_healthy = false;
                    h.half_open_attempts = 0; // Reset for next half-open window
                    tracing::warn!(
                        token_id = token_id,
                        url = url,
                        failures = h.failure_count,
                        threshold = config.failure_threshold,
                        "circuit breaker OPENED: upstream marked unhealthy"
                    );
                }
            }
        }
    }

    /// Mark an upstream as healthy. Resets the circuit breaker.
    pub fn mark_healthy(&self, token_id: &str, url: &str) {
        if let Some(mut healths) = self.health.get_mut(token_id) {
            if let Some(h) = healths.iter_mut().find(|h| h.url == url) {
                if !h.is_healthy {
                    tracing::info!(
                        token_id = token_id,
                        url = url,
                        "circuit breaker CLOSED: upstream recovered"
                    );
                }
                h.is_healthy = true;
                h.failure_count = 0;
                h.last_failure = None;
                h.half_open_attempts = 0;
            }
        }
    }

    /// Ensure health entries exist for the token's upstreams.
    fn ensure_health(&self, token_id: &str, upstreams: &[UpstreamTarget]) {
        self.health.entry(token_id.to_string()).or_insert_with(|| {
            tracing::info!(token_id = token_id, "Initializing health map for token");
            upstreams
                .iter()
                .map(|u| UpstreamHealth {
                    url: u.url.clone(),
                    is_healthy: true,
                    failure_count: 0,
                    last_failure: None,
                    half_open_attempts: 0,
                })
                .collect()
        });
    }

    /// Check if an upstream at a given index is considered healthy.
    /// `half_open_max` limits the number of probe requests allowed through
    /// during the half-open recovery window.
    fn is_healthy_at(
        &self,
        health_vec: Option<&Vec<UpstreamHealth>>,
        idx: usize,
        url: &str,
        cooldown_secs: u64,
        half_open_max: u32,
    ) -> bool {
        if let Some(healths) = health_vec {
            if let Some(h) = healths.iter().find(|h| h.url == url) {
                if h.is_healthy {
                    return true;
                }
                // Check if cooldown has passed (half-open state)
                if let Some(last) = h.last_failure {
                    if last.elapsed().as_secs() >= cooldown_secs {
                        // B9-1 FIX: enforce half_open_max_requests limit
                        if h.half_open_attempts < half_open_max {
                            return true; // allow probe (half-open, under limit)
                        }
                        return false; // half-open limit reached
                    }
                }
                return false;
            }
        }
        // No health data — assume healthy
        let _ = idx;
        true
    }

    /// Check if an unhealthy upstream has cooled down enough for a recovery attempt.
    fn check_recovery(&self, token_id: &str, url: &str, cooldown_secs: u64) -> bool {
        if let Some(healths) = self.health.get(token_id) {
            if let Some(h) = healths.iter().find(|h| h.url == url) {
                if let Some(last) = h.last_failure {
                    return last.elapsed().as_secs() >= cooldown_secs;
                }
            }
        }
        true
    }

    /// Get the circuit breaker state for a specific upstream.
    /// Returns `"closed"` (healthy), `"open"` (unhealthy), or `"half_open"` (cooling down).
    /// Returns `"closed"` if no health data exists yet.
    pub fn get_circuit_state(&self, token_id: &str, url: &str, cooldown_secs: u64) -> &'static str {
        if let Some(healths) = self.health.get(token_id) {
            if let Some(h) = healths.iter().find(|h| h.url == url) {
                if h.is_healthy {
                    return "closed";
                }
                if let Some(last) = h.last_failure {
                    if last.elapsed().as_secs() >= cooldown_secs {
                        return "half_open";
                    }
                }
                return "open";
            }
        }
        "closed"
    }

    // ── In-Flight Request Tracking (for LeastBusy routing) ───────

    /// Increment the half-open attempt counter for an upstream.
    /// Called by the handler when a request is routed to a half-open upstream.
    pub fn increment_half_open(&self, token_id: &str, url: &str) {
        if let Some(mut healths) = self.health.get_mut(token_id) {
            if let Some(h) = healths.iter_mut().find(|h| h.url == url) {
                if !h.is_healthy {
                    h.half_open_attempts += 1;
                }
            }
        }
    }

    /// Increment the in-flight counter for an upstream URL.
    /// Call at the start of a proxy request.
    pub fn increment_in_flight(&self, url: &str) {
        self.in_flight
            .entry(url.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the in-flight counter for an upstream URL.
    /// Call when the proxy request completes (success or failure).
    pub fn decrement_in_flight(&self, url: &str) {
        if let Some(counter) = self.in_flight.get(url) {
            // Avoid wrapping below zero
            let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v > 0 { Some(v - 1) } else { Some(0) }
            });
        }
    }

    /// Get the current in-flight count for an upstream URL.
    pub fn get_in_flight(&self, url: &str) -> u64 {
        self.in_flight
            .get(url)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

/// Parse upstreams from token JSONB. Returns empty vec if null or invalid.
pub fn parse_upstreams(upstreams_json: Option<&serde_json::Value>) -> Vec<UpstreamTarget> {
    match upstreams_json {
        Some(val) => serde_json::from_value::<Vec<UpstreamTarget>>(val.clone()).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Status snapshot of a single upstream (for dashboard API).
#[derive(Debug, Clone, Serialize)]
pub struct UpstreamStatus {
    pub token_id: String,
    pub url: String,
    pub is_healthy: bool,
    pub failure_count: u32,
    pub cooldown_remaining_secs: Option<u64>,
}

impl LoadBalancer {
    /// Return a snapshot of all tracked upstream health status.
    pub fn get_all_status(&self) -> Vec<UpstreamStatus> {
        tracing::info!(map_size = self.health.len(), "LoadBalancer::get_all_status called");
        let mut statuses = Vec::new();
        for entry in self.health.iter() {
            let token_id = entry.key().clone();
            for h in entry.value().iter() {
                let cooldown = if !h.is_healthy {
                    h.last_failure.map(|lf| {
                        let elapsed = lf.elapsed().as_secs();
                        let default_cooldown = default_recovery_secs();
                        default_cooldown.saturating_sub(elapsed)
                    })
                } else {
                    None
                };

                statuses.push(UpstreamStatus {
                    token_id: token_id.clone(),
                    url: h.url.clone(),
                    is_healthy: h.is_healthy,
                    failure_count: h.failure_count,
                    cooldown_remaining_secs: cooldown,
                });
            }
        }
        statuses
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_upstreams(n: usize) -> Vec<UpstreamTarget> {
        (0..n)
            .map(|i| UpstreamTarget {
                url: format!("https://api{}.example.com", i),
                credential_id: None,
                weight: 100,
                priority: 1,
            })
            .collect()
    }

    #[test]
    fn test_select_single_upstream() {
        let lb = LoadBalancer::new();
        let upstreams = make_upstreams(1);
        assert_eq!(lb.select("tok1", &upstreams, &CircuitBreakerConfig::default()), Some(0));
    }

    #[test]
    fn test_select_empty_returns_none() {
        let lb = LoadBalancer::new();
        assert_eq!(lb.select("tok1", &[], &CircuitBreakerConfig::default()), None);
    }

    #[test]
    fn test_round_robin_distributes() {
        let lb = LoadBalancer::new();
        let upstreams = make_upstreams(3);
        let config = CircuitBreakerConfig::default();
        let mut counts = [0u32; 3];
        for _ in 0..300 {
            if let Some(idx) = lb.select("tok1", &upstreams, &config) {
                counts[idx] += 1;
            }
        }
        // Each should get roughly 100 selections
        for count in &counts {
            assert!(*count > 50, "count {} is too low", count);
        }
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = vec![
            UpstreamTarget {
                url: "https://primary.com".into(),
                credential_id: None,
                weight: 100,
                priority: 1,
            },
            UpstreamTarget {
                url: "https://backup.com".into(),
                credential_id: None,
                weight: 100,
                priority: 1,
            },
        ];

        // Warm up health entries
        lb.select("tok1", &upstreams, &config);

        // Fail primary multiple times
        for _ in 0..config.failure_threshold {
            lb.mark_failed("tok1", "https://primary.com", &config);
        }

        // Now selections should avoid primary
        let mut primary_count = 0;
        for _ in 0..20 {
            if let Some(idx) = lb.select("tok1", &upstreams, &config) {
                if idx == 0 {
                    primary_count += 1;
                }
            }
        }
        assert_eq!(primary_count, 0, "primary should be avoided after circuit opens");
    }

    #[test]
    fn test_mark_healthy_resets_circuit() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = make_upstreams(2);

        lb.select("tok1", &upstreams, &config);

        for _ in 0..config.failure_threshold {
            lb.mark_failed("tok1", "https://api0.example.com", &config);
        }

        // Mark healthy again
        lb.mark_healthy("tok1", "https://api0.example.com");

        // Should now be selectable again
        let mut found = false;
        for _ in 0..20 {
            if lb.select("tok1", &upstreams, &config) == Some(0) {
                found = true;
                break;
            }
        }
        assert!(found, "recovered upstream should be selectable");
    }

    #[test]
    fn test_priority_tiers() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = vec![
            UpstreamTarget {
                url: "https://primary.com".into(),
                credential_id: None,
                weight: 100,
                priority: 1,
            },
            UpstreamTarget {
                url: "https://backup.com".into(),
                credential_id: None,
                weight: 100,
                priority: 2,  // lower priority (higher number)
            },
        ];

        // Should always prefer priority 1
        for _ in 0..20 {
            assert_eq!(lb.select("tok1", &upstreams, &config), Some(0));
        }
    }

    #[test]
    fn test_failover_to_lower_priority() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = vec![
            UpstreamTarget {
                url: "https://primary.com".into(),
                credential_id: None,
                weight: 100,
                priority: 1,
            },
            UpstreamTarget {
                url: "https://backup.com".into(),
                credential_id: None,
                weight: 100,
                priority: 2,
            },
        ];

        lb.select("tok1", &upstreams, &config);

        // Kill primary
        for _ in 0..config.failure_threshold {
            lb.mark_failed("tok1", "https://primary.com", &config);
        }

        // Should failover to backup
        assert_eq!(lb.select("tok1", &upstreams, &config), Some(1));
    }

    #[test]
    fn test_weighted_distribution() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = vec![
            UpstreamTarget {
                url: "https://heavy.com".into(),
                credential_id: None,
                weight: 70,
                priority: 1,
            },
            UpstreamTarget {
                url: "https://light.com".into(),
                credential_id: None,
                weight: 30,
                priority: 1,
            },
        ];

        let mut counts = [0u32; 2];
        for _ in 0..1000 {
            if let Some(idx) = lb.select("tok1", &upstreams, &config) {
                counts[idx] += 1;
            }
        }

        // Heavy should get ~70% (700 ± 100)
        assert!(counts[0] > 600, "heavy count {} too low", counts[0]);
        assert!(counts[0] < 800, "heavy count {} too high", counts[0]);
    }

    #[test]
    fn test_parse_upstreams_valid() {
        let json = serde_json::json!([
            {"url": "https://api.openai.com", "weight": 70, "priority": 1},
            {"url": "https://backup.openai.com", "weight": 30, "priority": 2}
        ]);
        let upstreams = parse_upstreams(Some(&json));
        assert_eq!(upstreams.len(), 2);
        assert_eq!(upstreams[0].weight, 70);
        assert_eq!(upstreams[1].priority, 2);
    }

    #[test]
    fn test_parse_upstreams_null() {
        assert!(parse_upstreams(None).is_empty());
    }

    #[test]
    fn test_parse_upstreams_invalid() {
        let json = serde_json::json!("not an array");
        assert!(parse_upstreams(Some(&json)).is_empty());
    }

    #[test]
    fn test_cb_disabled_bypasses_health() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig { enabled: false, ..Default::default() };
        let upstreams = vec![
            UpstreamTarget { url: "https://a.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://b.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];

        lb.select("tok1", &upstreams, &config);
        // Even after marking failed, disabled CB should still route to all upstreams
        lb.mark_failed("tok1", "https://a.com", &config); // no-op when disabled

        let mut a_count = 0;
        for _ in 0..20 {
            if lb.select("tok1", &upstreams, &config) == Some(0) {
                a_count += 1;
            }
        }
        // With CB disabled, round-robin means both get selected
        assert!(a_count > 0, "CB disabled: upstream A should still be routable");
    }

    #[test]
    fn test_cb_config_default() {
        let config = CircuitBreakerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.recovery_cooldown_secs, 30);
        assert_eq!(config.half_open_max_requests, 1);
    }

    #[test]
    fn test_cb_config_from_json() {
        let json = serde_json::json!({"enabled": false, "failure_threshold": 5});
        let config: CircuitBreakerConfig = serde_json::from_value(json).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.failure_threshold, 5);
        // Defaults for missing fields
        assert_eq!(config.recovery_cooldown_secs, 30);
        assert_eq!(config.half_open_max_requests, 1);
    }

    #[test]
    fn test_get_circuit_state_closed_by_default() {
        let lb = LoadBalancer::new();
        // No health data yet — should return "closed"
        assert_eq!(lb.get_circuit_state("tok1", "https://api.com", 30), "closed");
    }

    #[test]
    fn test_get_circuit_state_transitions() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = vec![
            UpstreamTarget { url: "https://primary.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://backup.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];

        // Initialize health tracking
        lb.select("tok1", &upstreams, &config);

        // Initially closed
        assert_eq!(lb.get_circuit_state("tok1", "https://primary.com", config.recovery_cooldown_secs), "closed");

        // Fail until circuit opens
        for _ in 0..config.failure_threshold {
            lb.mark_failed("tok1", "https://primary.com", &config);
        }

        // Now open
        assert_eq!(lb.get_circuit_state("tok1", "https://primary.com", config.recovery_cooldown_secs), "open");

        // Mark healthy → should be closed again
        lb.mark_healthy("tok1", "https://primary.com");
        assert_eq!(lb.get_circuit_state("tok1", "https://primary.com", config.recovery_cooldown_secs), "closed");
    }

    #[test]
    fn test_custom_failure_threshold() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 5,  // Higher than default 3
            recovery_cooldown_secs: 30,
            half_open_max_requests: 1,
        };
        let upstreams = vec![
            UpstreamTarget { url: "https://primary.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://backup.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];

        lb.select("tok1", &upstreams, &config);

        // Fail 3 times (below threshold of 5) — should still be healthy
        for _ in 0..3 {
            lb.mark_failed("tok1", "https://primary.com", &config);
        }
        assert_eq!(lb.get_circuit_state("tok1", "https://primary.com", config.recovery_cooldown_secs), "closed",
                   "Circuit should still be closed at 3 failures when threshold is 5");

        // Fail 2 more times to hit threshold of 5
        for _ in 0..2 {
            lb.mark_failed("tok1", "https://primary.com", &config);
        }
        assert_eq!(lb.get_circuit_state("tok1", "https://primary.com", config.recovery_cooldown_secs), "open",
                   "Circuit should open after 5 failures");
    }

    #[test]
    fn test_mark_failed_noop_when_disabled() {
        let lb = LoadBalancer::new();
        let enabled_config = CircuitBreakerConfig::default();
        let disabled_config = CircuitBreakerConfig { enabled: false, ..Default::default() };
        let upstreams = vec![
            UpstreamTarget { url: "https://api.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://backup.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];

        // Initialize health with enabled config
        lb.select("tok1", &upstreams, &enabled_config);

        // Mark failed with DISABLED config — should be no-op
        for _ in 0..10 {
            lb.mark_failed("tok1", "https://api.com", &disabled_config);
        }

        // Circuit should still be closed because mark_failed was no-op
        assert_eq!(lb.get_circuit_state("tok1", "https://api.com", 30), "closed",
                   "mark_failed should be no-op when CB is disabled");
    }

    #[test]
    fn test_get_all_status_returns_entries() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig::default();
        let upstreams = vec![
            UpstreamTarget { url: "https://api.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://backup.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];

        // Initialize health tracking
        lb.select("tok1", &upstreams, &config);

        let statuses = lb.get_all_status();
        assert_eq!(statuses.len(), 2, "Should have 2 upstream entries");
        assert!(statuses.iter().all(|s| s.is_healthy), "All should be healthy initially");
        assert!(statuses.iter().all(|s| s.failure_count == 0), "All should have 0 failures");

        // Fail one upstream
        for _ in 0..config.failure_threshold {
            lb.mark_failed("tok1", "https://api.com", &config);
        }

        let statuses = lb.get_all_status();
        let failed = statuses.iter().find(|s| s.url == "https://api.com").unwrap();
        assert!(!failed.is_healthy, "Failed upstream should be unhealthy");
        assert_eq!(failed.failure_count, config.failure_threshold);
        assert!(failed.cooldown_remaining_secs.is_some(), "Should have cooldown remaining");
    }

    #[test]
    fn test_cb_config_roundtrip_serialization() {
        let config = CircuitBreakerConfig {
            enabled: false,
            failure_threshold: 10,
            recovery_cooldown_secs: 120,
            half_open_max_requests: 3,
        };
        let json = serde_json::to_value(&config).unwrap();
        let deserialized: CircuitBreakerConfig = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.enabled, false);
        assert_eq!(deserialized.failure_threshold, 10);
        assert_eq!(deserialized.recovery_cooldown_secs, 120);
        assert_eq!(deserialized.half_open_max_requests, 3);
    }

    #[test]
    fn test_cb_empty_json_uses_defaults() {
        let config: CircuitBreakerConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(config.enabled);
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.recovery_cooldown_secs, 30);
        assert_eq!(config.half_open_max_requests, 1);
    }

    // ── Chaos: 429 Failover & Total Outage ─────────────────────

    /// Provider A gets 429 rate-limited (3 failures) → LB must route 100% to Provider B.
    #[test]
    fn test_lb_failover_on_429_to_backup_provider() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 3,
            recovery_cooldown_secs: 60,
            half_open_max_requests: 1,
        };
        let upstreams = vec![
            UpstreamTarget { url: "https://api.openai.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://backup.azure.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];
        lb.select("tok_prod", &upstreams, &config);

        // Simulate 3 consecutive 429s
        for _ in 0..3 {
            lb.mark_failed("tok_prod", "https://api.openai.com", &config);
        }

        let mut openai_count = 0;
        for _ in 0..20 {
            if let Some(idx) = lb.select("tok_prod", &upstreams, &config) {
                if idx == 0 { openai_count += 1; }
            }
        }
        assert_eq!(openai_count, 0, "OpenAI (circuit OPEN) should receive 0 requests");
    }

    /// All upstreams failed → LB must return None (no healthy target available).
    #[test]
    fn test_lb_all_upstreams_failed_returns_none() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 2,
            recovery_cooldown_secs: 3600,
            half_open_max_requests: 1,
        };
        let upstreams = vec![
            UpstreamTarget { url: "https://a.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://b.com".into(), credential_id: None, weight: 100, priority: 1 },
        ];
        lb.select("tok1", &upstreams, &config);
        for _ in 0..2 {
            lb.mark_failed("tok1", "https://a.com", &config);
            lb.mark_failed("tok1", "https://b.com", &config);
        }
        assert!(lb.select("tok1", &upstreams, &config).is_none(),
            "All upstreams failed — should return None");
    }

    /// After cooldown=0, a failed upstream should become eligible for half-open retry.
    #[test]
    fn test_lb_circuit_recovery_after_cooldown() {
        let lb = LoadBalancer::new();
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            recovery_cooldown_secs: 0,
            half_open_max_requests: 1,
        };
        let upstreams = vec![
            UpstreamTarget { url: "https://primary.com".into(), credential_id: None, weight: 100, priority: 1 },
            UpstreamTarget { url: "https://backup.com".into(), credential_id: None, weight: 100, priority: 2 },
        ];
        lb.select("tok1", &upstreams, &config);
        lb.mark_failed("tok1", "https://primary.com", &config);

        let mut found_primary = false;
        for _ in 0..10 {
            if lb.select("tok1", &upstreams, &config) == Some(0) {
                found_primary = true;
                break;
            }
        }
        assert!(found_primary, "Primary should be retryable after cooldown=0");
    }

    // ── In-Flight Tracking (Least Busy) ────────────────────────

    #[test]
    fn test_in_flight_increment_and_decrement() {
        let lb = LoadBalancer::new();
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 0);

        lb.increment_in_flight("https://api.openai.com");
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 1);

        lb.increment_in_flight("https://api.openai.com");
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 2);

        lb.decrement_in_flight("https://api.openai.com");
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 1);

        lb.decrement_in_flight("https://api.openai.com");
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 0);
    }

    #[test]
    fn test_in_flight_decrement_does_not_go_negative() {
        let lb = LoadBalancer::new();
        lb.decrement_in_flight("https://api.openai.com");
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 0);

        lb.increment_in_flight("https://api.openai.com");
        lb.decrement_in_flight("https://api.openai.com");
        lb.decrement_in_flight("https://api.openai.com");
        assert_eq!(lb.get_in_flight("https://api.openai.com"), 0);
    }

    #[test]
    fn test_in_flight_independent_per_url() {
        let lb = LoadBalancer::new();
        lb.increment_in_flight("https://api.openai.com");
        lb.increment_in_flight("https://api.openai.com");
        lb.increment_in_flight("https://api.anthropic.com");

        assert_eq!(lb.get_in_flight("https://api.openai.com"), 2);
        assert_eq!(lb.get_in_flight("https://api.anthropic.com"), 1);
        assert_eq!(lb.get_in_flight("https://unknown.com"), 0);
    }
}

