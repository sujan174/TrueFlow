# TrueFlow Dashboard: UI to Backend API Mapping

Based on the gateway's router implementation ([src/api/mod.rs](file:///Users/sujan/Developer/AILink/gateway/src/api/mod.rs)), here is the comprehensive list of all dashboard sections, the pages they contain, the content on those pages, and the specific backend APIs that power them.

All backend APIs are prefixed with `/api/v1` (the mount point for this router).

---

## 🏠 Core

### Overview Page
The "health at a glance" page for the active project.
* **Content:** Total requests, avg latency, total cost, error rate, active tokens.
* **APIs:**
  * `GET /analytics/summary` — Returns the high-level KPI cards (volume, cost, errors, avg latency).
  * `GET /analytics/timeseries` — Returns the time-series data for the main overview sparklines/charts.
  * `GET /health/upstreams` — Returns the status of connected upstream providers.

### Projects Page
Manage organization projects to isolate environments (e.g., dev, staging, prod).
* **Content:** List of projects, create new project, switch active context.
* **APIs:**
  * `GET /projects` — List all projects in the org.
  * `POST /projects` — Create a new project.
  * `PUT /projects/:id` — Update project name/settings.
  * `DELETE /projects/:id` — Delete a project.
  * `POST /projects/:id/purge` — GDPR erasure: purges all data (logs, sessions, usage) for a project.

---

## 🔑 Token Management

### Tokens Page
List and manage virtual tokens.
* **Content:** Table of tokens showing purpose (LLM/Tool/Both), upstream URL, attached policies, tag, and status (active/revoked).
* **APIs:**
  * `GET /tokens` — List all virtual tokens (paginated).
  * `POST /tokens` — Create a single virtual token.
  * `POST /tokens/bulk` — Create multiple tokens at once (for SaaS builder use cases).
  * `POST /tokens/bulk-revoke` — Revoke multiple tokens at once.

### Token Detail Page
Deep-dive configuration for a single virtual token.
* **Content:** Spend cap config, circuit breaker config, allowed models, MCP tool allow/block lists, tags, external user ID, log level overrrides.
* **APIs:**
  * `DELETE /tokens/:id` — Revoke/delete the token.
  * `GET /tokens/:id/usage` — Current usage stats (volume/spend) for this specific token.
  * `GET /tokens/:id/circuit-breaker` — Get current circuit breaker state (open/closed/half-open, error counts).
  * `PATCH /tokens/:id/circuit-breaker` — Manually trip or reset the circuit breaker.
  * `GET /tokens/:id/spend` — Get configured spend caps (daily/monthly).
  * `PUT /tokens/:id/spend` — Upsert a spend cap.
  * `DELETE /tokens/:id/spend/:period` — Delete a spend cap.

### Credentials (Vault) Page
Manage upstream provider API keys securely.
* **Content:** List of stored credentials, injection mode (bearer/header). Keys are never shown in plaintext after creation.
* **APIs:**
  * `GET /credentials` — List credentials (metadata only, no raw keys).
  * `POST /credentials` — Securely store a new API key (AES-GCM encrypted in DB).
  * `DELETE /credentials/:id` — Delete a credential.

---

## 📋 Policy & Guardrails

### Policies Page
Rule engine configuration.
* **Content:** List policies. Rule builder for setting up conditions (IP, model, token purpose) and actions (deny, redact, routing, tool scope). Toggle enforce/shadow mode.
* **APIs:**
  * `GET /policies` — List all policies.
  * `POST /policies` — Create a new policy.
  * `PUT /policies/:id` — Update an existing policy.
  * `DELETE /policies/:id` — Delete a policy.
  * `GET /policies/:id/versions` — View the historical versions/revisions of a policy.

