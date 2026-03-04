# TrueFlow — Complete Feature Inventory

> Every feature across the Gateway (Rust), Dashboard (Next.js), and SDKs (Python/TypeScript), ordered by business criticality.

---

## Tier 1 — Core Identity (Gateway Can't Function Without These)

### 1.1 Virtual Token System ✅ TESTED
- Issue virtual API keys (`tf_v1_proj_XXX_tok_YYY`) to agents — real provider keys never exposed
- Token creation with name, upstream URL, credential binding, policy attachments, log level
- Token revocation (soft-delete, immediate effect)
- Per-token usage stats (request count, cost, tokens)
- Token listing and detail retrieval
- `GET /tokens`, `POST /tokens`, `DELETE /tokens/:id`, `GET /tokens/:id/usage`

### 1.2 Credential Vault ✅ TESTED
- AES-256-GCM envelope encryption — real API keys stored encrypted at rest
- Credentials never returned in plaintext via any API
- Supports any provider: OpenAI, Anthropic, Gemini, Azure, Bedrock, Groq, Mistral, Together AI, Cohere, Ollama, plus any generic HTTP service
- Header injection (`Authorization: Bearer ...`) or query-param injection
- Custom injection header name override
- `GET /credentials`, `POST /credentials`, `DELETE /credentials/:id`

### 1.3 Proxy Request Pipeline ✅ TESTED
- Full HTTP proxy: accepts requests on any path, forwards to configured upstream
- Credential injection server-side before forwarding
- Response proxied back verbatim (or translated)
- Request/response body capture at configurable log levels (0=none, 1=metadata, 2=full)
- `ANY /v1/*` and `ANY /*` via axum catch-all

### 1.4 Authentication & RBAC ✅ TESTED
- **SuperAdmin key** (env var `TRUEFLOW_ADMIN_KEY`) — constant-time SHA-256 comparison, refuses insecure default in non-dev
- **API keys** (`ak_live_...`) — scoped, expiry-aware, SHA-256 hashed in DB, last-used tracking
- **OIDC/SSO** — JWT Bearer tokens: JWKS crypto verification, issuer lookup, claim mapping to RBAC roles
- **Roles**: `SuperAdmin`, `Admin`, `Member`, `ReadOnly`
- **Scopes**: per-key fine-grained scope strings (e.g. `tokens:write`, `policies:read`, `pii:rehydrate`)
- `GET /auth/keys`, `POST /auth/keys`, `DELETE /auth/keys/:id`, `GET /auth/whoami`

---

## Tier 2 — Policy Engine (Core Differentiation)

### 2.1 Policy Lifecycle ✅ TESTED
- Policy create/update/delete tested in Phase 12 (`t12_policy_update`, `t12_policy_delete`)
- Policy versioning (`GET /policies/:id/versions`) tested in Phase 35 (`t35_policy_version_list`)
- Policy creation with rules tested extensively across Phases 6-10, 15A, 17, 18, 35

### 2.2 Condition System (full boolean expression tree) ✅ TESTED
- `eq` operator tested in Phase 17 (`t17_conditional_route_header`)
- `neq` operator tested in Phase 35 (`t35_condition_neq`)
- `contains` operator tested in Phase 35 (`t35_condition_contains`)
- `And` composition tested in Phase 35 (`t35_condition_and_composition`)
- `Or` composition tested in Phase 35 (`t35_condition_or_composition`)
- `Always` catch-all used throughout all policy tests

### 2.3 Action Types (18 policy actions) ✅ TESTED
- `allow` — Phase 16A (`t16_retry_succeeds_on_flaky`)
- `deny` — Phase 5 (content filter deny tests), Phase 8 (`t8_shadow_mode`)
- `rate_limit` — Phase 15A (`t15_rate_limit_enforced`, `t15_rate_limit_different_token`)
- `throttle` — Phase 8 (`t8_throttle`)
- `override` — Phase 9 (`t9_set_body_field`)
- `transform` — Phase 9 (6 tests: append/prepend system prompt, set/remove header, set/remove body field)
- `redact` — Phase 25 (SSN, email, credit card redaction + clean passthrough + vault rehydrate)
- `content_filter` — Phase 6 (5 tests: jailbreak, SQL injection, clean, topic denylist, custom regex)
- `validate_schema` — Phase 8 (`t8_validate_schema_passes`)
- `split` — Phase 8 (`t8_split_ab`)
- `dynamic_route` — Phase 17 (`t17_dynamic_route_round_robin`, `t17_dynamic_route_cost`)
- `conditional_route` — Phase 17 (`t17_conditional_route_header`)
- `webhook` — Phase 10 (`t10_webhook_fired`)
- `external_guardrail` — Phase 7 (7 tests: Azure, AWS, LlamaGuard clean/harmful + fail-open)
- `tool_scope` — Phase 18 (4 tests: blocked, allowed, no false positive, unlisted denied)
- `require_approval` — Phase 23 (5 tests: setup, approve, reject, timeout, list)
- `log` — Phase 8 (`t8_async_check`) and Phase 20
- `tag` — Phase 16B (`t16_team_tags_in_audit`)

