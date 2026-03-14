# Implementation Plan: Routing & Circuit Breaker Fixes

**Spec:** `docs/superpowers/specs/2026-03-14-routing-circuit-breaker-fixes-design.md`
**Date:** 2026-03-14

---

## Phase 0: Setup

### Step 0.1: Create Git Worktree
- Create isolated worktree for development
- Branch name: `fix/routing-circuit-breaker-issues`

**Commands:**
```bash
git worktree add .claude/worktrees/routing-fixes -b fix/routing-circuit-breaker-issues
cd .claude/worktrees/routing-fixes
```

---

## Phase 1: High Severity Fixes

### Step 1.1: Fix Memory Leak - Token Health State Cleanup

**File:** `gateway/src/proxy/loadbalancer.rs`
- Add `cleanup_token(token_id: &str)` method to `LoadBalancer` impl
- Method should remove entries from both `self.health` and `self.counters` DashMaps

**File:** `gateway/src/store/postgres/tokens.rs`
- Locate the revoke_token handler
- Add call to `state.lb.cleanup_token(&id)` after existing `cleanup_round_robin_counter` call

**Verification:** `cargo check`

---

### Step 1.2: Fix Race Condition in Half-Open Counter

**File:** `gateway/src/proxy/loadbalancer.rs`
- Change `half_open_attempts` field in `UpstreamHealth` from `u32` to `AtomicU32`
- Update `is_healthy_at()` to increment atomically with fetch_add
- Add logic to decrement if limit exceeded
- Update any other references to `half_open_attempts` to use atomic operations

**File:** `gateway/src/proxy/handler/core.rs`
- Find and remove the `increment_half_open()` call (around line 2422)
- The counter is now incremented atomically during selection

**Verification:** `cargo check`

---

## Phase 2: Medium Severity Fixes

### Step 2.1: Fix Fallback to Unhealthy Upstream

**File:** `gateway/src/errors.rs`
- Add new error variant `AllUpstreamsExhausted { details: Option<serde_json::Value> }`

**File:** `gateway/src/proxy/handler/core.rs`
- Find the fallback logic (around line 1806)
- Replace fallback with `Err(AppError::AllUpstreamsExhausted { ... })`

**Verification:** `cargo check`

---

### Step 2.2: Document Redis Circuit Breaker Limitation

**File:** `gateway/src/proxy/loadbalancer.rs`
- Add documentation comment above `get_distributed_failure_count()` explaining the limitation

**Verification:** `cargo check`

---

### Step 2.3: Fix Fallback Bypasses Health Check

**File:** `gateway/src/proxy/smart_router.rs`
- In `dynamic_route()` function, check fallback's circuit state before using it
- Return `None` if fallback is also unhealthy

**Verification:** `cargo check`

---

### Step 2.4: Fix Unbounded Memory Accumulation

**File:** `gateway/src/proxy/stream.rs`
- Add constant `MAX_ACCUMULATED_CONTENT: usize = 10 * 1024 * 1024`
- Add field `accumulation_truncated: bool` to `StreamAccumulator`
- Add size check in content append locations (push_sse_line, etc.)
- Log warning when limit exceeded

**Verification:** `cargo check`

---

### Step 2.5: Fix Unbounded Tool Calls Vector

**File:** `gateway/src/proxy/stream.rs`
- Add constant `MAX_TOOL_CALLS: usize = 100`
- Add index validation before vector expansion
- Log warning when index exceeds limit

**Verification:** `cargo check`

---

## Phase 3: Low Severity Fixes

### Step 3.1: Fix Counter Overflow in Round-Robin

**File:** `gateway/src/proxy/loadbalancer.rs`
- Use `wrapping_add(1)` on the counter result to document intentional behavior

**Verification:** `cargo check`

---

### Step 3.2: Add Regex Cache

**File:** `gateway/src/proxy/smart_router.rs`
- Add thread-local `ROUTE_REGEX_CACHE`
- Modify regex branch in `evaluate_condition` to use cache
- Add simple eviction when cache exceeds 64 entries

**Verification:** `cargo check`

---

### Step 3.3: Fix Chunk Counter Overflow

**File:** `gateway/src/proxy/stream.rs`
- Change `self.chunk_count += 1` to `self.chunk_count = self.chunk_count.saturating_add(1)`

**Verification:** `cargo check`

---

### Step 3.4: Fix Misleading Test Comment

**File:** `gateway/src/proxy/stream.rs`
- Update comment to correctly describe that chunk_count only increments after successful JSON parse

**Verification:** `cargo check`

---

## Phase 4: Testing & Verification

### Step 4.1: Full Compilation Check
```bash
cargo check --all-targets
```

### Step 4.2: Run Test Suite
```bash
cargo test
```

### Step 4.3: Run Clippy
```bash
cargo clippy --all-targets
```

---

## Phase 5: Commit & Cleanup

### Step 5.1: Commit Changes
- Stage all modified files
- Create descriptive commit message

### Step 5.2: Exit Worktree
- Return to main working directory
- Decide whether to keep or remove worktree

---

## Summary

| Phase | Steps | Files Modified |
|-------|-------|----------------|
| 0: Setup | 1 | worktree creation |
| 1: High | 2 | loadbalancer.rs, core.rs, tokens.rs |
| 2: Medium | 5 | errors.rs, loadbalancer.rs, smart_router.rs, stream.rs, core.rs |
| 3: Low | 4 | loadbalancer.rs, smart_router.rs, stream.rs |
| 4: Test | 3 | verification |
| 5: Commit | 2 | git operations |

**Total Steps:** 17
**Files Modified:** 6