### Guardrail Presets Page
One-click security and safety templates.
* **Content:** Bundles like "PII Redaction" or "Prompt Injection Detection" that quickly auto-generate underlying policies.
* **APIs:**
  * `GET /guardrails/presets` — List available predefined guardrail bundles.
  * `GET /guardrails/status` — See which presets are actively enabled.
  * `POST /guardrails/enable` — Enable a preset (generates/attaches policies behind the scenes).
  * `DELETE /guardrails/disable` — Disable an active preset.

---

## 🔧 MCP & Tools

### MCP Servers Page
Manage connected Model Context Protocol servers.
* **Content:** List of registered servers, connection status, tool counts, auth method (OAuth2/Bearer/None).
* **APIs:**
  * `GET /mcp/servers` — List registered MCP servers.
  * `POST /mcp/servers` — Register a new server manually (providing an endpoint and optional static key).
  * `DELETE /mcp/servers/:id` — Remove an MCP server.
  * `POST /mcp/servers/:id/refresh` — Force a refresh of the server's available tools.
  * `GET /mcp/servers/:id/tools` — View the specific tools exposed by this server.
  * `POST /mcp/servers/:id/reauth` — Force OAuth2 token refresh/re-authentication for the server.

### MCP Discovery Page
Probe URLs before adding them to check compatibility.
* **Content:** Enter a URL to run a dry-run probe, showing if it supports MCP, its auth requirements, and available tools.
* **APIs:**
  * `POST /mcp/servers/discover` — Run an OAuth2/MCP discovery probe against a URL without saving it.
  * `POST /mcp/servers/test` — Test a connection to an MCP server (validates credentials).

---

## 📊 Analytics

*Note: All analytics APIs usually accept time range parameters (start/end) and grouping options.*

### Traffic Page
* **Content:** Volume over time, status code breakdown, latency percentiles.
* **APIs:**
  * `GET /analytics/volume` — Request sequence over time.
  * `GET /analytics/status` — Ratio of 2xx, 4xx, 5xx responses.
  * `GET /analytics/latency` — p50, p90, p95, p99 latency stats.

### Cost Page
* **Content:** Total spend, burn rate over time. Breakdown by team, user, or tag.
* **APIs:**
  * `GET /analytics/spend/breakdown` — Cost broken down by selected dimension (e.g., tags, teams).
  * `GET /analytics/users` — Spend attributed to specific end-users (external `user_id`).

### Tokens Page (Analytics View)
* **Content:** Performance and usage metrics sliced specifically by virtual token.
* **APIs:**
  * `GET /analytics/tokens` — Aggregate analytics across all tokens.
  * `GET /analytics/tokens/:id/volume` — Volume for a specific token.
  * `GET /analytics/tokens/:id/status` — Success/error rate for a specific token.
  * `GET /analytics/tokens/:id/latency` — Latency for a specific token.

### Cache Page
* **Content:** Semantic cache hit rate, bandwidth saved, latency reduction.
* **APIs:**
  * `GET /system/cache-stats` — Global semantic cache performance metrics.
  * `POST /system/flush-cache` — Admin action to clear the semantic cache.

### Security Page
* **Content:** Policy violations, blocked requests, and identified anomalies.
* **APIs:**
  * `GET /anomalies` — List detected anomalous request patterns (based on guardrails/WAF).
  * *(Also uses `GET /audit` filtered by blocked/denied status).*

### Prompt Management / Evaluation Page
* **Content:** Manage prompt templates, versions, run tests/evaluations.
* **APIs:**
  * `GET /prompts`, `POST /prompts` — List and create prompt templates.
  * `GET /prompts/folders` — Organize prompts.
  * `GET /prompts/:id/versions`, `POST /prompts/:id/versions` — Version control for prompts.
  * `POST /prompts/:id/deploy` — Mark a specific version as the active deployment.
  * `GET /prompts/by-slug/:slug/render`, `POST .../render` — Render a prompt template with variables for preview.

