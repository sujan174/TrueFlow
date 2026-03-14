# TrueFlow Changelog

## [Unreleased] — 2026-03-14

### Security

**SEC-FIX-6 — Load Balancer: Memory Leak on Token Revocation**

When tokens were revoked, the `LoadBalancer`'s `health` and `counters` DashMaps were
never cleaned up, causing unbounded memory growth proportional to token churn.

Fix: Added `LoadBalancer::cleanup_token(token_id)` method and called it from the
token revoke handler to remove entries from both DashMaps.

Affected files:
- `gateway/src/proxy/loadbalancer.rs` — added `cleanup_token()` method
- `gateway/src/api/handlers/tokens.rs` — call cleanup on token revocation

---

**SEC-FIX-7 — Circuit Breaker: Race Condition in Half-Open Counter**

The `half_open_attempts` counter was checked during `select()` but incremented
afterwards in a separate call, allowing concurrent requests to exceed the
`half_open_max_requests` limit during recovery probing.

Fix: Changed `half_open_attempts` from `u32` to `AtomicU32` and incremented it
atomically within `is_healthy_at()` using fetch_add with compare-and-swap pattern.

Affected files:
- `gateway/src/proxy/loadbalancer.rs` — atomic half_open_attempts
- `gateway/src/proxy/handler/core.rs` — removed separate increment_half_open call

---

**SEC-FIX-8 — Stream Accumulator: Unbounded Memory (DoS Vector)**

`StreamAccumulator` accumulated content without size limits. Malicious or
misconfigured upstream LLM providers could send extremely large streaming
responses, causing memory exhaustion.

Fix: Added `MAX_ACCUMULATED_CONTENT` constant (10MB) and stop accumulating
when exceeded. Added `accumulation_truncated` flag to track this state.

Also fixed unbounded `tool_call_deltas` vector growth by adding
`MAX_TOOL_CALLS` limit (100) with index validation.

Affected files:
- `gateway/src/proxy/stream.rs` — added size limits and validation

---

**SEC-FIX-9 — Dynamic Routing: Fallback Bypasses Health Check**

When all pool targets were unhealthy, `dynamic_route()` returned the fallback
target without checking if its circuit breaker was also open, potentially
routing requests to a known-failing endpoint.

Fix: Added circuit state check for fallback target before using it.

Affected files:
- `gateway/src/proxy/smart_router.rs` — check fallback health

---

### Fixed

**FIX-1 — Circuit Breaker: Fallback to Unhealthy Upstream**

When all upstreams were unhealthy, the proxy handler fell back to the primary
URL regardless of circuit state, defeating the purpose of the circuit breaker.

Fix: Return `AppError::AllUpstreamsExhausted` error instead of falling back
to a potentially broken upstream.

Affected files:
- `gateway/src/proxy/handler/core.rs` — return error on all unhealthy
- `gateway/src/errors.rs` — `AllUpstreamsExhausted` error variant

---

**FIX-2 — Chunk Counter: Potential Overflow**

`chunk_count` was incremented with `+= 1` which could panic on overflow in debug
mode or wrap in release mode after 4+ billion chunks.

Fix: Changed to `saturating_add(1)` for safe handling.

Affected files:
- `gateway/src/proxy/stream.rs` — use saturating_add

---

**FIX-3 — Round-Robin Counter: Document Overflow Behavior**

The `AtomicU64` counter for round-robin selection wraps at 2^64 requests.

Fix: Added comment documenting this intentional behavior.

Affected files:
- `gateway/src/proxy/loadbalancer.rs` — document overflow

---

**FIX-4 — Redis Circuit Breaker: Document Limitation**

`get_distributed_failure_count()` writes to Redis but is never read during
selection, making distributed circuit breaking effectively local-only.

Fix: Added documentation comment explaining the limitation.

Affected files:
- `gateway/src/proxy/loadbalancer.rs` — document Redis CB limitation

---

**FIX-5 — Model Router: Security Hardening**

Improved input validation and error handling across model router components
for provider-specific request/response handling.

Affected files:
- `gateway/src/proxy/model_router/bedrock.rs`
- `gateway/src/proxy/model_router/request.rs`
- `gateway/src/proxy/model_router/streaming.rs`
- `gateway/src/proxy/model_router/url_rewrite.rs`
- `gateway/src/proxy/model_router/tests.rs`