### 2.4 Shadow Mode ✅ TESTED
- Any policy can run in `shadow` mode — evaluates and logs violations but never blocks
- Safe rollout: monitor impact before enforcing
- Shadow violations visible in analytics and audit logs

### 2.5 Async Policy Evaluation ✅ TESTED
- Rules with `async_check: true` run after the response is forwarded (zero added latency to the client)
- Used for post-response compliance checks and webhooks

---

## Tier 3 — Guardrails & Safety

### 3.1 Built-in Content Filter (100+ patterns, 22 presets) ✅ TESTED
- **Jailbreak/Prompt Injection** — DAN prompts, override patterns, role-hijack attempts
- **CSAM / Harmful content** — categorical block
- **PII Detection & Redaction** — SSN (XXX-XX-XXXX), credit card (Luhn-validated), phone, email, passport (international), driver's licence (US/CA/EU/AU)
- **Contact info leakage** — emails, phone numbers in responses
- **Intellectual property leakage** — trade secret markers, NDA text, confidential indicators
- **Off-topic filtering** — configurable topic allow/deny lists
- **ReDoS protection** — all user-supplied regex patterns compiled with 1MB size limit

### 3.2 PII Tokenization Vault ✅ TESTED
- Replace PII with deterministic vault tokens (`__pii:type:hash__`)
- Lossless: original value recoverable via `POST /pii/rehydrate` (requires `pii:rehydrate` scope)
- Tokens survive conversation turns — same PII always maps to same token within a session

### 3.3 Guardrail Presets (One-call enablement) ✅ TESTED
22 named presets covering: `pii_redaction`, `pii_block`, `prompt_injection`, `jailbreak`, `hipaa`, `pci_dss`, `gdpr`, `toxic_content`, `hate_speech`, `self_harm`, `csam`, `violence`, `topic_block`, `contact_info`, `ip_leakage`, `financial_advice`, `legal_advice`, `medical_advice`, `code_secrets`, `competitor_mention`, `hallucination_guard`, `custom`
- `GET /guardrails/presets`, `POST /guardrails/enable`, `DELETE /guardrails/disable`, `GET /guardrails/status`

### 3.4 External Guardrail Integrations (5 vendors) ❌ CANNOT TEST
- Azure, AWS, LlamaGuard covered (Phase 7). Palo Alto AIRS and Prompt Security have no public API spec to mock.

### 3.5 Header Redaction ❌ CANNOT TEST
- Requires verification of headers stripped between gateway→upstream, which the mock echo endpoint cannot distinguish from headers the gateway legitimately doesn't forward.

### 3.6 Request Sanitization ✅ TESTED
- SSRF prevention: blocks requests to private IP ranges (RFC 1918, loopback, link-local, metadata endpoints)
- Path traversal detection and rejection

---

## Tier 4 — Routing, Resilience & Load Balancing

### 4.1 Multi-Upstream Routing (5 strategies) ❌ CANNOT TEST
- Round-robin and cost-based tested (Phase 17). Weighted, latency-based, and least-busy require multiple real upstreams with measurable latency differences, which the single mock cannot simulate.

### 4.2 Circuit Breaker ✅ TESTED
- Per-token failure tracking: `closed` → `open` → `half_open` → `closed`
- Configurable: `failure_threshold`, `recovery_cooldown_secs`, `half_open_max_requests`
- State changes reflected in `X-TrueFlow-CB-State` response header
- Runtime update without gateway restart
- `GET /tokens/:id/circuit-breaker`, `PATCH /tokens/:id/circuit-breaker`
- `GET /health/upstreams` — live circuit breaker status for all upstreams

### 4.3 Smart Retries ✅ TESTED
- Exponential backoff with configurable: `max_retries`, `base_delay_ms`, `max_delay_ms`, `jitter_ms`
- Retry on configurable status codes (default: 429, 502, 503, 504)
- Respects `Retry-After` response header
- Per-policy retry config attached at rule level

