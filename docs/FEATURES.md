# AILink — Complete Feature Inventory

> Every feature across the Gateway (Rust), Dashboard (Next.js), and SDKs (Python/TypeScript), ordered by business criticality.

---

## Tier 1 — Core Identity (Gateway Can't Function Without These)

### 1.1 Virtual Token System
- Issue virtual API keys (`ailink_v1_proj_XXX_tok_YYY`) to agents — real provider keys never exposed
- Token creation with name, upstream URL, credential binding, policy attachments, log level
- Token revocation (soft-delete, immediate effect)
- Per-token usage stats (request count, cost, tokens)
- Token listing and detail retrieval
- `GET /tokens`, `POST /tokens`, `DELETE /tokens/:id`, `GET /tokens/:id/usage`

### 1.2 Credential Vault
- AES-256-GCM envelope encryption — real API keys stored encrypted at rest
- Credentials never returned in plaintext via any API
- Supports any provider: OpenAI, Anthropic, Gemini, Azure, Bedrock, Groq, Mistral, Together AI, Cohere, Ollama, plus any generic HTTP service
- Header injection (`Authorization: Bearer ...`) or query-param injection
- Custom injection header name override
- `GET /credentials`, `POST /credentials`, `DELETE /credentials/:id`

### 1.3 Proxy Request Pipeline
- Full HTTP proxy: accepts requests on any path, forwards to configured upstream
- Credential injection server-side before forwarding
- Response proxied back verbatim (or translated)
- Request/response body capture at configurable log levels (0=none, 1=metadata, 2=full)
- `ANY /v1/*` and `ANY /*` via axum catch-all

### 1.4 Authentication & RBAC
- **SuperAdmin key** (env var `AILINK_ADMIN_KEY`) — constant-time SHA-256 comparison, refuses insecure default in non-dev
- **API keys** (`ak_live_...`) — scoped, expiry-aware, SHA-256 hashed in DB, last-used tracking
- **OIDC/SSO** — JWT Bearer tokens: JWKS crypto verification, issuer lookup, claim mapping to RBAC roles
- **Roles**: `SuperAdmin`, `Admin`, `Member`, `ReadOnly`
- **Scopes**: per-key fine-grained scope strings (e.g. `tokens:write`, `policies:read`, `pii:rehydrate`)
- `GET /auth/keys`, `POST /auth/keys`, `DELETE /auth/keys/:id`, `GET /auth/whoami`

---

## Tier 2 — Policy Engine (Core Differentiation)

### 2.1 Policy Lifecycle
- Named, versioned policies with full edit history
- Two modes: `enforce` (blocks/modifies traffic) and `shadow` (logs matches, no blocking)
- Rules evaluated in order, first match wins
- Policies bind to tokens or scoped globally
- Config-as-Code: export/import policies as YAML or JSON
- `GET /policies`, `POST /policies`, `PUT /policies/:id`, `DELETE /policies/:id`, `GET /policies/:id/versions`

### 2.2 Condition System (full boolean expression tree)
Conditions can be composed with `And`, `Or`, `Not`, plus a single `Check` leaf and `Always` catch-all.

**Operators available on any field:**
| Operator | Description |
|----------|-------------|
| `eq` | Deep equality with string/number coercion |
| `neq` | Not equal |
| `gt` / `gte` | Numeric greater-than |
| `lt` / `lte` | Numeric less-than |
| `contains` | Substring or array membership |
| `not_contains` | Negated contains |
| `starts_with` | Prefix check |
| `ends_with` | Suffix check |
| `in` | Value in array |
| `not_in` | Value not in array |
| `regex` | PCRE regex (1MB size limit to prevent ReDoS) |
| `glob` | Glob pattern (`*` and `?`) |
| `exists` | Field is present |
| `not_exists` | Field is absent |

**Addressable fields:** `request.method`, `request.path`, `request.body.*`, `request.headers.*`, `response.status`, `response.body.*`, `response.headers.*`, `token.id`, `token.name`, `token.spend.daily`, `token.spend.monthly`, `session.id`, and any JSON path in the body.

