# TrueFlow — API Reference

> This reference covers the Management API for configuring TrueFlow. For making requests *through* the gateway, see the [Python SDK](../sdks/python.md) or [TypeScript SDK](../sdks/typescript.md).

## Management API

Base URL: `http://localhost:8443/api/v1`  
Auth: `Authorization: Bearer <api_key>` (create keys via `/auth/keys`)

### Authentication & Authorization

Every Management API request requires a valid API key in the `Authorization: Bearer` header (or the `TRUEFLOW_ADMIN_KEY` env-var key via `X-Admin-Key`).

Access is controlled by **role + scopes**:

| Role | Scope Behavior | Typical Use |
|------|---------------|-------------|
| `admin` | Auto-passes all scope checks | Operators, CI/CD pipelines |
| `member` | Must have each scope explicitly | Team developers |
| `read_only` | Must have each scope explicitly | Dashboards, monitoring |

Endpoints below are annotated with: **🔒 admin** (requires admin role) and **📋 scope** (requires specific scope). Admin/SuperAdmin keys bypass all scope checks.

For full RBAC details, see [Security — RBAC](security.md#role-based-access-control-rbac).

---

### API Keys (Admin Auth)

Manage programmatic access keys for the Management API.

| Endpoint | Auth |
|----------|------|
| `GET /auth/keys` | 📋 `keys:manage` |
| `POST /auth/keys` | 🔒 admin + 📋 `keys:manage` |
| `DELETE /auth/keys/{id}` | 🔒 admin + 📋 `keys:manage` |
| `GET /auth/whoami` | any authenticated key |

#### List API Keys
`GET /auth/keys`

#### Create API Key
`POST /auth/keys`

```json
{ "name": "ci-pipeline", "role": "admin", "scopes": ["tokens:write", "policies:read"] }
```

Roles: `admin` (full access within org), `member` (read/write, no delete), `read_only`.

#### Revoke API Key
`DELETE /auth/keys/{id}`

#### Who Am I
`GET /auth/whoami` — Returns current auth context (org_id, role, scopes).

---

### Projects

Logical groups for tokens and policies.

| Endpoint | Auth |
|----------|------|
| `GET /projects` | 📋 `projects:read` |
| `POST /projects` | 📋 `projects:write` |
| `PUT /projects/{id}` | 📋 `projects:write` |
| `DELETE /projects/{id}` | 🔒 admin |
| `POST /projects/{id}/purge` | 🔒 admin |

#### List Projects
`GET /projects`

#### Create Project
`POST /projects`
```json
{ "name": "finance-team" }
```

#### Update Project
`PUT /projects/{id}`

#### Delete Project
`DELETE /projects/{id}`

#### Purge Project Data (GDPR)
`POST /projects/{id}/purge`

Permanently erases all audit logs, sessions, and usage data for a project. Irreversible. Implements GDPR Article 17 (Right to Erasure).

---

### Tokens

Virtual tokens issued to AI agents. Agents use these instead of real API keys.

| Endpoint | Auth |
|----------|------|
| `GET /tokens` | 📋 `tokens:read` |
| `POST /tokens` | 🔒 admin + 📋 `tokens:write` |
| `DELETE /tokens/{id}` | 🔒 admin + 📋 `tokens:write` |
| `GET /tokens/{id}/usage` | 📋 `tokens:read` |

#### List Tokens
`GET /tokens`

#### Get Token
`GET /tokens/{id}`

#### Create Token
`POST /tokens`

```json
{
  "name": "billing-agent-prod",
  "credential_id": "uuid",
  "upstream_url": "https://api.openai.com",
  "upstreams": [
    { "url": "https://api.primary.com", "weight": 70, "priority": 1 },
    { "url": "https://api.backup.com", "weight": 30, "priority": 1 }
  ],
  "policy_ids": ["policy-uuid-1"],
  "log_level": 0,
  "circuit_breaker": {
    "enabled": true,
    "failure_threshold": 3,
    "failure_rate_threshold": 0.5,
    "min_sample_size": 10,
    "recovery_cooldown_secs": 30,
    "half_open_max_requests": 1
  }
}
```

#### Revoke Token
`DELETE /tokens/{id}`

#### Get Token Usage
`GET /tokens/{id}/usage`

---

### Circuit Breaker

Per-token circuit breaker configuration for upstream resilience.

#### Get Circuit Breaker Config
`GET /tokens/{id}/circuit-breaker`

```json
{
  "enabled": true,
  "failure_threshold": 3,
  "failure_rate_threshold": 0.5,
  "min_sample_size": 10,
  "recovery_cooldown_secs": 30,
  "half_open_max_requests": 1
}
```

#### Update Circuit Breaker Config
`PATCH /tokens/{id}/circuit-breaker`

```json
{
  "enabled": true,
  "failure_threshold": 5,
  "failure_rate_threshold": 0.3,
  "min_sample_size": 20,
  "recovery_cooldown_secs": 60,
  "half_open_max_requests": 2
}
```

Update at runtime without gateway restart. CB states: `closed` → `open` (after N continuous failures or when failure rate > threshold) → `half_open` (cooldown elapsed) → `closed`.

> Response headers on every proxied request:
> - `X-TrueFlow-CB-State: closed | open | half_open | disabled`
> - `X-TrueFlow-Upstream: https://api.primary.com`

---

### Spend Caps

Monetary limits per token (enforced atomically via Redis Lua scripts).

> **Auth**: These endpoints currently have no scope check — any authenticated key can manage spend caps.

#### Get Spend Caps
`GET /tokens/{id}/spend`

```json
{
  "daily_limit_usd": 50.0,
  "monthly_limit_usd": 500.0,
  "current_daily_usd": 12.34,
  "current_monthly_usd": 89.01
}
```

#### Set Spend Cap
`PUT /tokens/{id}/spend`
```json
{ "period": "daily", "limit_usd": 50.00 }
```

#### Remove Spend Cap
`DELETE /tokens/{id}/spend/{period}` — `period` is `daily`, `monthly`, or `lifetime`.

---

### Policies

Traffic control rules. Bind conditions (method, path, spend, time) to actions (deny, rate_limit, redact, webhook, transform).

| Endpoint | Auth |
|----------|------|
| `GET /policies` | 📋 `policies:read` |
| `POST /policies` | 🔒 admin + 📋 `policies:write` |
| `PUT /policies/{id}` | 🔒 admin + 📋 `policies:write` |
| `DELETE /policies/{id}` | 🔒 admin + 📋 `policies:write` |
| `GET /policies/{id}/versions` | 📋 `policies:read` |

#### List Policies
`GET /policies`

#### Create Policy
`POST /policies`

```json
{
  "name": "prod-safety",
  "mode": "enforce",
  "rules": [
    {
      "when": { "field": "request.body.messages[0].content", "op": "contains", "value": "sk_live" },
      "then": { "action": "deny", "message": "Cannot forward API keys" }
    }
  ]
}
```

Modes: `enforce` (blocks/modifies), `shadow` (logs only — safe rollout).

#### Update Policy
`PUT /policies/{id}`

#### Delete Policy
`DELETE /policies/{id}`

#### List Policy Versions
`GET /policies/{id}/versions` — Full audit trail of every policy change.

---

### Credentials

Real API keys stored in the vault (AES-256-GCM envelope encrypted — never returned in plaintext).

| Endpoint | Auth |
|----------|------|
| `GET /credentials` | 📋 `credentials:read` |
| `POST /credentials` | 🔒 admin + 📋 `credentials:write` |
| `DELETE /credentials/{id}` | 🔒 admin + 📋 `credentials:write` |

#### List Credentials
`GET /credentials` — Returns metadata only (name, provider, rotation status).

#### Create Credential
`POST /credentials`
```json
{
  "name": "openai-prod",
  "provider": "openai",
  "secret": "sk_live_...",
  "injection_mode": "header",
  "injection_header": "Authorization"
}
```

| Field | Default | Description |
|---|---|---|
| `name` | required | Display name |
| `provider` | required | Provider identifier (e.g., `openai`, `anthropic`, `stripe`) |
| `secret` | required | The real API key (encrypted at rest) |
| `injection_mode` | `"header"` | How the secret is injected: `"header"` or `"query"` |
| `injection_header` | `"Authorization"` | Header name for injection (when mode is `"header"`) |

#### Delete Credential
`DELETE /credentials/{id}`

---

### Guardrail Presets

One-call safety rule bundles (PII, prompt injection, HIPAA, etc.). Backed by 100+ patterns across 22 preset categories.

| Endpoint | Auth |
|----------|------|
| `GET /guardrails/presets` | any authenticated key |
| `POST /guardrails/enable` | 🔒 admin |
| `GET /guardrails/status` | any authenticated key |
| `DELETE /guardrails/disable` | 🔒 admin |

#### List Available Presets
`GET /guardrails/presets`

#### Enable Guardrails
`POST /guardrails/enable`
```json
{
  "token_id": "tf_v1_...",
  "presets": ["pii_redaction", "prompt_injection", "hipaa"],
  "source": "dashboard",
  "topic_allowlist": ["billing"],
  "topic_denylist": ["competitors"]
}
```

#### Check Guardrail Status
`GET /guardrails/status?token_id={id}`

Returns active presets and source (sdk/dashboard) for drift detection.

#### Disable Guardrails
`DELETE /guardrails/disable` (body: `{"token_id": "..."}`)

---

### Per-Request Guardrail Headers

Control guardrails on a per-request basis using HTTP headers:

#### X-TrueFlow-Guardrails-Enable

Add specific guardrails to the request:

```http
X-TrueFlow-Guardrails-Enable: pii_redaction,jailbreak_protection
```

Supported presets: `pii_redaction`, `pii_block`, `prompt_injection`, `code_injection`, `hipaa`, `pci`, `pii_enterprise`

#### X-TrueFlow-Guardrails-Disable

Remove specific guardrails from the request:

```http
X-TrueFlow-Guardrails-Disable: pii_redaction
```

**Note:** These headers require the token's `guardrail_header_mode` to be set to `"append"` or `"override"`. Default is `"disabled"` for security.

---

### Prompt Management

CRUD for reusable prompt templates with immutable versioning, label-based deployment (`production` / `staging`), folder organisation, and server-side `{{variable}}` rendering.

> **Auth**: All prompt endpoints require only a valid authenticated API key (any role, no specific scope). Prompts are operational resources accessible to all team members.

#### List Prompts
`GET /prompts?folder=/production` — Filter by folder path (optional).

#### Create Prompt
`POST /prompts`
```json
{ "name": "Customer Support Agent", "folder": "/support", "description": "..." }
```

#### Get Prompt
`GET /prompts/{id}` — Returns prompt with its latest version.

#### Update Prompt Metadata
`PUT /prompts/{id}`

#### Delete Prompt
`DELETE /prompts/{id}` — Soft-delete.

#### List Versions
`GET /prompts/{id}/versions`

#### Publish New Version
`POST /prompts/{id}/versions`
```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You help {{user_name}} with {{topic}}." },
    { "role": "user", "content": "{{question}}" }
  ],
  "temperature": 0.7,
  "commit_message": "Improved tone"
}
```

#### Get Specific Version
`GET /prompts/{id}/versions/{version}`

#### Deploy Version to Label
`POST /prompts/{id}/deploy`
```json
{ "version": 2, "label": "production" }
```
Atomically promotes a version. The previous holder of the label is demoted. Use this for zero-downtime prompt rollouts.

#### Render Prompt (GET — query params)
`GET /prompts/by-slug/{slug}/render?label=production&user_name=Alice&topic=billing`

#### Render Prompt (POST — body variables)
`POST /prompts/by-slug/{slug}/render`
```json
{ "label": "production", "variables": { "user_name": "Alice", "topic": "billing", "question": "Where is my invoice?" } }
```
Returns an OpenAI-compatible payload ready to pass to any chat completions endpoint:
```json
{ "model": "gpt-4o", "messages": [...], "temperature": 0.7, "prompt_id": "uuid", "prompt_slug": "customer-support-agent", "version": 2 }
```

#### List Folders
`GET /prompts/folders` — Unique folder paths across all prompts.

---

### MCP Server Management

Register Model Context Protocol servers. The gateway auto-discovers tools and injects them into LLM requests via the `X-MCP-Servers` header.

| Endpoint | Auth |
|----------|------|
| `GET /mcp/servers` | 📋 `mcp:read` |
| `POST /mcp/servers` | 🔒 admin + 📋 `mcp:write` |
| `DELETE /mcp/servers/{id}` | 🔒 admin + 📋 `mcp:write` |
| `POST /mcp/servers/test` | 🔒 admin + 📋 `mcp:write` |
| `POST /mcp/servers/discover` | 🔒 admin + 📋 `mcp:read` |
| `POST /mcp/servers/{id}/refresh` | 🔒 admin + 📋 `mcp:write` |
| `POST /mcp/servers/{id}/reauth` | 🔒 admin + 📋 `mcp:write` |
| `GET /mcp/servers/{id}/tools` | 📋 `mcp:read` |

#### List MCP Servers
`GET /mcp/servers`

#### Register MCP Server
`POST /mcp/servers`
```json
{ "name": "brave", "endpoint": "http://localhost:3001/mcp", "api_key": "optional" }
```

Performs the MCP `initialize` handshake and caches tool schemas. Server names must be alphanumeric (hyphens/underscores allowed).

#### Delete MCP Server
`DELETE /mcp/servers/{id}`

#### Test Connection (without registering)
`POST /mcp/servers/test`

#### Refresh Tool Cache
`POST /mcp/servers/{id}/refresh`

#### Re-authenticate MCP Server (OAuth)
`POST /mcp/servers/{id}/reauth` — Re-initiates OAuth 2.0 token exchange for a registered MCP server.

#### List Cached Tools
`GET /mcp/servers/{id}/tools`

**Usage**: Add `X-MCP-Servers: brave,slack` header to any proxy request. Tools are injected as `mcp__brave__search`, `mcp__slack__send_message`, etc. The gateway executes tool calls autonomously (up to 10 iterations).

---

### Human-in-the-Loop (HITL)

High-stakes operations that pause for manual review.

| Endpoint | Auth |
|----------|------|
| `GET /approvals` | 📋 `approvals:read` |
| `POST /approvals/{id}/decision` | 📋 `approvals:write` |

#### List Pending Approvals
`GET /approvals`

> **Note**: Approval requests are rate-limited. If a token exceeds `HITL_MAX_PENDING_PER_TOKEN` (default: 10) pending requests, the proxy will return `403 Forbidden` (`code: hitl_concurrency_cap_exceeded`) to prevent resource exhaustion.

#### Decide Approval
`POST /approvals/{id}/decision`
```json
{ "decision": "approved" }
```
Values: `approved` (resumes request), `rejected` (agent receives 403).

---

### Sessions

Tracked multi-turn interactions across the gateway.

| Endpoint | Auth |
|----------|------|
| `GET /sessions` | 📋 `audit:read` |
| `GET /sessions/{id}` | 📋 `audit:read` |
| `PATCH /sessions/{id}/status` | 🔒 admin + 📋 `sessions:write` |
| `PUT /sessions/{id}/spend-cap` | 🔒 admin + 📋 `sessions:write` |
| `GET /sessions/{id}/entity` | 📋 `audit:read` |

#### List Sessions
`GET /sessions?limit=50&offset=0`

#### Get Session Details
`GET /sessions/{id}`

#### Update Session Status
`PATCH /sessions/{id}/status`
```json
{ "status": "paused" }
```
Values: `active`, `paused`, `completed`.

#### Set Session Spend Cap
`PUT /sessions/{id}/spend-cap`

#### Get Session Entity
`GET /sessions/{id}/entity` — Returns real-time cost, token totals, and cap status.

---

### Audit Logs

Immutable request audit trail. Partitioned by month in PostgreSQL.

| Endpoint | Auth |
|----------|------|
| `GET /audit` | 📋 `audit:read` |
| `GET /audit/{id}` | 📋 `audit:read` |
| `GET /audit/stream` | 📋 `audit:read` |

#### Query Audit Logs
`GET /audit?limit=50&offset=0&token_id={id}`

#### Get Audit Log Detail
`GET /audit/{id}` — Full request/response bodies (if captured at log level ≥ 1).

#### Stream Audit Logs (SSE)
`GET /audit/stream` — Server-sent events for real-time log streaming to the dashboard.

---

### Analytics

| Endpoint | Auth |
|----------|------|
| `GET /analytics/volume` | any authenticated key |
| `GET /analytics/status` | any authenticated key |
| `GET /analytics/latency` | any authenticated key |
| `GET /analytics/summary` | 📋 `analytics:read` |
| `GET /analytics/timeseries` | 📋 `analytics:read` |
| `GET /analytics/experiments` | 📋 `analytics:read` |
| `GET /analytics/tokens` | 📋 `analytics:read` |
| `GET /analytics/tokens/{id}/*` | 📋 `analytics:read` |
| `GET /analytics/spend/breakdown` | 📋 `analytics:read` |

#### Request Volume
`GET /analytics/volume` — Hourly request counts (last 24h).

#### Status Distribution
`GET /analytics/status` — Count by HTTP status class (2xx, 4xx, 5xx).

#### Latency Percentiles
`GET /analytics/latency` — P50, P90, P99, mean (ms).

#### Analytics Summary
`GET /analytics/summary` — Aggregated: total requests, errors, cost, tokens.

#### Analytics Timeseries
`GET /analytics/timeseries` — Per-bucket: request count, error count, cost, latency, tokens.

#### Experiments Analytics
`GET /analytics/experiments` — Per-variant A/B experiment metrics (requests, latency, cost, tokens, error rate). For managing experiments themselves, see the [Experiments API](#experiments) below.

#### Token Analytics
`GET /analytics/tokens` — Per-token request volume and error rates.

#### Token Volume
`GET /analytics/tokens/{id}/volume`

#### Token Status
`GET /analytics/tokens/{id}/status`

#### Token Latency
`GET /analytics/tokens/{id}/latency`

#### Spend Breakdown
`GET /analytics/spend/breakdown` — Cost by model, token, or project.

---

### Teams

Organizational hierarchy for multi-team deployments.

| Endpoint | Auth |
|----------|------|
| `GET /teams` | 📋 `tokens:read` |
| `POST /teams` | 📋 `tokens:write` |
| `PUT /teams/{id}` | 📋 `tokens:write` |
| `DELETE /teams/{id}` | 📋 `tokens:write` |
| `GET /teams/{id}/members` | 📋 `tokens:read` |
| `POST /teams/{id}/members` | 📋 `tokens:write` |
| `DELETE /teams/{id}/members/{user_id}` | 📋 `tokens:write` |

> **Note**: Team endpoints currently use `tokens:read/write` scopes rather than dedicated `teams:*` scopes.

#### List Teams
`GET /teams`

#### Create Team
`POST /teams`
```json
{ "name": "platform-team" }
```

#### Update Team
`PUT /teams/{id}`

#### Delete Team
`DELETE /teams/{id}`

#### List Team Members
`GET /teams/{id}/members`

#### Add Team Member
`POST /teams/{id}/members`
```json
{ "user_id": "uuid", "role": "member" }
```

#### Remove Team Member
`DELETE /teams/{id}/members/{user_id}`

#### Team Spend
`GET /teams/{id}/spend` — Aggregate cost for all tokens belonging to the team.

---

### Model Access Groups

Fine-grained RBAC — restrict which models a token or team can access.

| Endpoint | Auth |
|----------|------|
| `GET /model-access-groups` | 📋 `tokens:read` |
| `POST /model-access-groups` | 📋 `tokens:write` |
| `PUT /model-access-groups/{id}` | 📋 `tokens:write` |
| `DELETE /model-access-groups/{id}` | 📋 `tokens:write` |

> **Note**: Model Access Group endpoints currently use `tokens:read/write` scopes rather than dedicated `model-access:*` scopes.

#### List Groups
`GET /model-access-groups`

#### Create Group
`POST /model-access-groups`
```json
{ "name": "gpt4-only", "allowed_models": ["gpt-4o", "gpt-4o-mini"] }
```

#### Update Group
`PUT /model-access-groups/{id}`

#### Delete Group
`DELETE /model-access-groups/{id}`

---

### Services (Action Gateway)

Register external APIs for secure, credential-injected proxying.

| Endpoint | Auth |
|----------|------|
| `GET /services` | 📋 `services:read` |
| `POST /services` | 🔒 admin + 📋 `services:write` |
| `DELETE /services/{id}` | 🔒 admin + 📋 `services:write` |

#### List Services
`GET /services`

#### Create Service
`POST /services`
```json
{
  "name": "stripe",
  "base_url": "https://api.stripe.com",
  "service_type": "generic",
  "credential_id": "uuid"
}
```

#### Delete Service
`DELETE /services/{id}`

#### Proxy Through a Service
`ANY /v1/proxy/services/{service_name}/*`

---

### Proxy API (For Agents)

The Proxy API is how AI agents make requests *through* TrueFlow to upstream LLM providers or registered services.

| Endpoint | Description |
|----------|-------------|
| `ANY /v1/*` | Routes to LLM provider chat completions, embeddings, etc. |
| `ANY /v1/proxy/services/{service_name}/*` | Routes to registered external services |

**Request Headers (Accepted)**

| Header | Description |
|--------|-------------|
| `Authorization` | `Bearer tf_v1_...` (Virtual Token) |
| `X-Agent-Name` | Optional context for audit logging (e.g., `fraud-bot`) |
| `X-MCP-Servers` | Comma-separated list of registered MCP servers to auto-inject tools |
| `x-trueflow-no-cache` | Set to `true` to bypass response caching. *Requires the token to have the `cache:bypass` scope.* |
| `Idempotency-Key` | UUID to prevent duplicate operations (useful for async HITL) |

**Response Headers (Returned by TrueFlow)**

| Header | Description |
|--------|-------------|
| `X-TrueFlow-Request-Id` | Unique UUID for the gateway transaction, used for tracing |
| `X-TrueFlow-CB-State` | `closed`, `open`, `half_open`, or `disabled` |
| `X-TrueFlow-Upstream` | The URL of the upstream provider that serviced the request |
| `X-TrueFlow-Cache` | `HIT` or `MISS` |

---

### Webhooks

Event-driven notifications for automated workflows.

| Endpoint | Auth |
|----------|------|
| `GET /webhooks` | 📋 `webhooks:read` |
| `POST /webhooks` | 🔒 admin + 📋 `webhooks:write` |
| `DELETE /webhooks/{id}` | 🔒 admin + 📋 `webhooks:write` |
| `POST /webhooks/test` | 🔒 admin + 📋 `webhooks:write` |

#### List Webhooks
`GET /webhooks`

#### Create Webhook
`POST /webhooks`
```json
{ "url": "https://example.com/hook", "events": ["policy_violation", "spend_cap_exceeded"] }
```
Events: `policy_violation`, `spend_cap_exceeded`, `rate_limit_exceeded`, `hitl_requested`, `token_created`.

#### Delete Webhook
`DELETE /webhooks/{id}`

#### Test Webhook
`POST /webhooks/test`
```json
{ "url": "https://example.com/hook" }
```

---

### Model Pricing

Custom cost-per-token overrides for accurate spend tracking.

| Endpoint | Auth |
|----------|------|
| `GET /pricing` | 📋 `pricing:read` |
| `PUT /pricing` | 🔒 admin + 📋 `pricing:write` |
| `DELETE /pricing/{id}` | 🔒 admin + 📋 `pricing:write` |

#### List Pricing
`GET /pricing`

#### Upsert Pricing
`PUT /pricing`
```json
{ "provider": "openai", "model_pattern": "gpt-4o*", "input_per_m": 2.50, "output_per_m": 10.00 }
```
`model_pattern` supports glob matching.

#### Delete Pricing
`DELETE /pricing/{id}`

---

### Notifications

In-app notifications for alerts and events.

| Endpoint | Auth |
|----------|------|
| `GET /notifications` | 📋 `notifications:read` |
| `GET /notifications/unread` | 📋 `notifications:read` |
| `POST /notifications/{id}/read` | 📋 `notifications:write` |
| `POST /notifications/read-all` | 📋 `notifications:write` |

#### List Notifications
`GET /notifications`

#### Count Unread
`GET /notifications/unread`

#### Mark Read
`POST /notifications/{id}/read`

#### Mark All Read
`POST /notifications/read-all`

---

### Billing

Organization-level usage and cost tracking.

| Endpoint | Auth |
|----------|------|
| `GET /billing/usage` | 📋 `billing:read` |

#### Get Usage
`GET /billing/usage?period=2026-02` — Returns total requests, tokens used, and spend for the given month.

---

### Anomaly Detection

Automatic traffic anomaly detection using sigma-based statistical analysis.

| Endpoint | Auth |
|----------|------|
| `GET /anomalies` | 🔒 admin |

#### Get Anomaly Events
`GET /anomalies`

Returns tokens with anomalous request velocity compared to their baseline. Flags sudden spikes > N standard deviations.

---

### Experiments (A/B Testing)

Create and monitor A/B experiments to compare models, prompts, or routing strategies. Experiments are a convenience layer over the policy engine's `Action::Split` — creating one auto-generates a weighted Split policy.

> **Auth**: All experiment endpoints require only a valid authenticated API key (any role, no specific scope).

#### Create Experiment
`POST /experiments`
```json
{
  "name": "gpt4o-vs-claude",
  "variants": [
    { "name": "control",   "weight": 50, "model": "gpt-4o" },
    { "name": "treatment", "weight": 50, "model": "claude-3-5-sonnet-20241022" }
  ]
}
```
Variant selection is deterministic per `request_id` — the same caller always gets the same variant within a request. Weights do not need to sum to 100 (e.g. 1+1 = 50/50).

#### List Experiments
`GET /experiments` — All running experiments.

#### Get Experiment
`GET /experiments/{id}` — Returns experiment config + live analytics.

#### Get Results
`GET /experiments/{id}/results`
```json
{
  "experiment": "gpt4o-vs-claude",
  "status": "running",
  "variants": [
    { "variant": "control",   "total_requests": 1240, "avg_latency_ms": 342, "total_cost_usd": 1.23, "error_rate": 0.01 },
    { "variant": "treatment", "total_requests": 1238, "avg_latency_ms": 289, "total_cost_usd": 0.87, "error_rate": 0.00 }
  ]
}
```

#### Update Weights
`PUT /experiments/{id}` — Adjust variant weights mid-experiment without stopping.

#### Stop Experiment
`POST /experiments/{id}/stop` — Soft-deletes the underlying Split policy.

---

### Settings

| Endpoint | Auth |
|----------|------|
| `GET /settings` | 🔒 admin |
| `PUT /settings` | 🔒 admin |

#### Get Settings
`GET /settings`

#### Update Settings
`PUT /settings`

---

### Config-as-Code

Export/import your full gateway configuration as version-controlled YAML or JSON.

> **Auth**: All config export/import endpoints require only a valid authenticated API key (any role, no specific scope).

#### Export Full Config
`GET /config/export` (YAML default, `?format=json` for JSON)

#### Export Policies Only
`GET /config/export/policies`

#### Export Tokens Only
`GET /config/export/tokens`

#### Import Config
`POST /config/import` — Upserts policies and creates token stubs.

---

### System

| Endpoint | Auth |
|----------|------|
| `GET /system/cache-stats` | 🔒 admin |
| `POST /system/flush-cache` | 🔒 admin |
| `POST /pii/rehydrate` | 🔒 admin + 📋 `pii:rehydrate` |

#### Get Cache Statistics
`GET /system/cache-stats` — Redis hit rates, memory usage, namespace breakdown.

#### Flush Cache
`POST /system/flush-cache` — Clears all cached token/policy mappings (use with caution).

#### PII Vault Rehydration
`POST /pii/rehydrate` — Decrypt tokenized PII references (requires `pii:rehydrate` scope).

---

### Health

#### Liveness
`GET /healthz` — 200 OK if process is running.

#### Readiness
`GET /readyz` — 200 OK if Postgres and Redis are reachable.

#### Upstream Health
`GET /health/upstreams` — Circuit breaker health for all tracked upstreams.

```json
[
  {
    "token_id": "tf_v1_proj_abc_tok_xyz",
    "url": "https://api.openai.com",
    "is_healthy": true,
    "failure_count": 0,
    "cooldown_remaining_secs": null
  }
]
```

---

### Prometheus Metrics

#### Scrape Metrics
`GET /metrics` — Prometheus-compatible text exposition format. No authentication required.

Exposes:
- `trueflow_requests_total` — Counter by method, status, token
- `trueflow_request_duration_seconds` — Histogram of proxy latency
- `trueflow_upstream_errors_total` — Counter by upstream URL and error type
- `trueflow_active_tokens` — Gauge of active tokens
- `trueflow_cache_hits_total` / `trueflow_cache_misses_total` — Response cache counters

---

### SSO / OIDC

TrueFlow supports OIDC-based SSO authentication. Identity providers are configured at the database level and validated via JWKS-based JWT verification during request authentication.

> **Note**: There is no Management API for OIDC provider CRUD. Providers are registered directly in the `oidc_providers` database table. JWT tokens from registered providers are validated automatically against the provider's OIDC discovery document.
