# Fix Design: Routing & Circuit Breaker Issues

**Date:** 2026-03-14
**Scope:** `loadbalancer.rs`, `smart_router.rs`, `stream.rs`, `core.rs`, `tokens.rs`

## Summary

This spec addresses 12 issues identified in the code review of the routing and circuit breaker components. Issues are organized by severity with corresponding fix strategies.

---

## High Severity Fixes

### Fix 1: Memory Leak - Token Health State Cleanup

**Issue:** `LoadBalancer::health` and `counters` DashMaps grow unbounded when tokens are revoked.

**Files:**
- `gateway/src/proxy/loadbalancer.rs`
- `gateway/src/store/postgres/tokens.rs`

**Solution:**
1. Add `LoadBalancer::cleanup_token(token_id: &str)` method that removes entries from both `health` and `counters` DashMaps
2. Call this method from `tokens.rs` revoke handler alongside existing `cleanup_round_robin_counter`

**Code changes:**
```rust
// loadbalancer.rs - add to LoadBalancer impl
pub fn cleanup_token(&self, token_id: &str) {
    self.health.remove(token_id);
    self.counters.remove(token_id);
}

// tokens.rs - in revoke handler, after cleanup_round_robin_counter call
state.lb.cleanup_token(&id);
```

---

### Fix 2: Race Condition in Half-Open Counter

**Issue:** `half_open_attempts` is checked before selection but incremented after, allowing concurrent requests to exceed `half_open_max_requests`.

**Files:**
- `gateway/src/proxy/loadbalancer.rs`
- `gateway/src/proxy/handler/core.rs`

**Solution:**
1. Change `half_open_attempts` from `u32` to `AtomicU32` in `UpstreamHealth`
2. Increment atomically within `is_healthy_at()` using fetch_add
3. Remove the separate `increment_half_open()` call from `core.rs`

**Code changes:**
```rust
// loadbalancer.rs - UpstreamHealth struct
pub struct UpstreamHealth {
    // ... other fields
    pub half_open_attempts: AtomicU32, // was u32
}

// loadbalancer.rs - is_healthy_at function
if h.half_open_attempts.fetch_add(1, Ordering::AcqRel) >= half_open_max {
    h.half_open_attempts.fetch_sub(1, Ordering::AcqRel);
    return false; // exceeded limit
}
// ... proceed with half-open logic

// core.rs - remove increment_half_open call (around line 2422)
// The counter is now incremented atomically during selection
```

---

## Medium Severity Fixes

### Fix 3: Fallback to Unhealthy Upstream

**Issue:** When all upstreams are unhealthy, code falls back to primary URL regardless of circuit state.

**Files:**
- `gateway/src/errors.rs`
- `gateway/src/proxy/handler/core.rs`

**Solution:** Return a proper error instead of sending traffic to a known-failing upstream.

**Code changes:**
```rust
// errors.rs - add new error variant
#[error("All upstreams exhausted")]
AllUpstreamsExhausted {
    details: Option<serde_json::Value>,
},

// core.rs - replace fallback logic (around line 1806)
} else {
    return Err(AppError::AllUpstreamsExhausted {
        details: Some(serde_json::json!({
            "reason": "all_upstreams_unhealthy",
            "token_id": token.id,
        })),
    });
}
```

---

### Fix 4: Redis Circuit Breaker State Not Read

**Issue:** `get_distributed_failure_count()` is never called in selection path - incomplete feature.

**Files:**
- `gateway/src/proxy/loadbalancer.rs`

**Solution:** Add documentation comment explaining the limitation. Completing this feature requires architectural changes (async in sync context) and is out of scope for this fix.

**Code changes:**
```rust
// loadbalancer.rs - add doc comment above get_distributed_failure_count
/// Note: Distributed failure counts are currently written to Redis for external
/// monitoring but not read during selection. This feature is planned for future
/// implementation with async health checks.
```

---

### Fix 5: Fallback Bypasses Health Check (smart_router.rs)

**Issue:** When all pool targets unhealthy, returns fallback without checking its health.

**Files:**
- `gateway/src/proxy/smart_router.rs`

**Solution:** Check fallback's circuit state before using it.

**Code changes:**
```rust
// smart_router.rs - around line 73
let candidates = if healthy.is_empty() {
    tracing::warn!(token_id, "dynamic_route: all pool targets unhealthy, trying fallback");
    if let Some(fb) = fallback {
        let state = lb.get_circuit_state(
            token_id,
            &fb.upstream_url,
            cb_cooldown_secs.unwrap_or(30),
        );
        if state != "open" {
            return Some(RouteDecision {
                model: fb.model.clone(),
                upstream_url: fb.upstream_url.clone(),
                credential_id: fb.credential_id,
                strategy_used: "fallback".to_string(),
                reason: "all pool targets unhealthy".to_string(),
            });
        }
    }
    return None; // All targets including fallback are unhealthy
}
```

---

### Fix 6: Unbounded Memory Accumulation