### 2.3 Action Types (18 policy actions)

| Action | Phase | Description |
|--------|-------|-------------|
| `allow` | pre/post | Explicit no-op allow; short-circuit further rules |
| `deny` | pre/post | Return HTTP error (default 403) with custom message |
| `rate_limit` | pre | Token-bucket rate limiting (window + max requests or `per_token`) |
| `throttle` | pre | Artificial delay (`delay_ms`) before forwarding |
| `override` | pre | Set/replace body fields (e.g. force model downgrade) |
| `transform` | pre/post | Set headers, append to system prompt, replace body fields |
| `redact` | pre/post | Inline PII redaction on specified fields; or block on match |
| `content_filter` | pre/post | Block jailbreak, CSAM, harmful content, PII, off-topic, contact info, IP leakage |
| `validate_schema` | pre/post | JSON Schema validation of request/response body |
| `split` | pre | Weighted A/B traffic split across variants (A/B experiments) |
| `dynamic_route` | pre | Runtime routing: round-robin, weighted, latency-based, cost-based, least-busy |
| `conditional_route` | pre | Branch routing: evaluate sub-conditions, route to different upstreams |
| `webhook` | pre/post | Fire HTTP webhook; configurable timeout; `on_fail`: fail-open or fail-closed |
| `external_guardrail` | pre/post | Call external vendor APIs (Azure, AWS, LlamaGuard, Palo Alto AIRS, Prompt Security) |
| `tool_scope` | pre | Allow/block specific tool/function names (cross-provider) |
| `require_approval` | pre | HITL gate: pause request, wait for human decision |
| `log` | pre/post | Force a specific log level for this request; add structured tags |
| `tag` | pre/post | Attach metadata key/value to the audit log entry |

### 2.4 Shadow Mode
- Any policy can run in `shadow` mode — evaluates and logs violations but never blocks
- Safe rollout: monitor impact before enforcing
- Shadow violations visible in analytics and audit logs

### 2.5 Async Policy Evaluation
- Rules with `async_check: true` run after the response is forwarded (zero added latency to the client)
- Used for post-response compliance checks and webhooks

---

## Tier 3 — Guardrails & Safety

### 3.1 Built-in Content Filter (100+ patterns, 22 presets)
- **Jailbreak/Prompt Injection** — DAN prompts, override patterns, role-hijack attempts
- **CSAM / Harmful content** — categorical block
- **PII Detection & Redaction** — SSN (XXX-XX-XXXX), credit card (Luhn-validated), phone, email, passport (international), driver's licence (US/CA/EU/AU)
- **Contact info leakage** — emails, phone numbers in responses
- **Intellectual property leakage** — trade secret markers, NDA text, confidential indicators
- **Off-topic filtering** — configurable topic allow/deny lists
- **ReDoS protection** — all user-supplied regex patterns compiled with 1MB size limit

### 3.2 PII Tokenization Vault
- Replace PII with deterministic vault tokens (`__pii:type:hash__`)
- Lossless: original value recoverable via `POST /pii/rehydrate` (requires `pii:rehydrate` scope)
- Tokens survive conversation turns — same PII always maps to same token within a session

### 3.3 Guardrail Presets (One-call enablement)
22 named presets covering: `pii_redaction`, `pii_block`, `prompt_injection`, `jailbreak`, `hipaa`, `pci_dss`, `gdpr`, `toxic_content`, `hate_speech`, `self_harm`, `csam`, `violence`, `topic_block`, `contact_info`, `ip_leakage`, `financial_advice`, `legal_advice`, `medical_advice`, `code_secrets`, `competitor_mention`, `hallucination_guard`, `custom`
- `GET /guardrails/presets`, `POST /guardrails/enable`, `DELETE /guardrails/disable`, `GET /guardrails/status`