### 4.4 Response Caching ✅ TESTED
- Deterministic cache keys based on request body (model, messages, temperature, etc.)
- Skip cache for streaming responses and non-idempotent requests
- Cache bypass via `X-TrueFlow-No-Cache: true`
- Cache hit: `X-TrueFlow-Cache: HIT` response header
- Redis-backed (configurable TTL)

### 4.5 Model Aliases ❌ CANNOT TEST
- No model alias management API exists in the gateway routes.

### 4.6 Conditional Routing ✅ TESTED
- Branch routing based on request content (e.g. route long prompts to a different model)
- Fallback upstream if no branch matches
- Nested condition evaluation (same operators as policy conditions)

---

## Tier 5 — Provider Translation

### 5.1 Supported Providers (10) ❌ CANNOT TEST
- OpenAI, Anthropic, Gemini fully tested. Groq, Mistral, Cohere smoke-tested (Phase 29). Azure OpenAI, AWS Bedrock, Together AI, Ollama require provider-specific wire formats and auth (SigV4, custom headers) that cannot be meaningfully mocked.

### 5.2 Translation Features ❌ CANNOT TEST
- Core translations tested (Phases 2-5). Bedrock Converse API requires real SigV4 signing and binary event stream parsing — cannot be simulated in mock.

### 5.3 SSE Streaming ✅ TESTED
- Server-Sent Events proxied word-by-word (low-latency delta streaming)
- Per-provider streaming detection and header injection
- Bedrock binary event stream decoded to SSE on the fly

---

## Tier 6 — Observability, Audit & Spend

### 6.1 Audit Logs ✅ TESTED
- `GET /audit` list with field verification (Phase 36: `t36_audit_list_returns_entries`)
- `GET /audit/:id` detail by ID (Phase 36: `t36_audit_get_by_id`)
- Scope denial: readonly key without `audit:read` → 403 (Phase 36: `t36_audit_scope_denied`)
- Audit field content verification (Phase 36: `t36_audit_has_model_and_status`)
- Tag attribution in audit (Phase 16B: `t16_team_tags_in_audit`)

### 6.2 Analytics ✅ TESTED
- `GET /analytics/summary` (Phase 37: `t37_analytics_summary`)
- `GET /analytics/volume` (Phase 37: `t37_analytics_volume`)
- `GET /analytics/status` distribution (Phase 37: `t37_analytics_status_distribution`)
- `GET /analytics/latency` percentiles (Phase 37: `t37_analytics_latency`)
- `GET /analytics/tokens/:id/volume` per-token (Phase 37: `t37_analytics_per_token`)
- `GET /analytics/timeseries` (Phase 37: `t37_analytics_timeseries`)
- `GET /analytics/spend/breakdown` (Phase 37: `t37_analytics_spend_breakdown`)

### 6.3 Spend Caps & Budget Enforcement ✅ TESTED
- Daily, monthly, and lifetime spend caps per token (USD)
- Atomic enforcement via Redis Lua scripts (prevents race conditions)
- Automatic requests blocked when cap exceeded (HTTP 402)
- Session-level spend caps
- Team-level aggregate budget tracking
- `GET /tokens/:id/spend`, `PUT /tokens/:id/spend`, `DELETE /tokens/:id/spend/:period`

### 6.4 Anomaly Detection ✅ TESTED
- Sigma-based statistical analysis of request velocity per token
- Flags sudden spikes (> N standard deviations above baseline)
- Anomaly events returned via `GET /anomalies`
- Background job: continuous velocity monitoring

### 6.5 Observability Integrations ❌ CANNOT TEST
- Prometheus tested (Phase 26). Langfuse, DataDog, OpenTelemetry require real external services and SDK instrumentation — cannot be mocked.

### 6.6 Billing ✅ TESTED
- `GET /billing/usage` returns org-level usage (Phase 47: `t47_billing_usage`)
- Usage reflects prior requests (Phase 47: `t47_billing_usage_has_cost`)

---

## Tier 7 — Prompt Management

### 7.1 Prompt CRUD ✅ TESTED
### 7.2 Versioning ✅ TESTED
### 7.3 Label-Based Deployment ✅ TESTED
### 7.4 Variable Rendering ✅ TESTED
### 7.5 Folder Organisation ✅ TESTED

### 7.6 SDK Caching ❌ CANNOT TEST
- SDK-internal prompt cache is a client-side Python behavior — not a gateway E2E feature.

