# TrueFlow — Policy Guide

> Policies define **what** an agent can do with a token.

## Structure

Policies are JSON documents containing a list of `rules`. Each rule has a `when` condition and a `then` action (or list of actions).

```json
{
  "name": "stripe-billing-worker",
  "mode": "enforce", 
  "phase": "pre",
  "rules": [
    {
      "when": { "field": "path", "op": "starts_with", "value": "/v1/charges" },
      "then": { "action": "allow" }
    }
  ]
}
```

### Top-Level Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | required | Human-readable policy name |
| `mode` | `"enforce"` \| `"shadow"` | `"enforce"` | Whether to enforce or just log violations |
| `phase` | `"pre"` \| `"post"` | `"pre"` | When to evaluate: before or after the upstream call |
| `rules` | array | required | Ordered list of condition→action rules |

### Rule Fields

Inside the `rules` array, each rule object supports:

| Field | Type | Default | Description |
|---|---|---|---|
| `comment` | string | `null` | Optional description of the rule |
| `when` | object | required | Condition that triggers the rule |
| `then` | object \| array | required | Action(s) to execute if matched |
| `async_check` | boolean | `false` | Run rule asynchronously in the background (non-blocking). Allowed for `log`, `tag`, `webhook`, `validate_schema`, `content_filter`, `external_guardrail`. |

---

## Policy Modes

### Enforce Mode (default)

Rules are evaluated and **actions are executed**. A `deny` blocks the request. A `rate_limit` rejects over-limit requests.

```json
{ "mode": "enforce" }
```

### Shadow Mode

Rules are evaluated and matches are **logged to the audit trail**, but **no actions are executed**. The request always proceeds normally. Use this to test policies before promoting them to enforce mode.

```json
{ "mode": "shadow" }
```

**Workflow:** Create a policy in shadow mode → review audit logs for false positives → promote to enforce mode with `PATCH /policies/{id}`.

```json
// Promote from shadow to enforce
PATCH /policies/{policy_id}
{ "mode": "enforce" }
```

Shadow violations appear in the audit log with `"shadow": true` so you can query for them.

---

## Policy Phases

### Pre-Flight (`"pre"`) — default

Evaluated **before** the request is forwarded to the upstream API. Use for access control, rate limiting, request redaction, and approval gates.

Available fields: `request.*`, `agent.*`, `token.*`, `context.*`, `usage.*`

### Post-Flight (`"post"`)

Evaluated **after** the upstream response is received. Use for response redaction, logging/tagging based on response status, and conditional alerts.

Available fields: everything from `pre`, plus `response.*` fields.

```json
{
  "name": "redact-pii-from-responses",
  "phase": "post",
  "rules": [
    {
      "comment": "Redact emails from all successful responses",
      "when": { "field": "response.status", "op": "lt", "value": 400 },
      "then": { "action": "redact", "direction": "response", "patterns": ["email", "ssn"] }
    }
  ]
}
```

```json
{
  "name": "alert-on-errors",
  "phase": "post",
  "rules": [
    {
      "comment": "Fire webhook on upstream 5xx errors",
      "when": { "field": "response.status", "op": "gte", "value": 500 },
      "then": { "action": "webhook", "url": "https://hooks.slack.com/...", "timeout_ms": 5000 }
    }
  ]
}
```

---

## 1. Conditions (`when`)

Conditions determine if a rule triggers.

### Direct Match

Match a specific field against a value.

```json
"when": {
  "field": "request.method",
  "op": "eq",
  "value": "POST"
}
```

### Logic Operators

Combine multiple conditions.

```json
"when": {
  "and": [
    { "field": "request.method", "op": "eq", "value": "DELETE" },
    { "field": "request.path", "op": "starts_with", "value": "/v1/customers" }
  ]
}
```

```json
"when": {
  "or": [
    { "field": "request.path", "op": "eq", "value": "/health" },
    { "field": "request.path", "op": "eq", "value": "/metrics" }
  ]
}
```