### 3.4 External Guardrail Integrations (5 vendors)
- **Azure Content Safety** — categories: Hate, Violence, Sexual, Self-Harm; configurable threshold
- **AWS Bedrock Guardrails** — Claude-grade safety via Bedrock's guardrail API
- **LlamaGuard** — Meta's open-source safety model (self-hosted or Together AI)
- **Palo Alto AIRS** — enterprise AI security platform
- **Prompt Security** — real-time prompt injection and data leakage prevention
- All support `on_fail`: `block` (deny on vendor error) or `pass` (fail-open)

### 3.5 Header Redaction
- Strip/mask sensitive request and response headers before logging
- Configurable list of headers to redact (defaults: `Authorization`, `x-api-key`, `x-admin-key`)
- Log-level-aware: full logs at level ≥ 2, masked at level 1

### 3.6 Request Sanitization
- SSRF prevention: blocks requests to private IP ranges (RFC 1918, loopback, link-local, metadata endpoints)
- Path traversal detection and rejection

---

## Tier 4 — Routing, Resilience & Load Balancing

### 4.1 Multi-Upstream Routing (5 strategies)
- **Round-robin** — equal distribution across upstreams
- **Weighted** — percentage-based traffic split
- **Latency-based** — always route to the fastest upstream (P95 tracking)
- **Cost-based** — route to the cheapest upstream (per-model pricing)
- **Least-busy** — route to the upstream with fewest in-flight requests
- P95 latency cache: per-upstream rolling window tracked in Redis

### 4.2 Circuit Breaker
- Per-token failure tracking: `closed` → `open` → `half_open` → `closed`
- Configurable: `failure_threshold`, `recovery_cooldown_secs`, `half_open_max_requests`
- State changes reflected in `X-AILink-CB-State` response header
- Runtime update without gateway restart
- `GET /tokens/:id/circuit-breaker`, `PATCH /tokens/:id/circuit-breaker`
- `GET /health/upstreams` — live circuit breaker status for all upstreams

### 4.3 Smart Retries
- Exponential backoff with configurable: `max_retries`, `base_delay_ms`, `max_delay_ms`, `jitter_ms`
- Retry on configurable status codes (default: 429, 502, 503, 504)
- Respects `Retry-After` response header
- Per-policy retry config attached at rule level

### 4.4 Response Caching
- Deterministic cache keys based on request body (model, messages, temperature, etc.)
- Skip cache for streaming responses and non-idempotent requests
- Cache bypass via `X-AILink-No-Cache: true`
- Cache hit: `X-AILink-Cache: HIT` response header
- Redis-backed (configurable TTL)

### 4.5 Model Aliases
- Map alias names to real model identifiers (e.g. `"smart"` → `"gpt-4o"`)
- Swap providers without any agent code changes
- `GET /model-aliases`, `POST /model-aliases`, `DELETE /model-aliases/:name`

### 4.6 Conditional Routing
- Branch routing based on request content (e.g. route long prompts to a different model)
- Fallback upstream if no branch matches
- Nested condition evaluation (same operators as policy conditions)

---

## Tier 5 — Provider Translation

### 5.1 Supported Providers (10)
| Provider | Auto-detect | Format Translation | Streaming |
|----------|-------------|-------------------|-----------|
| OpenAI | `gpt-*`, `o1-*`, `o3-*`, `o4-*`, `text-*`, `tts-*`, `whisper-*`, `dall-e-*` | Passthrough | ✅ |
| Azure OpenAI | URL: `.openai.azure.com` | Passthrough + URL rewrite | ✅ |
| Anthropic | `claude-*` | ✅ Full bidirectional | ✅ |
| Google Gemini | `gemini-*` | ✅ Full bidirectional | ✅ |
| AWS Bedrock | `anthropic.*`, `amazon.*`, `meta.*`, `cohere.*` | ✅ Converse API | ✅ event stream |
| Groq | URL: `api.groq.com` | Passthrough | ✅ |
| Mistral | `mistral-*`, `mixtral-*` | Passthrough | ✅ |
| Together AI | Slash-separated (`meta-llama/*`, `Qwen/*`) | Passthrough | ✅ |
| Cohere | `command-r*`, `command-*` | Passthrough | ✅ |
| Ollama | URL: `localhost:11434` | Passthrough | ✅ |

