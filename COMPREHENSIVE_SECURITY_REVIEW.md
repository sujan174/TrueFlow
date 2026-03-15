# Comprehensive Gateway Security Review
## 5-Agent Parallel Analysis - Aggregated Findings

**Review Date:** 2026-03-15
**Agents:** Policy Engine, Spend & Rate Limiting, Proxy Core Flow, Vault & Encryption, API Authentication
**Files Reviewed:** 40+ files across gateway/src/

---

## Executive Summary

| Severity | Count | Description |
|----------|-------|-------------|
| **Critical** | 4 | Security vulnerabilities requiring immediate attention |
| **High** | 13 | Logic bugs and security weaknesses |
| **Medium** | 15 | Edge cases, error handling gaps |
| **Low** | 10 | Code quality, maintainability |

**Cross-Cutting Theme:** Multi-tenancy isolation and cache consistency are the weakest areas.

---

## Critical Findings (4)

### CRIT-1: Multi-Tenancy Isolation Breach
**Domain:** API Authentication
**File:** `gateway/src/api/mod.rs:76-85`
**Description:** `default_project_id()` returns a hardcoded UUID shared by ALL organizations. When handlers use this default instead of explicit project_id, cross-tenant data access is possible.
**Impact:** Token ownership verification at `helpers.rs:48` compares against this shared ID, allowing potential cross-org access.
**Fix:** Remove hardcoded default; require explicit project_id in all queries; add per-org default project mapping.

### CRIT-2: Redis Failure Allows Unlimited Spend (Fail-Open)
**Domain:** Spend & Rate Limiting
**Files:** `gateway/src/proxy/handler/core.rs:1916-1925, 2828-2838, 3135-3145, 3257-3267`
**Description:** When `check_and_increment_spend` returns an error (Redis unavailable), the error is logged but request CONTINUES.
**Impact:** Complete spend cap bypass if Redis fails or becomes unreachable.
**Fix:** Implement fail-closed pattern - deny request when Redis is unavailable.

### CRIT-3: Lua Script Increments Counters Even When Cap Exceeded
**Domain:** Spend & Rate Limiting
**File:** `gateway/src/middleware/spend.rs:356-371`
**Description:** The Lua script Phase 3 ALWAYS increments counters even when Phase 2 detects cap exceeded.
**Impact:** "Phantom spend" - denied requests consume budget, reducing available spend for legitimate requests.
**Fix:** Only increment counters if no cap is exceeded (move INCRBYFLOAT inside the "OK" branch).

### CRIT-4: KEK Not Zeroized on Drop
**Domain:** Vault & Encryption
**File:** `gateway/src/vault/builtin.rs:29-31`
**Description:** `VaultCrypto` struct holds master key encryption key (KEK) but does not implement `Drop` to clear from memory.
**Impact:** Core dumps or swapped memory could expose the master key.
**Fix:**
```rust
impl Drop for VaultCrypto {
    fn drop(&mut self) {
        self.kek.zeroize();
    }
}
```

---

## High Findings (13)

### HIGH-1: OIDC Audience Validation Disabled by Default
**Domain:** API Authentication
**File:** `gateway/src/middleware/oidc.rs:425-433`
**Description:** When `provider.audience` is None, audience validation is completely disabled.
**Impact:** Tokens issued for other clients could be accepted.
**Fix:** Require audience configuration for all OIDC providers.

### HIGH-2: Credential Deletion Missing Project Isolation
**Domain:** API Authentication
**File:** `gateway/src/api/handlers/credentials.rs:114-136`
**Description:** `delete_credential` uses `auth.default_project_id()` without verifying credential belongs to that project.
**Fix:** Look up credential first, verify `credential.project_id` matches before deletion.

### HIGH-3: Policy Operations Use Default Project
**Domain:** API Authentication
**File:** `gateway/src/api/handlers/policies.rs:146, 203`
**Description:** `update_policy` and `delete_policy` use default project ID, bypassing multi-project isolation.
**Fix:** Accept project_id in request; verify policy belongs to project before update/delete.

### HIGH-4: No Rule-Level Short-Circuit for Deny
**Domain:** Policy Engine
**File:** `gateway/src/middleware/engine/mod.rs:37-57`
**Description:** When multiple policies match, ALL actions are accumulated even if one triggers `deny`.
**Impact:** Wasted computation; side effects (webhooks) could fire even when deny is imminent.
**Fix:** Add early termination when deny action is encountered during evaluation.

### HIGH-5: TOCTOU Race in Pre-Flight Spend Check
**Domain:** Spend & Rate Limiting
**File:** `gateway/src/middleware/spend.rs:41-121`
**Description:** `check_spend_cap` reads Redis, then returns. Actual atomic check happens later. Concurrent requests could push spend over limit between checks.
**Fix:** Rely solely on atomic check-and-increment; make pre-flight advisory-only.