### Always True

Useful for default rules (e.g., "Always Rate Limit").

```json
"when": { "always": true }
```

### Operators (`op`)

| Operator | Description | Example |
|---|---|---|
| `eq` | Equals (with type coercion) | `"value": "POST"` |
| `neq` | Not equals | `"value": "DELETE"` |
| `gt` | Greater than (numeric) | `"value": 100` |
| `gte` | Greater than or equal | `"value": 50.00` |
| `lt` | Less than (numeric) | `"value": 400` |
| `lte` | Less than or equal | `"value": 1000` |
| `in` | Value is in array | `"value": ["GET", "HEAD"]` |
| `contains` | Substring or array membership | `"value": "admin"` |
| `starts_with` | String prefix | `"value": "/v1/"` |
| `ends_with` | String suffix | `"value": ".json"` |
| `glob` | Glob pattern (`*`, `?`) | `"value": "/v1/*/charges"` |
| `regex` | Regular expression | `"value": "^sk_(live|test)_"` |
| `exists` | Field exists (non-null) | (no `value` needed) |

---

## 2. Field Reference

All fields use dot-notation. Shorthand aliases (e.g. `method`) resolve to their full path (`request.method`).

### Request Fields (Pre + Post)

| Field | Type | Description |
|---|---|---|
| `request.method` | string | HTTP method (GET, POST, etc.) |
| `request.path` | string | URL path |
| `request.body_size` | number | Request body size in bytes |
| `request.body` | object | Full JSON body |
| `request.body.<path>` | any | Dot-notation into JSON body (e.g. `request.body.model`) |
| `request.headers.<name>` | string | Request header value |
| `request.query.<name>` | string | Query parameter value |

### Response Fields (Post-Flight Only)

| Field | Type | Description |
|---|---|---|
| `response.status` | number | HTTP status code |
| `response.body.<path>` | any | Dot-notation into response JSON |
| `response.headers.<name>` | string | Response header value |

### Identity Fields

| Field | Type | Description |
|---|---|---|
| `agent.name` | string | Agent name from `X-Agent-Name` header |
| `token.id` | string | Virtual token ID |
| `token.name` | string | Token display name |
| `token.project_id` | string | Project UUID |
| `context.ip` | string | Client IP address |

### Time Fields

| Field | Type | Description |
|---|---|---|
| `context.time.hour` | number | Current hour (0–23 UTC) |
| `context.time.weekday` | string | Day of the week (`monday`, etc.) |
| `context.time.date` | string | ISO date (`2026-02-17`) |

### Usage Counters

Usage counters are tracked in Redis and updated in real-time.

| Field | Type | Description |
|---|---|---|
| `usage.spend_today_usd` | number | Total spend today (USD) |
| `usage.spend_month_usd` | number | Total spend this month (USD) |
| `usage.requests_today` | number | Request count today |
| `usage.requests_this_hour` | number | Request count this hour |

---

## 3. Actions (`then`)

### `deny`

Block the request with a custom status code and message.

```json
{ "action": "deny", "status": 403, "message": "Access Forbidden" }
```

Defaults: `status: 403`, `message: "denied by policy"`.

### `rate_limit`

Limits requests per time window, scoped by key.

```json
{
  "action": "rate_limit",
  "window": "1m",
  "max_requests": 60,
  "key": "token"
}
```

| Param | Options | Default |
|---|---|---|
| `window` | `"1s"`, `"1m"`, `"1h"`, `"1d"` | required |
| `max_requests` | integer | required |
| `key` | `"token"`, `"agent"`, `"ip"`, `"global"` | `"token"` |

### `require_approval` (HITL)

Pauses the request until a human approves it via Dashboard or Slack.

```json
{
  "action": "require_approval",
  "timeout": "5m",
  "fallback": "deny",
  "notify": {
    "channel": "slack",
    "webhook_url": "https://hooks.slack.com/services/..."
  }
}
```