### 5.2 Translation Features
- **OpenAI ↔ Anthropic**: full message format, system prompt, tool calls/results, streaming deltas, stop reasons, usage
- **OpenAI ↔ Gemini**: contents/parts, system instruction, functionDeclarations, tool_choice→functionCallingConfig, response_format→responseMimeType+schema, streaming candidates
- **OpenAI ↔ Bedrock**: Converse API, SigV4 auth, binary event stream for streaming, tool use format
- Multimodal: image_url (HTTP and base64 data URIs) translated per-provider
- Tool/function call format translated: OpenAI `tools[]` ↔ Anthropic `tools[]` ↔ Gemini `functionDeclarations[]`
- URL rewriting: correct endpoint paths per provider (e.g. `/v1/messages`, `:generateContent`, Converse API)

### 5.3 SSE Streaming
- Server-Sent Events proxied word-by-word (low-latency delta streaming)
- Per-provider streaming detection and header injection
- Bedrock binary event stream decoded to SSE on the fly

---

## Tier 6 — Observability, Audit & Spend

### 6.1 Audit Logs
- Immutable request/response audit trail in PostgreSQL (partitioned by month)
- Every proxied request: who (token), what (method, path, upstream status), when, which policy triggered, latency, cost, tokens
- Full body capture at log level ≥ 2
- `GET /audit?limit=&offset=&token_id=`, `GET /audit/:id`
- **Real-time SSE stream**: `GET /audit/stream` — live log feed to dashboard

### 6.2 Analytics
| Endpoint | Description |
|----------|-------------|
| `GET /analytics/summary` | Total requests, errors, cost, tokens (all time) |
| `GET /analytics/timeseries` | Per-bucket: requests, errors, cost, latency, tokens |
| `GET /analytics/volume` | Hourly request counts (last 24h) |
| `GET /analytics/status` | Status code distribution (2xx/4xx/5xx) |
| `GET /analytics/latency` | P50, P90, P99, mean latency (ms) |
| `GET /analytics/spend/breakdown` | Cost breakdown by model, token, or project |
| `GET /analytics/tokens` | Per-token request volume and error rates |
| `GET /analytics/tokens/:id/volume` | Volume over time for one token |
| `GET /analytics/tokens/:id/status` | Status distribution for one token |
| `GET /analytics/tokens/:id/latency` | Latency percentiles for one token |
| `GET /analytics/experiments` | A/B variant metrics (requests, latency, cost, error rate) |

### 6.3 Spend Caps & Budget Enforcement
- Daily, monthly, and lifetime spend caps per token (USD)
- Atomic enforcement via Redis Lua scripts (prevents race conditions)
- Automatic requests blocked when cap exceeded (HTTP 402)
- Session-level spend caps
- Team-level aggregate budget tracking
- `GET /tokens/:id/spend`, `PUT /tokens/:id/spend`, `DELETE /tokens/:id/spend/:period`

### 6.4 Anomaly Detection
- Sigma-based statistical analysis of request velocity per token
- Flags sudden spikes (> N standard deviations above baseline)
- Anomaly events returned via `GET /anomalies`
- Background job: continuous velocity monitoring

### 6.5 Observability Integrations
- **Prometheus** — `GET /metrics` exposes: `ailink_requests_total`, `ailink_request_duration_seconds`, `ailink_upstream_errors_total`, `ailink_active_tokens`, `ailink_cache_hits_total`, `ailink_cache_misses_total` (no auth required)
- **Langfuse** — traces exported: prompts, completions, costs, latency per session
- **DataDog** — log ingestion via DataDog agent integration
- **OpenTelemetry** — distributed traces via Jaeger/OTLP
- **Webhooks** — event-driven notifications: `policy_violation`, `spend_cap_exceeded`, `rate_limit_exceeded`, `hitl_requested`, `token_created`