### HIGH-6: Fire-and-Forget DB Writes for Spend
**Domain:** Spend & Rate Limiting
**File:** `gateway/src/middleware/spend.rs:479-490`
**Description:** DB persistence spawned in tokio task that swallows errors.
**Impact:** Redis and DB will diverge on write failures.
**Fix:** Implement retry logic or dead-letter queue for failed DB writes.

### HIGH-7: Provider Detection False Positive - Model "o" Prefix
**Domain:** Proxy Core Flow
**File:** `gateway/src/proxy/model_router/mod.rs:129-135`
**Description:** Models like "other-model" could be misdetect as OpenAI o-series.
**Fix:** Add explicit prefix check: `starts_with("o1-") || starts_with("o3-") || starts_with("o4-")`.

### HIGH-8: Streaming No Retry for Early Connection Failures
**Domain:** Proxy Core Flow
**File:** `gateway/src/proxy/upstream.rs:44-45`
**Description:** SSE streams don't retry, but connection failures before any bytes received could safely retry.
**Fix:** Add single retry for connection-level failures before response bytes received.

### HIGH-9: Circuit Breaker Race in Half-Open State
**Domain:** Proxy Core Flow
**File:** `gateway/src/proxy/loadbalancer.rs:2421`
**Description:** In multi-instance deployments, two instances could both allow requests through half-open upstream, exceeding `half_open_max_requests`.
**Fix:** Use Redis INCR for atomic half-open attempt counting across instances.

### HIGH-10: Cache Race During Credential Rotation
**Domain:** Vault & Encryption
**File:** `gateway/src/rotation.rs:202-235`
**Description:** Cache invalidated AFTER DB update. Concurrent requests may fetch stale cached credentials.
**Fix:** Invalidate cache FIRST (both local and Redis), then update DB.

### HIGH-11: Redis Cache Not Invalidated on Credential Delete
**Domain:** Vault & Encryption
**File:** `gateway/src/vault/builtin.rs:156-163`
**Description:** `delete` only soft-deletes in DB; doesn't invalidate Redis cache.
**Fix:** Add cache invalidation to delete method.

### HIGH-12: Plaintext Secret Exposed During Re-encryption
**Domain:** Vault & Encryption
**File:** `gateway/src/rotation.rs:181-191`
**Description:** Plaintext exists in memory during re-encryption; if operation fails, zeroization may not be reached.
**Fix:** Use `Zeroizing<String>` wrapper for automatic zeroization.

### HIGH-13: Rate Limit Increments Counter for Denied Requests
**Domain:** Spend & Rate Limiting
**File:** `gateway/src/proxy/handler/core.rs:557-625`
**Description:** Sliding window rate limiter increments counter BEFORE checking limit.
**Impact:** Denied requests count against limit, faster exhaustion under attack.
**Fix:** Document behavior or consider separate "denied requests" counter.

---

## Medium Findings (15)

| ID | Domain | File | Description |
|----|--------|------|-------------|
| MED-1 | API Auth | `audit.rs:79-87` | SSE stream auth failure silently ignored |
| MED-2 | API Auth | `auth.rs:53` | "superadmin" role creation allowed silently |
| MED-3 | API Auth | `oidc.rs:263-275` | JWT "kid" extracted before verification |
| MED-4 | API Auth | `mod.rs:447-450` | Weak key detection insufficient |
| MED-5 | Policy | `evaluate.rs:47-63` | Empty condition arrays have unexpected semantics |
| MED-6 | Policy | `mod.rs:43-57` | Action ordering not guaranteed across policies |
| MED-7 | Spend | `spend.rs:15-19` | f64 precision for money amounts |
| MED-8 | Spend | `spend.rs:541-556` | Cache stampede on miss |
| MED-9 | Spend | `spend.rs:634-643` | Race in cache invalidation |
| MED-10 | Proxy | `core.rs:2470-2476` | Credential zeroization gap on early return |
| MED-11 | Proxy | `stream_bridge.rs:143` | SSE chunk count before JSON parse |
| MED-12 | Proxy | `stream_bridge.rs:77` | UTF-8 residual buffer unbounded |
| MED-13 | Proxy | `core.rs:1016` | Guardrail threshold validation missing |
| MED-14 | Vault | `redact.rs:26-34` | SSN pattern false positives on 9-digit numbers |
| MED-15 | Vault | `redact.rs:36-37` | Credit card pattern missing Luhn validation |

---

## Low Findings (10)