| Param | Options | Default |
|---|---|---|
| `timeout` | Duration string | `"5m"` |
| `fallback` | `"allow"` \| `"deny"` | `"deny"` |
| `notify` | NotifyConfig object | `null` |

### `redact` (PII Scrubbing)

Removes sensitive data from request or response bodies.

```json
{
  "action": "redact",
  "direction": "both",
  "patterns": ["ssn", "credit_card", "email", "api_key", "phone"],
  "fields": ["request.body.secret_key"]
}
```

| Param | Description |
|---|---|
| `direction` | `"request"`, `"response"`, or `"both"` |
| `patterns` | Built-in PII patterns or custom regex strings |
| `fields` | Specific body fields to fully redact |

**Built-in patterns:** `ssn`, `email`, `phone`, `credit_card`, `api_key`.

### `transform`

Modifies headers, JSON body fields, or injects synthetic messages and system prompts.

```json
{
  "action": "transform",
  "operations": [
    { "type": "set_header", "name": "X-Custom", "value": "Verified" },
    { "type": "remove_header", "name": "X-Internal-Debug" },
    { "type": "prepend_system_prompt", "text": "Do not hallucinate." },
    { "type": "set_body_field", "path": "temperature", "value": 0.5 },
    { "type": "remove_body_field", "path": "user.id" },
    { "type": "regex_replace", "pattern": "internal-db-\\w+", "replacement": "[REDACTED]", "global": true },
    { "type": "add_to_message_list", "role": "system", "content": "Keep it brief.", "position": "first" }
  ]
}
```

| Operation Type | Description |
|---|---|
| `set_header` | Sets an HTTP header (request or response) |
| `remove_header` | Removes an HTTP header (including standard headers like `User-Agent`) |
| `prepend_system_prompt`\|`append_system_prompt`| Injects text into the OpenAI `system` message or Anthropic `system` field. Creates one if absent. |
| `regex_replace` | Regex find/replace across all strings in the JSON body |
| `set_body_field` | Sets a JSON field by dot-path (e.g. `temperature`) |
| `remove_body_field` | Deletes a JSON field by dot-path |
| `add_to_message_list` | Injects synthetic message into the OpenAI messages array (`position`: `first`, `last`, `before_last`) |

### `override`

Override body fields before forwarding. Useful for model downgrades.

```json
{
  "action": "override",
  "set_body_fields": {
    "model": "gpt-3.5-turbo",
    "max_tokens": 1000
  }
}
```

**Example:** Force GPT-4 requests to use GPT-3.5 when daily spend exceeds $50:

```json
{
  "when": {
    "and": [
      { "field": "request.body.model", "op": "eq", "value": "gpt-4" },
      { "field": "usage.spend_today_usd", "op": "gt", "value": 50.00 }
    ]
  },
  "then": { "action": "override", "set_body_fields": { "model": "gpt-3.5-turbo" } }
}
```

### `throttle`

Artificially delay the request (useful for testing or cost-dampening).

```json
{ "action": "throttle", "delay_ms": 2000 }
```

### `log` / `tag`

Adds metadata to the audit trail without affecting the request.

```json
{ "action": "log", "level": "warn", "tags": { "risk": "high" } }
```

```json
{ "action": "tag", "key": "department", "value": "finance" }
```

### `webhook`

Fire an external webhook (e.g. Slack, PagerDuty) when a rule matches.

```json
{
  "action": "webhook",
  "url": "https://hooks.slack.com/services/...",
  "timeout_ms": 5000,
  "on_fail": "allow"
}
```

| Param | Options | Default |
|---|---|---|
| `timeout_ms` | integer (ms) | `5000` |
| `on_fail` | `"allow"`, `"deny"`, or `"log"` | `"allow"` |

### `validate_schema`

Validates that the LLM response body (or extracted JSON from markdown blocks) conforms to a declared JSON Schema (draft 2020-12). *Applied in the `"post"` phase only.*