### 6.6 Billing
- `GET /billing/usage?period=YYYY-MM` — org-level total requests, tokens, spend for a given month

---

## Tier 7 — Prompt Management

### 7.1 Prompt CRUD
- Create prompts with name, optional slug (auto-generated), description, folder path, tags
- List all prompts, filterable by folder
- Get full prompt with latest version details
- Update metadata (name, description, folder, tags)
- Soft-delete
- `GET /prompts`, `POST /prompts`, `GET /prompts/:id`, `PUT /prompts/:id`, `DELETE /prompts/:id`

### 7.2 Versioning
- Every `POST /prompts/:id/versions` creates a new immutable version
- Stores: model, messages, temperature, max_tokens, top_p, tools, commit_message
- Versions are immutable; a "rollback" is just deploying an old version
- `GET /prompts/:id/versions`, `POST /prompts/:id/versions`, `GET /prompts/:id/versions/:version`

### 7.3 Label-Based Deployment
- Labels are human-readable pointers to versions: `production`, `staging`, `canary`
- Atomic promotion — exactly one version holds a label at a time
- Zero-downtime rollout: deploy to `staging`, test, then promote to `production`
- `POST /prompts/:id/deploy` → `{ "version": N, "label": "production" }`

### 7.4 Variable Rendering
- `{{variable_name}}` syntax in message content fields
- Server-side substitution on render
- GET render (variables as query params) or POST render (variables in JSON body)
- Returns OpenAI-compatible payload (`model`, `messages`, `temperature`, `max_tokens`, etc.) ready for direct use
- `GET /prompts/by-slug/:slug/render`, `POST /prompts/by-slug/:slug/render`

### 7.5 Folder Organisation
- Hierarchical folder paths: `/support`, `/finance/billing`, etc.
- `GET /prompts/folders` — unique folder list for navigation
- Ahead of all major competitors (Portkey, Helicone, LangFuse all lack folders)

### 7.6 SDK Caching
- Python: `PromptsResource` caches `render()` results in-process (default 60s TTL)
- TypeScript: same — `Map<string, CacheEntry>` with FNV-1a variable hash
- Configurable TTL, `clear_cache()`, `invalidate(slug)` for manual control

---

## Tier 8 — A/B Experiments

### 8.1 Experiment CRUD
- Create named experiments with N variants + weights
- Each variant can override: model, upstream URL, any body field
- Experiments map to `Action::Split` policies internally (consistent with policy engine)
- `POST /experiments`, `GET /experiments`, `GET /experiments/:id`
- `PUT /experiments/:id` — update weights mid-experiment (live traffic shift)
- `POST /experiments/:id/stop` — soft-delete underlying Split policy

### 8.2 Traffic Splitting
- Deterministic variant assignment per `request_id` (same request → same variant)
- Weights are relative (50+50, 1+1, 70+30 — all valid)
- Any number of variants supported

### 8.3 Per-Variant Analytics
- `GET /experiments/:id/results` — per-variant breakdown:
  - Total requests, average latency (ms), total cost (USD), total tokens, error rate
- Live data from audit log — no separate data pipeline

---

## Tier 9 — MCP (Model Context Protocol)

### 9.1 MCP Server Registry
- Register any MCP-compliant server with name, endpoint, optional API key
- Performs `initialize` handshake and caches tool schemas on registration
- `GET /mcp/servers`, `POST /mcp/servers`, `DELETE /mcp/servers/:id`

### 9.2 Tool Discovery & Injection
- `POST /mcp/servers/:id/refresh` — re-fetch tool schemas
- `GET /mcp/servers/:id/tools` — inspect cached tool list
- Tool injection: add `X-MCP-Servers: server1,server2` header to any proxy request
- Tools injected as `mcp__server__tool_name` in the LLM request

### 9.3 Autonomous Tool Execution
- Gateway executes tool calls returned by the LLM autonomously (up to 10 iterations)
- No agent-side implementation required — MCP calls handled server-side in the proxy pipeline