### Experiments Page (A/B Testing)
* **Content:** Setup A/B tests to route traffic across different models/prompts, view the winning variant based on latency, cost, and errors.
* **APIs:**
  * `GET /experiments`, `POST /experiments` — List or create A/B tests (creates a Split policy variant).
  * `GET /experiments/:id`, `PUT /experiments/:id` — View details or update traffic weights for variants.
  * `GET /experiments/:id/results` — The analytics/results for the experiment variants.
  * `POST /experiments/:id/stop` — Conclude the experiment.

---

## 🛡️ Human-in-the-Loop (HITL)

### HITL Queue Page
* **Content:** Real-time queue of requests paused by `require_approval` policies. Shows the payload awaiting human review.
* **APIs:**
  * `GET /approvals` — List pending approval requests.
  * `POST /approvals/:id/decision` — Submit an "approve" or "reject" decision to release the paused request back to the client.

---

## 📝 Audit & Observability

### Audit Logs Page
* **Content:** The raw, searchable ledger of all requests. Filterable by token, purpose (LLM/Tool), project, and status.
* **APIs:**
  * `GET /audit` — Searchable, paginated list of audit records.
  * `GET /audit/:id` — Full JSON detail of a single request/response.
  * `GET /audit/stream` — SSE endpoint for real-time tailing of the audit log in the UI.

---

## 👥 Organization & Access

### Teams Page
* **Content:** Create teams, group virtual tokens under teams for aggregated budget tracking.
* **APIs:**
  * `GET /teams`, `POST /teams` — Manage teams.
  * `PUT /teams/:id`, `DELETE /teams/:id` — Edit/Delete teams.
  * `GET /teams/:id/members`, `POST /teams/:id/members`, `DELETE /teams/:id/members/:user_id` — Manage user membership in teams.
  * `GET /teams/:id/spend` — Total spend aggregated across the team.

### Access Control (RBAC & API Keys)
* **Content:** Manage dashboard access, issue API keys for CI/CD or developers. Define model access groups.
* **APIs:**
  * `GET /auth/keys`, `POST /auth/keys`, `DELETE /auth/keys/:id` — Issue and revoke API keys with specific roles (Admin/Member/ReadOnly).
  * `GET /auth/whoami` — Retrieve current logged-in user context/permissions.
  * `GET /model-access-groups`, `POST /model-access-groups`, `PUT ...`, `DELETE ...` — Manage groups defining which physical models specific teams/users are allowed to access.

### Sessions Page
* **Content:** Manage long-running agentic or chat sessions.
* **APIs:**
  * `GET /sessions`, `GET /sessions/:id` — List and view sessions.
  * `PATCH /sessions/:id/status` — Pause, resume, or terminate a session forcefully.
  * `PUT /sessions/:id/spend-cap` — Set a max budget for a single continuous session.

---

## ⚙️ Settings

### Gateway Settings
* **Content:** Global key-value configurations.
* **APIs:**
  * `GET /settings`, `PUT /settings` — Read/update global config.

### Pricing Config
* **Content:** Define custom cost calculation rules mapping model regex patterns to input/output token costs.
* **APIs:**
  * `GET /pricing` — List custom pricing rules.
  * `PUT /pricing/:id` — Upsert a pricing rule.
  * `DELETE /pricing/:id` — Remove a custom rule (falls back to defaults).

### System Data & Webhooks
* **Content:** Configure webhooks for system events, export/import state.
* **APIs:**
  * `GET /webhooks`, `POST /webhooks`, `DELETE /webhooks/:id` — Manage event webhooks.
  * `POST /webhooks/test` — Send a test payload to verify a webhook URL.
  * `GET /config/export` — Export entire setup (tokens + policies) as YAML/JSON.
  * `POST /config/import` — Import setup (Config-as-Code).
  * `GET /notifications`, `POST /notifications/:id/read` — System alerts (e.g., spend cap breached).
  * `POST /pii/rehydrate` — Admin endpoint to reveal tokenized PII securely.