```json
{
  "action": "validate_schema",
  "schema": {
    "type": "object",
    "required": ["answer", "confidence"],
    "properties": {
      "answer": { "type": "string" },
      "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
    }
  },
  "not": false,
  "message": "Response must include answer and confidence fields"
}
```

### `dynamic_route`

Selects an upstream based on a routing strategy. Evaluates at request time — useful for load balancing or cost-optimized routing across multiple providers.

```json
{
  "action": "dynamic_route",
  "strategy": "round_robin",
  "targets": [
    {"model": "gpt-4o", "upstream_url": "https://api.openai.com"},
    {"model": "claude-3-5-sonnet-20241022", "upstream_url": "https://api.anthropic.com"}
  ]
}
```

| Param | Options | Description |
|---|---|---|
| `strategy` | `"round_robin"`, `"lowest_cost"`, `"latency"`, `"random"` | Routing algorithm |
| `targets` | array of `{model, upstream_url}` | Available upstream targets |

### `conditional_route`

Selects an upstream target based on request properties. The first branch whose condition evaluates to true wins. Can replace `dynamic_route` when hardcoded conditional fallback paths are needed.

```json
{
  "action": "conditional_route",
  "branches": [
    {
      "condition": {"field": "body.model", "op": "eq", "value": "gpt-4o"},
      "target": {"model": "claude-3-5-sonnet-20241022", "upstream_url": "https://api.anthropic.com"}
    },
    {
      "condition": {"field": "header.x-user-tier", "op": "eq", "value": "premium"},
      "target": {"model": "gpt-4o-mini", "upstream_url": "https://api.openai.com"}
    }
  ],
  "fallback": {"model": "gpt-4o-mini", "upstream_url": "https://api.openai.com"}
}
```

### `external_guardrail`

Delegates the guardrail safety check to an external vendor API. Can be extremely fast when combined with `async_check: true`.

```json
{
  "action": "external_guardrail",
  "vendor": "azure_content_safety",
  "endpoint": "https://<your-resource>.cognitiveservices.azure.com",
  "api_key_env": "AZURE_CONTENT_SAFETY_KEY",
  "threshold": 4,
  "on_fail": "deny"
}
```

| Param | Description |
|---|---|
| `vendor` | `"azure_content_safety"`, `"aws_comprehend"`, or `"llama_guard"` |
| `endpoint` | Upstream vendor URL (for LlamaGuard, your Ollama/vLLM server) |
| `api_key_env` | Environment variable name holding the API key |
| `threshold` | Float. Threshold above which a request/response is flagged (vendor-specific) |
| `on_fail` | `"allow"`, `"deny"`, or `"log"` (default: `"deny"`) |

### `content_filter`

Built-in content filtering used by guardrail presets. Checks request/response text against regex patterns and rejects on match.

```json
{
  "action": "content_filter",
  "patterns": ["ssn", "credit_card", "api_key"],
  "direction": "request",
  "on_match": "deny",
  "message": "Content blocked by safety filter"
}
```

| Param | Description |
|---|---|
| `patterns` | Array of pattern names (built-in) or regex strings |
| `direction` | `"request"`, `"response"`, or `"both"` |
| `on_match` | `"deny"` (block with 403), `"redact"` (scrub and continue), or `"log"` (record only) |
| `message` | Custom error message when blocked |

> **Note:** This action is primarily used internally by guardrail presets (e.g., `pii_redaction`, `prompt_injection`). For most use cases, prefer the `POST /guardrails/enable` API instead of crafting `content_filter` rules manually.

---

## 4. Spend Caps

Spend caps are implemented through the policy engine using `usage.*` fields. Redis-backed counters track daily and monthly spend per token in real-time.

### Daily Spend Cap

```json
{
  "name": "daily-budget-50",
  "rules": [
    {
      "comment": "Block when daily spend exceeds $50",
      "when": { "field": "usage.spend_today_usd", "op": "gt", "value": 50.00 },
      "then": { "action": "deny", "status": 429, "message": "Daily budget exceeded ($50)" }
    }
  ]
}
```