### 9.4 MCP OAuth
- OAuth 2.0 flow support for MCP servers that require auth
- `POST /mcp/servers/:id/reauth` — refresh OAuth tokens
- Token storage and automatic refresh lifecycle

### 9.5 MCP Discover
- `POST /mcp/servers/discover` — probe an endpoint for MCP compatibility before registration
- `POST /mcp/servers/test` — test connection without persisting

### 9.6 Tool-Level RBAC (Policy Integration)
- `Action::ToolScope` — allow/block specific tool names in the `tools[]` request field
- Cross-provider: detects tools from OpenAI `tools[].function.name`, Anthropic `tools[].name`, Gemini `functionDeclarations[].name`

---

## Tier 10 — Human-in-the-Loop (HITL)

### 10.1 Approval Gate
- `Action::RequireApproval` — pauses request, sends `202 Accepted` + `request_id` to agent
- Agent polls (or SDK `wait_for_approval()`) until decision is made
- Configurable timeout; expired requests automatically rejected
- `GET /approvals`, `POST /approvals/:id/decision` → `{ "decision": "approved" | "rejected" }`

### 10.2 Synchronous & Async Modes
- **Async** (default): agent receives 202, can poll or be notified
- **Sync** (SDK): `wait_for_approval=True` — SDK blocks until decision
- **Idempotency keys**: safe retries — same idempotency key returns same result

---

## Tier 11 — Multi-Tenancy & Teams

### 11.1 Projects
- Logical namespaces for tokens and policies
- GDPR right to erasure: `POST /projects/:id/purge` — permanently delete all audit logs, sessions, usage data
- `GET /projects`, `POST /projects`, `PUT /projects/:id`, `DELETE /projects/:id`

### 11.2 Teams
- Hierarchical org structure below projects
- Teams contain members with roles
- Per-team spend tracking and enforcement
- `GET /teams`, `POST /teams`, `PUT /teams/:id`, `DELETE /teams/:id`
- `GET /teams/:id/members`, `POST /teams/:id/members`, `DELETE /teams/:id/members/:user_id`
- `GET /teams/:id/spend`

### 11.3 Model Access Groups (Fine-Grained RBAC)
- Define named groups with `allowed_models` lists
- Prevents tokens/teams from accessing unauthorized models
- `GET /model-access-groups`, `POST /model-access-groups`, `PUT /model-access-groups/:id`, `DELETE /model-access-groups/:id`

### 11.4 SSO / OIDC Providers
- Register external identity providers (Okta, Auth0, Entra ID)
- JWKS endpoint auto-fetched and cached
- Claim mappings: map custom JWT claims → AILink roles and scopes
- Default role/scopes for new OIDC users
- `GET /oidc/providers`, `POST /oidc/providers`, `PUT /oidc/providers/:id`, `DELETE /oidc/providers/:id`

---

## Tier 12 — Sessions

### 12.1 Session Tracking
- Multi-turn agent workflows tracked end-to-end
- Sessions aggregated cost, tokens, request count
- Status lifecycle: `active` → `paused` → `completed`
- Session-level spend caps (blocks further requests when exceeded)
- `GET /sessions`, `GET /sessions/:id`, `PATCH /sessions/:id/status`, `PUT /sessions/:id/spend-cap`, `GET /sessions/:id/entity`

---

## Tier 13 — Service Gateway (Action Gateway)

### 13.1 Service Registry
- Register any external REST API (Stripe, GitHub, Slack, etc.) with a credential
- Proxy any HTTP method through the gateway with secure credential injection
- `GET /services`, `POST /services`, `DELETE /services/:id`
- `ANY /v1/proxy/services/:service_name/*` — catch-all service proxy route

---

## Tier 14 — Webhooks & Notifications

### 14.1 Webhooks
- Event-driven HTTP callbacks: `policy_violation`, `spend_cap_exceeded`, `rate_limit_exceeded`, `hitl_requested`, `token_created`
- Configurable timeout; optional retry on failure
- `GET /webhooks`, `POST /webhooks`, `DELETE /webhooks/:id`, `POST /webhooks/test`