---

## [2026-03-06]

### Security

**SEC-FIX-1 — Approval Decision: Missing Ownership Check**  
`POST /api/v1/approvals/:id/decision`

Previously, any admin with `approvals:write` could approve or reject any approval
request by guessing its UUID, regardless of project ownership.

Fix: The handler now fetches the approval record before acting on it and verifies
that `approval.project_id == auth.default_project_id()`. On mismatch, it returns
`404 Not Found` (not `403`) to prevent cross-project ID enumeration.

Test added: `test_approval_decision_cross_project_returns_404`

---

**SEC-FIX-2 — Circuit Breaker: Missing Ownership Check**  
`GET  /api/v1/tokens/:id/circuit-breaker`  
`PATCH /api/v1/tokens/:id/circuit-breaker`

Previously, both endpoints would return or modify circuit-breaker config for any
token UUID regardless of project ownership, allowing cross-project data reads and
configuration tampering.

Fix: Both handlers now compare `token.project_id` against `auth.default_project_id()`
after fetching the token. On mismatch, they return `404 Not Found`.

Tests added:
- `test_get_cb_config_cross_project_returns_404`
- `test_patch_cb_config_cross_project_returns_404`

---

**SEC-FIX-3 — Policy Versions: Missing Ownership Check**  
`GET /api/v1/policies/:id/versions`

Previously, any caller with `policies:read` scope could enumerate version history
for any policy UUID, including policies from other projects.

Fix: The handler now calls `db.policy_belongs_to_project(id, project_id)` before
returning version history. If the policy does not belong to the caller's project
(whether it exists elsewhere or not), it returns `404 Not Found`.

New DB method: `Db::policy_belongs_to_project(policy_id, project_id) -> bool`

Test added: `test_policy_versions_cross_project_returns_404`

---

**SEC-FIX-4 — Teams and Model Access Groups: Missing Admin Role Check**  
Affected endpoints (8 total):  
- `POST   /api/v1/teams`
- `PUT    /api/v1/teams/:id`
- `DELETE /api/v1/teams/:id`
- `POST   /api/v1/teams/:id/members`
- `DELETE /api/v1/teams/:id/members/:user_id`
- `POST   /api/v1/model-access-groups`
- `PUT    /api/v1/model-access-groups/:id`
- `DELETE /api/v1/model-access-groups/:id`

Previously, these endpoints only required the `tokens:write` scope. A `Member`-role
API key with `tokens:write` could create, modify, or delete teams and model access
groups — which control budget allocation and which AI models tokens can access.

Fix: Added `auth.require_role("admin")?` as the first guard in each of the 8
handlers, before any DB call.

Tests added:
- `test_create_team_requires_admin_role`
- `test_create_model_access_group_requires_admin_role`

---

**SEC-FIX-5 — Anomaly Detection: Cross-Project Redis Data Leak**  
`GET /api/v1/anomalies`

Previously, the anomaly endpoint SCANned Redis for `anomaly:tok:*` keys which
spanned all projects in the Redis instance. Any admin could see velocity anomaly
data for tokens in other projects.

Fix: Changed the Redis key format from `anomaly:tok:{token_id}` to
`anomaly:tok:{project_id}:{token_id}`. The SCAN in the API handler now uses the
project-scoped pattern `anomaly:tok:{project_id}:*`, inherently isolating results
to the caller's project without extra DB lookups.

Affected files:
- `gateway/src/middleware/anomaly.rs` — `record_and_check()` signature updated
  to accept `project_id: &str`; key format changed
- `gateway/src/proxy/handler.rs` — call site updated to pass `token.project_id`
- `gateway/src/api/handlers.rs` — SCAN pattern uses project-scoped prefix

Test added: `test_anomalies_scoped_to_project`

---

### Added

- `Db::get_approval_request(id)` — fetches a single approval request by UUID for
  ownership verification in the approval decision handler
- `Db::policy_belongs_to_project(policy_id, project_id)` — checks if a policy
  belongs to a specific project; used by the policy versions ownership check
- `docs/reference/security.md` — new "Authorization Model" section documenting
  role vs scope semantics, 404-on-mismatch semantics, SuperAdmin scope behavior,
  and anomaly key schema requirements for future contributors