**Issue:** `StreamAccumulator` accumulates content without size limit - DoS vector.

**Files:**
- `gateway/src/proxy/stream.rs`

**Solution:** Add configurable maximum (default 10MB) and stop accumulating when exceeded.

**Code changes:**
```rust
// stream.rs - add constant
const MAX_ACCUMULATED_CONTENT: usize = 10 * 1024 * 1024; // 10MB

// stream.rs - in push_sse_line and content append locations
if self.content.len() + content.len() > MAX_ACCUMULATED_CONTENT {
    tracing::warn!(
        accumulated = self.content.len(),
        max = MAX_ACCUMULATED_CONTENT,
        "Stream content exceeded maximum, stopping accumulation"
    );
    self.accumulation_truncated = true;
    return; // Stop accumulating but continue streaming
}

// Add flag to StreamAccumulator
pub accumulation_truncated: bool,
```

---

### Fix 7: Unbounded Tool Calls Vector

**Issue:** Malicious upstream can force large allocation via high `index` value.

**Files:**
- `gateway/src/proxy/stream.rs`

**Solution:** Add maximum tool calls limit (default 100).

**Code changes:**
```rust
// stream.rs - add constant
const MAX_TOOL_CALLS: usize = 100;

// stream.rs - in tool call delta expansion (lines 184-191, 242-249)
if index >= MAX_TOOL_CALLS {
    tracing::warn!(
        index,
        max = MAX_TOOL_CALLS,
        "Tool call index exceeds maximum, skipping"
    );
    return;
}
while self.tool_call_deltas.len() <= index {
    self.tool_call_deltas.push(ToolCallDelta { ... });
}
```

---

## Low Severity Fixes

### Fix 8: Counter Overflow in Round-Robin

**Issue:** `u64` counter wraps at 2^64 requests.

**Files:**
- `gateway/src/proxy/loadbalancer.rs`

**Solution:** Use explicit wrapping arithmetic to document intentional behavior.

**Code changes:**
```rust
// loadbalancer.rs - line 172 area
let round = counter.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
```

---

### Fix 9: Regex Recompilation Overhead

**Issue:** Regex patterns compiled on every request without caching.

**Files:**
- `gateway/src/proxy/smart_router.rs`

**Solution:** Add thread-local cache similar to `operators.rs`.

**Code changes:**
```rust
// smart_router.rs - add at module level
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static ROUTE_REGEX_CACHE: RefCell<HashMap<String, Option<Regex>>> =
        RefCell::new(HashMap::with_capacity(64));
}

// In evaluate_condition, regex branch
"regex" => {
    let Some(val) = actual else { return false };
    let pattern = cond.value.as_str().unwrap_or("");

    ROUTE_REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(pattern) {
            return cached.as_ref().map_or(false, |re| {
                re.is_match(val.as_str().unwrap_or(""))
            });
        }

        let result = regex::RegexBuilder::new(pattern)
            .size_limit(1 << 20)
            .build()
            .ok();

        let is_match = result.as_ref().map_or(false, |re| {
            re.is_match(val.as_str().unwrap_or(""))
        });

        cache.insert(pattern.to_string(), result);
        if cache.len() > 64 {
            // Simple eviction: clear half
            let keys: Vec<_> = cache.keys().take(32).cloned().collect();
            for k in keys {
                cache.remove(&k);
            }
        }

        is_match
    })
}
```

---

### Fix 10: Chunk Counter Overflow

**Issue:** `chunk_count: u32` can overflow on 4B+ chunks.

**Files:**
- `gateway/src/proxy/stream.rs`

**Solution:** Use saturating add.

**Code changes:**
```rust
// stream.rs - line 133
self.chunk_count = self.chunk_count.saturating_add(1);
```

---

### Fix 11: Misleading Test Comment

**Issue:** Comment says chunk_count increments before parse, but code does after.

**Files:**
- `gateway/src/proxy/stream.rs`

**Solution:** Fix the comment to match actual behavior.

**Code changes:**
```rust
// stream.rs - line 628-629
// chunk_count only increments after successful JSON parse
// malformed chunks are discarded safely and not counted
```

---

## Implementation Order

1. **High severity** (Fixes 1-2): Memory leak and race condition
2. **Medium severity** (Fixes 3-7): Circuit breaker and DoS protections
3. **Low severity** (Fixes 8-11): Code quality improvements

## Testing Strategy

- **High + Medium:** Run `cargo test` and verify existing tests pass
- **Low:** Compilation only (`cargo check`)

## Files Modified

| File | Fixes |
|------|-------|
| `gateway/src/proxy/loadbalancer.rs` | 1, 2, 4, 8 |
| `gateway/src/proxy/smart_router.rs` | 5, 9 |
| `gateway/src/proxy/stream.rs` | 6, 7, 10, 11 |
| `gateway/src/proxy/handler/core.rs` | 2, 3 |
| `gateway/src/store/postgres/tokens.rs` | 1 |
| `gateway/src/errors.rs` | 3 |