### 14.2 In-App Notifications
- Notification inbox with unread count
- Mark read individually or all at once
- `GET /notifications`, `GET /notifications/unread`, `POST /notifications/:id/read`, `POST /notifications/read-all`

---

## Tier 15 — System & Configuration

### 15.1 Config-as-Code
- Export full gateway config as YAML or JSON (all policies + tokens)
- Selective export: policies only, tokens only
- Import: upserts policies, creates token stubs (no secret exposure)
- `GET /config/export`, `GET /config/export/policies`, `GET /config/export/tokens`, `POST /config/import`

### 15.2 Model Pricing
- Custom input/output cost-per-million-token overrides for accurate spend tracking
- Glob pattern matching for model names (`gpt-4o*`)
- `GET /pricing`, `PUT /pricing`, `DELETE /pricing/:id`

### 15.3 Settings
- Org-wide gateway settings
- `GET /settings`, `PUT /settings`

### 15.4 Cache Management
- Redis cache stats: hit rates, memory usage, namespace breakdown
- Emergency cache flush without gateway restart
- `GET /system/cache-stats`, `POST /system/flush-cache`

### 15.5 Health Checks
- `GET /healthz` — liveness: process alive
- `GET /readyz` — readiness: Postgres + Redis reachable
- `GET /health/upstreams` — per-upstream circuit breaker state

---

## Tier 16 — Dashboard (Next.js)

### Pages & Features

| Page | Key Features |
|------|-------------|
| **Home / Dashboard** | Real-time charts: request volume (24h), status distribution, latency percentiles, spend breakdown; budget meter; live anomaly alerts; recent audit events; top tokens by spend |
| **Virtual Keys** | List/create/revoke tokens; per-token usage charts (volume, status, latency); circuit breaker status badge and config editor |
| **Policies** | Policy list; create/edit/delete; visual rule builder with field/operator/value selectors; mode toggle (enforce/shadow); policy version history |
| **Vault** | Credential list; create credential (masked secret entry); delete; provider badges |
| **Guardrails** | Preset toggle cards (22 presets); per-preset enable/disable per token; live status indicator (SDK vs dashboard source) |
| **Prompts** | Prompt list with folder tree; create/edit prompt; version history with commit messages; label deploy UI (production/staging); playground: render with variables, live preview |
| **Experiments** | Experiment list; create with variant weight sliders; live results table with per-variant metrics; stop experiment |
| **Audit** | Log table with filtering by token, status, method; log detail drawer with full request/response; real-time SSE streaming |
| **Analytics** | Multi-chart dashboard: volume, status, latency, spend breakdown by model/token; timeseries toggle (1h/24h/7d/30d) |
| **Sessions** | Session list; detail view with run history; status lifecycle controls; spend cap configuration |
| **Approvals (HITL)** | Pending approval queue; request summary with body preview; approve/reject button |
| **Upstreams** | Upstream list; add/edit/delete; health status per upstream; weight editor for load balancing |
| **Teams** | Team list; create team; member management (add/remove with role); team spend view |
| **Model Access Groups** | Group list; create with allowed-models multi-select; assign to tokens |
| **Webhooks** | Webhook list; create with event type checkboxes; test webhook |
| **Cache** | Cache stats: hit rate, memory usage by namespace; flush button |
| **Tools (MCP)** | MCP server list; register new server; tool browser per server; refresh tools; reauth OAuth |
| **Config** | YAML/JSON export; import with diff preview |
| **Settings > Team** | Team member management, invite, role assignment |
| **Settings > OIDC** | OIDC provider registration; claim mapping editor; test SSO flow |
| **Settings > API Keys** | API key management for dashboard/CI access |
| **Billing** | Monthly usage summary: requests, tokens, spend |
| **Notifications** | In-app notification inbox with mark-read |
| **Playground** | LLM playground with model selector, all providers, streaming output, per-request guardrail toggles |

---

## Tier 17 — SDKs