### Monthly Spend Cap with Model Downgrade

```json
{
  "name": "cost-management",
  "rules": [
    {
      "comment": "Hard block at $500/month",
      "when": { "field": "usage.spend_month_usd", "op": "gt", "value": 500.00 },
      "then": { "action": "deny", "message": "Monthly budget exceeded" }
    },
    {
      "comment": "Downgrade GPT-4 to GPT-3.5 after $100/day",
      "when": {
        "and": [
          { "field": "usage.spend_today_usd", "op": "gt", "value": 100.00 },
          { "field": "request.body.model", "op": "eq", "value": "gpt-4" }
        ]
      },
      "then": { "action": "override", "set_body_fields": { "model": "gpt-3.5-turbo" } }
    }
  ]
}
```

### Request Volume Cap

```json
{
  "comment": "Max 1000 requests per day",
  "when": { "field": "usage.requests_today", "op": "gt", "value": 1000 },
  "then": { "action": "deny", "status": 429, "message": "Daily request limit reached" }
}
```

### Dedicated Spend Cap Enforcement

In addition to policy-based budget rules, TrueFlow provides **dedicated per-token spend caps** stored in the `spend_caps` database table. These enforce hard budget limits independently of any policy configuration.

**How it works:**
1. Per-token daily and monthly limits are stored in the `spend_caps` table
2. Redis counters (`spend:{token_id}:daily:{date}`) track real-time spend
3. `check_spend_cap()` runs after policy evaluation — if a cap is exceeded, the request is blocked with HTTP 429
4. A `spend_cap_exceeded` webhook event is automatically dispatched

```bash
# Example: Set a $50/day cap on a token
curl -X PUT http://localhost:8443/api/v1/tokens/your-token-id/spend \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"period": "daily", "limit_usd": 50.00}'

# Example: Set an additional $500/month cap
curl -X PUT http://localhost:8443/api/v1/tokens/your-token-id/spend \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"period": "monthly", "limit_usd": 500.00}'
```

### Webhook Notifications

When a policy violation, rate limit exceedance, or spend cap breach occurs, TrueFlow dispatches a webhook to all configured URLs.

**Configuration:**

```bash
TRUEFLOW_WEBHOOK_URLS=https://hooks.slack.com/services/...,https://webhook.site/your-id
```

**Event types:**

| Event Type | Trigger |
|---|---|
| `policy_violation` | Policy `deny` action fires |
| `rate_limit_exceeded` | Rate limit counter exceeded |
| `spend_cap_exceeded` | Daily or monthly spend cap hit |

**Payload example:**

```json
{
  "event_type": "spend_cap_exceeded",
  "timestamp": "2026-02-18T16:07:00Z",
  "token_id": "tok_abc123",
  "token_name": "production-agent",
  "project_id": "00000000-0000-0000-0000-000000000001",
  "details": { "reason": "daily spend cap of $50.00 exceeded" }
}
```

Webhooks are best-effort (fire-and-forget). Failures are logged but never block requests.

---

## 5. Full Examples

### Comprehensive Protection

```json
{
  "name": "full-protection",
  "rules": [
    {
      "comment": "Block high-spend agents",
      "when": { "field": "usage.spend_today_usd", "op": "gt", "value": 50.00 },
      "then": { "action": "deny", "message": "Daily budget exceeded" }
    },
    {
      "comment": "Rate limit: 100 req/hour",
      "when": { "always": true },
      "then": { "action": "rate_limit", "window": "1h", "max_requests": 100 }
    },
    {
      "comment": "Redact PII from responses",
      "when": { "always": true },
      "then": { "action": "redact", "direction": "response", "patterns": ["email", "ssn"] }
    },
    {
      "comment": "Require approval for DELETE",
      "when": { "field": "request.method", "op": "eq", "value": "DELETE" },
      "then": { "action": "require_approval" }
    }
  ]
}
```