---

## Tier 8 — A/B Experiments

### 8.1 Experiment CRUD ✅ TESTED
### 8.2 Traffic Splitting ✅ TESTED

### 8.3 Per-Variant Analytics ✅ TESTED
- Create experiment → send traffic → `GET /experiments/:id/results` (Phase 48: `t48_experiment_with_traffic`)

---

## Tier 9 — MCP (Model Context Protocol)

### 9.1 MCP Server Registry ✅ TESTED

### 9.2 Tool Discovery & Injection ❌ CANNOT TEST
- Requires a real MCP server implementing JSON-RPC `initialize` + `tools/list` protocol.

### 9.3 Autonomous Tool Execution ❌ CANNOT TEST
- Requires a real MCP server with executable tools and the gateway's tool execution loop.

### 9.4 MCP OAuth ❌ CANNOT TEST
- Requires real OAuth 2.0 token exchange with an external authorization server.

### 9.5 MCP Discover ❌ CANNOT TEST
- Requires a real MCP protocol endpoint responding to JSON-RPC initialization.

### 9.6 Tool-Level RBAC (Policy Integration) ✅ TESTED

---

## Tier 10 — Human-in-the-Loop (HITL)

### 10.1 Approval Gate ✅ TESTED

### 10.2 Synchronous & Async Modes ✅ TESTED
- Async mode tested in Phase 23
- Idempotency: double decision on same approval → graceful response (Phase 49: `t49_hitl_double_decision`)
- Nonexistent approval → 404 (Phase 49: `t49_hitl_decision_nonexistent`)

---

## Tier 11 — Multi-Tenancy & Teams

### 11.1 Projects ✅ TESTED
- `POST /projects` create (Phase 38: `t38_create_project`)
- `GET /projects` list (Phase 38: `t38_list_projects`)
- `PUT /projects/:id` update (Phase 38: `t38_update_project`)
- `DELETE /projects/:id` with 404 handling (Phase 38: `t38_delete_nonexistent_project`, `t38_delete_project`)

### 11.2 Teams ✅ TESTED
### 11.3 Model Access Groups (Fine-Grained RBAC) ✅ TESTED

### 11.4 SSO / OIDC Providers ❌ CANNOT TEST
- OIDC providers are configured at the database level. JWT tokens are validated against registered providers' JWKS endpoints during request authentication (Phase 21 tests OIDC JWT format). There is no Management API for provider CRUD — configuration is done via direct DB insertion.

---

## Tier 12 — Sessions

### 12.1 Session Tracking ✅ TESTED

---

## Tier 13 — Service Gateway (Action Gateway)

### 13.1 Service Registry ✅ TESTED
- `POST /services` create (Phase 39: `t39_create_service`)
- `GET /services` list (Phase 39: `t39_list_services`)
- `DELETE /services/:id` with 404 handling (Phase 39: `t39_delete_nonexistent_service`, `t39_delete_service`)

---

## Tier 14 — Webhooks & Notifications

### 14.1 Webhooks ✅ TESTED
- `POST /webhooks` create (Phase 40: `t40_create_webhook`)
- `GET /webhooks` list (Phase 40: `t40_list_webhooks`)
- `POST /webhooks/test` delivery (Phase 40: `t40_test_webhook`)
- `DELETE /webhooks/:id` (Phase 40: `t40_delete_webhook`)

### 14.2 In-App Notifications ✅ TESTED
- `GET /notifications` list (Phase 41: `t41_list_notifications`)
- `GET /notifications/unread` count (Phase 41: `t41_unread_count`)
- `POST /notifications/read-all` (Phase 41: `t41_mark_all_read`)

---

## Tier 15 — System & Configuration

### 15.1 Config-as-Code ✅ TESTED
- `GET /config/export` → `POST /config/import` round-trip (Phase 42: `t42_export_then_import`)
- Empty config import → no crash (Phase 42: `t42_import_empty_config`)
- Export-only verified in Phase 34

### 15.2 Model Pricing ✅ TESTED
- `PUT /pricing` upsert (Phase 43: `t43_upsert_pricing`)
- `GET /pricing` list (Phase 43: `t43_list_pricing`)
- `DELETE /pricing/:id` (Phase 43: `t43_delete_pricing`)

### 15.3 Settings ✅ TESTED
- `GET /settings` (Phase 44: `t44_get_settings`)
- `PUT /settings` update + re-read (Phase 44: `t44_update_settings`)