| ID | Domain | File | Description |
|----|--------|------|-------------|
| LOW-1 | API Auth | `mod.rs:30-32` | Wildcard scope grants full access (document behavior) |
| LOW-2 | API Auth | `mod.rs` | Missing rate limiting on auth endpoints |
| LOW-3 | Policy | `evaluate.rs:95-98` | Unreachable code branch in evaluate_operator |
| LOW-4 | Policy | `operators.rs:210-214` | Aggressive regex cache eviction (clear all vs LRU) |
| LOW-5 | Spend | `cache.rs:125-143` | Legacy fixed-window rate limiter still exists |
| LOW-6 | Spend | `main.rs:401-422` | Local cache eviction relies on background job |
| LOW-7 | Proxy | `streaming.rs:89-90` | Double trim in SSE translation |
| LOW-8 | Proxy | `core.rs:2500-2501` | Timeout calculation overflow potential |
| LOW-9 | Proxy | `core.rs:2372-2378` | Query param credential injection encoding |
| LOW-10 | Vault | `pii/mod.rs:92-118` | NLP entity position mismatch after regex redaction |

---

## Cross-Cutting Issues

### 1. Multi-Tenancy Isolation
**Affected Domains:** API Auth, Policy, Spend
**Pattern:** Default project ID used across multiple handlers bypasses tenant isolation.
**Files:** `api/mod.rs:76-85`, `api/handlers/credentials.rs:114-136`, `api/handlers/policies.rs:146`

### 2. Cache Consistency
**Affected Domains:** Spend, Vault, Proxy
**Pattern:** Cache invalidation happens after DB writes, allowing stale data.
**Files:** `rotation.rs:202-235`, `spend.rs:634-643`, `vault/builtin.rs:156-163`

### 3. Fail-Open Patterns
**Affected Domains:** Spend, Policy
**Pattern:** Errors from external systems (Redis, policy evaluation) allow requests to continue.
**Files:** `proxy/handler/core.rs:1916-1925`, `middleware/engine/mod.rs:37-57`

### 4. Memory Safety for Secrets
**Affected Domains:** Vault, Proxy
**Pattern:** Secrets remain in memory longer than necessary or not zeroized on all paths.
**Files:** `vault/builtin.rs:29-31`, `rotation.rs:181-191`, `proxy/handler/core.rs:2470-2476`

---

## Positive Security Controls Observed

| Domain | Control | File |
|--------|---------|------|
| API Auth | Algorithm confusion prevention | `oidc.rs:320-333` |
| API Auth | JWKS cache auto-refresh | `oidc.rs:212-241` |
| API Auth | SSRF protection | `oidc.rs:117-179` |
| API Auth | Atomic API key revocation | `auth.rs:126-145` |
| API Auth | Timing-safe key comparison | `mod.rs:455-461` |
| Policy | ReDoS protection (1MB limit) | `operators.rs:207` |
| Policy | Glob DoS protection (100K iterations) | `operators.rs:85` |
| Policy | Recursion depth limit (100) | `evaluate.rs:12` |
| Proxy | Security headers allowlist | `core.rs:2349-2354` |
| Proxy | SSRF protection for webhooks | `core.rs:791` |
| Proxy | HITL token re-validation | `core.rs:1637-1687` |
| Proxy | Stream limits (10MB/100 tools) | `stream_bridge.rs` |
| Vault | DEK zeroization | `builtin.rs:64-66` |
| Vault | Envelope encryption | `builtin.rs` |
| Vault | Nonce generation (OsRng) | `builtin.rs:177-181` |

---

## Recommended Fix Priority

### Immediate (Critical)
1. CRIT-2: Implement fail-closed pattern for Redis unavailability
2. CRIT-3: Fix Lua script to not increment on deny
3. CRIT-1: Fix multi-tenancy isolation with per-org default projects
4. CRIT-4: Implement Drop for VaultCrypto to zeroize KEK

### Short-Term (High)
5. HIGH-1: Require OIDC audience validation
6. HIGH-10: Fix cache invalidation ordering in rotation
7. HIGH-4: Add deny short-circuit in policy evaluation
8. HIGH-5: Fix TOCTOU in spend pre-flight check

### Medium-Term
9. All Medium findings
10. Implement comprehensive cache invalidation strategy

### Long-Term
11. All Low findings
12. Code quality improvements

---

## Files Reviewed by Agent

| Agent | Files Reviewed |
|-------|----------------|
| Policy Engine | `middleware/engine/` (4 files), `models/policy.rs`, `middleware/policy.rs` |
| Spend & Rate | `middleware/spend.rs`, `cache.rs`, `main.rs` |
| Proxy Core | `proxy/handler/` (3 files), `proxy/model_router/` (4 files), `proxy/*.rs` |
| Vault & Encryption | `vault/` (2 files), `middleware/redact.rs`, `middleware/pii/`, `rotation.rs` |
| API Auth | `api/mod.rs`, `api/handlers/*.rs` (7 files), `middleware/rbac.rs`, `middleware/oidc.rs`, `middleware/teams.rs` |