### Time-Based Access Control

```json
{
  "name": "business-hours-only",
  "rules": [
    {
      "comment": "Block non-GET requests outside business hours (9-17 UTC)",
      "when": {
        "and": [
          { "field": "request.method", "op": "neq", "value": "GET" },
          {
            "or": [
              { "field": "context.time.hour", "op": "lt", "value": 9 },
              { "field": "context.time.hour", "op": "gte", "value": 17 }
            ]
          }
        ]
      },
      "then": { "action": "deny", "message": "Write operations restricted to business hours" }
    }
  ]
}
```

### Shadow Mode Testing

```json
{
  "name": "strict-path-test",
  "mode": "shadow",
  "rules": [
    {
      "comment": "Would block all paths except /v1/charges — shadow only",
      "when": {
        "field": "request.path",
        "op": "starts_with",
        "value": "/v1/charges"
      },
      "then": { "action": "allow" }
    },
    {
      "when": { "always": true },
      "then": { "action": "deny", "message": "Path not allowed" }
    }
  ]
}
```

Review the audit logs for shadow violations, then promote:

```bash
# Check shadow violations
curl http://localhost:8443/api/v1/audit?shadow=true

# Promote to enforce
curl -X PATCH http://localhost:8443/api/v1/policies/{id} \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -d '{"mode": "enforce"}'
```

---

## 6. Service-Scoped Policies

When using the **Service Registry** (Action Gateway), requests are proxied via `/v1/proxy/services/{service_name}/...`. Policies can target specific services using path matching.

### Restrict Access to Specific Services

```json
{
  "name": "only-stripe-and-github",
  "rules": [
    {
      "comment": "Allow Stripe API access",
      "when": { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/stripe/" },
      "then": { "action": "allow" }
    },
    {
      "comment": "Allow GitHub API access",
      "when": { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/github/" },
      "then": { "action": "allow" }
    },
    {
      "comment": "Block all other services",
      "when": { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/" },
      "then": { "action": "deny", "message": "Service not allowed for this token" }
    }
  ]
}
```

### Per-Service Rate Limits

```json
{
  "name": "service-rate-limits",
  "rules": [
    {
      "comment": "Stripe: 30 req/min (API rate limit alignment)",
      "when": { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/stripe/" },
      "then": { "action": "rate_limit", "window": "1m", "max_requests": 30 }
    },
    {
      "comment": "Slack: 10 req/min (avoid spam)",
      "when": { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/slack/" },
      "then": { "action": "rate_limit", "window": "1m", "max_requests": 10 }
    }
  ]
}
```

### HITL for Destructive Service Operations

```json
{
  "name": "approve-destructive-service-calls",
  "rules": [
    {
      "comment": "Require approval for DELETE on any service",
      "when": {
        "and": [
          { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/" },
          { "field": "request.method", "op": "eq", "value": "DELETE" }
        ]
      },
      "then": { "action": "require_approval", "timeout": "10m", "fallback": "deny" }
    }
  ]
}
```

### Multi-Service Agent with Full Protection

```json
{
  "name": "multi-service-agent-policy",
  "rules": [
    {
      "comment": "Stripe: read-only",
      "when": {
        "and": [
          { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/stripe/" },
          { "field": "request.method", "op": "neq", "value": "GET" }
        ]
      },
      "then": { "action": "deny", "message": "Stripe access is read-only" }
    },
    {
      "comment": "GitHub: redact API keys from responses",
      "when": { "field": "request.path", "op": "starts_with", "value": "/v1/proxy/services/github/" },
      "then": { "action": "redact", "direction": "response", "patterns": ["api_key"] }
    },
    {
      "comment": "Global: 500 req/day across all services",
      "when": { "field": "usage.requests_today", "op": "gt", "value": 500 },
      "then": { "action": "deny", "status": 429, "message": "Daily limit reached" }
    }
  ]
}
```