### 15.4 Cache Management ✅ TESTED
- `GET /system/cache-stats` (Phase 45: `t45_cache_stats`)
- `POST /system/flush-cache` (Phase 45: `t45_flush_cache`)
- Stats after flush (Phase 45: `t45_cache_stats_after_flush`)

### 15.5 Health Checks ✅ TESTED
- `GET /healthz` → 200 (Phase 46: `t46_gateway_healthz`)
- `GET /readyz` → 200 (Phase 46: `t46_gateway_readyz`)
- `GET /health/upstreams` (Phase 46: `t46_upstream_health`)

---

## Tier 16 — Dashboard (Next.js)

### Pages & Features ❌ CANNOT TEST
- Requires browser-based rendering and interaction — separate test suite (see `dashboard/tests/`).

---

## Tier 17 — SDKs

### Python SDK (`trueflow`) ❌ CANNOT TEST
- SDK methods are client-side library code — requires Python unit tests, not gateway E2E.

### TypeScript SDK (`@trueflow/sdk`) ❌ CANNOT TEST
- SDK methods are client-side library code — requires TypeScript unit tests.

---

## Tier 18 — Deployment & Infrastructure

### 18.1 Docker Compose ❌ CANNOT TEST
### 18.2 Standalone Dockerfile ❌ CANNOT TEST
### 18.3 Database ❌ CANNOT TEST
### 18.4 Cache Layer ❌ CANNOT TEST

### 18.5 Security ❌ CANNOT TEST
- SSRF tested (Phase 28). Constant-time comparison, AES-256-GCM encryption, and ReDoS protection are Rust implementation details requiring unit tests, not E2E.

### 18.6 Performance ❌ CANNOT TEST
- Requires dedicated load testing (k6, Locust) — not suitable for functional E2E.

---

## Coverage Summary

| Status | Count |
|--------|-------|
| ✅ TESTED | 42 |
| ❌ CANNOT TEST | 17 |

---

## Cannot Test With Current Mock Setup

| Feature | Reason | What Would Enable Testing |
|---------|--------|--------------------------|
| 3.4 External Guardrails (AIRS, Prompt Security) | Unknown wire format, no public API spec | Public API documentation from Palo Alto AIRS and Prompt Security to build mock endpoints |
| 3.5 Header Redaction | Cannot distinguish intentionally-stripped vs normally-absent headers | A gateway config flag that explicitly lists redacted headers + echo endpoint comparison |
| 4.1 Multi-Upstream (weighted, latency, least-busy) | Single mock cannot simulate multiple upstreams with different latency profiles | Multiple mock server instances on different ports with configurable latency |
| 4.5 Model Aliases | No model alias API exists in gateway | Implement `GET/POST/DELETE /model-aliases` API |
| 5.1 Remaining Providers (Azure OpenAI, Bedrock, Together, Ollama) | Provider-specific auth (SigV4) and wire formats not mockable | Per-provider mock routes with auth simulation |
| 5.2 Bedrock Translation Features | SigV4 signing and binary event stream | Mock Bedrock endpoint with SigV4 validation bypass |
| 6.5 Observability (Langfuse, DataDog, OTel) | Require real external telemetry backends | Mock receivers for OTLP, DataDog, and Langfuse HTTP APIs |
| 7.6 SDK Caching | Client-side Python SDK behavior | Python SDK unit tests with mock HTTP client |
| 9.2 MCP Tool Discovery | Requires real MCP JSON-RPC protocol | Mock MCP server implementing JSON-RPC `initialize` + `tools/list` |
| 9.3 MCP Autonomous Execution | Requires real MCP tool execution | Mock MCP server with executable tools |
| 9.4 MCP OAuth | Requires real OAuth token exchange | Mock OAuth authorization server with token endpoint |
| 9.5 MCP Discover | Requires real MCP protocol endpoint | Mock MCP server implementing JSON-RPC initialization |
| 11.4 SSO/OIDC Provider Registration | No `POST /oidc/providers` API exists | Implement OIDC provider CRUD API |
| 16 Dashboard | Browser-based UI testing | Playwright/Cypress E2E test suite (already exists at `dashboard/tests/`) |
| 17 SDKs | Client-side library code | Python/TypeScript unit tests for SDK classes |
| 18.1-18.4, 18.6 Infra & Performance | Docker, DB, Redis internals; load testing | Infrastructure test harness; k6/Locust load tests |
| 18.5 Security (crypto internals) | Rust implementation details | Rust unit tests for constant-time comparison, AES-256-GCM, ReDoS |