### Python SDK (`ailink`)
- `AIlinkClient` — proxy usage (OpenAI/Anthropic drop-in), admin usage, async client
- `client.openai()` / `client.anthropic()` — configured SDK instances routed through gateway
- `client.trace(session_id=...)` — session correlation
- `client.with_guardrails([...])` — per-request guardrail attachment
- `client.with_upstream_key("sk-...")` — BYOK passthrough
- `client.is_healthy()` / `client.with_fallback(fallback)` — resilience patterns
- `HealthPoller` / `AsyncHealthPoller` — background health monitoring
- `PolicyBuilder` — fluent DSL for building policy rule JSON
- **Admin resources**: `credentials`, `tokens`, `policies`, `approvals`, `audit`, `sessions`, `services`, `webhooks`, `model_aliases`, `guardrails`, `analytics`, `config`, `teams`, `model_access_groups`, `prompts`, `experiments`
- `PromptsResource`: full CRUD + versioning + label deploy + render (with 60s TTL cache, `clear_cache()`, `invalidate()`)
- `ExperimentsResource`: create, list, get, results, update, stop
- Pydantic typed responses, automatic pagination (`list_all()`), rich typed exceptions
- Framework integrations: LangChain, CrewAI, LlamaIndex

### TypeScript SDK (`@ailink/sdk`)
- `AILinkClient` — proxy + admin; `client.openai()` / `client.anthropic()`
- `client.trace(...)`, `client.withGuardrails([...])`, `client.withUpstreamKey("sk-...")`
- `client.isHealthy()` / `client.withFallback(fallback)`, `HealthPoller`
- `streamSSE<T>(response)` — typed `AsyncIterable` over SSE streams
- **Admin resources**: same 18 resource groups as Python SDK
- `PromptsResource`: full CRUD + versioning + label deploy + render (with 60s TTL cache, `clearCache()`, `invalidate()`)
- `ExperimentsResource`: create, list, get, results, update, stop
- 10 typed error classes: `RateLimitError`, `PolicyDeniedError`, `ContentBlockedError`, `SpendCapError`, `AuthenticationError`, `AccessDeniedError`, `NotFoundError`, `ValidationError`, `PayloadTooLargeError`, `GatewayError`
- Zero dependencies — native `fetch`, works in Node 18+, Deno, Bun, Cloudflare Workers
- Dual ESM + CJS build, full `.d.ts` declarations

---

## Tier 18 — Deployment & Infrastructure

### 18.1 Docker Compose
- Multi-service stack: gateway, dashboard, PostgreSQL 16, Redis 7
- One-command dev start: `docker compose up -d`
- Production-ready Compose with environment variable configuration

### 18.2 Standalone Dockerfile
- `Dockerfile.standalone` — single-container build (gateway only, external DB/Redis)
- `entrypoint.sh` — handles DB migrations, then starts gateway

### 18.3 Database
- PostgreSQL 16 — all state: policies, tokens, credentials (encrypted), audit logs, sessions, prompts, experiments
- 39 SQL migrations (001–039), applied sequentially at startup
- Audit log table partitioned by month for query performance

### 18.4 Cache Layer
- Redis 7 — rate limiting (Lua scripts), response cache, latency cache, session data, spend cap counters
- Tiered cache: hot path in Redis, cold storage in Postgres

### 18.5 Security
- Constant-time admin key comparison (SHA-256 normalised, `subtle::ConstantTimeEq`)
- Insecure default key refused in non-dev environments
- API keys stored as SHA-256 hashes — original key shown exactly once on creation
- AES-256-GCM envelope encryption for credentials
- SSRF prevention (private IP blocklist enforced at proxy layer)
- ReDoS protection (regex size limits)
- Header redaction on all logged request/responses

### 18.6 Performance
- Written in Rust (Axum + Tower + Hyper + Tokio) — sub-millisecond policy evaluation overhead
- Async-first: all DB and HTTP calls non-blocking
- Zero-copy response proxying for streaming
- Connection pooling: `sqlx` for Postgres, `deadpool-redis` for Redis
