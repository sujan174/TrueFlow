# TrueFlow Gateway - Comprehensive Feature Documentation

## Document Purpose

This document provides a complete inventory of all implemented features in the TrueFlow AI Gateway, including implementation scope, degree of completion, and key technical details.

---

## Executive Summary

TrueFlow is an enterprise-grade AI agent gateway built with Rust (Axum), providing:
- **Security**: PII redaction, content filtering, guardrails, audit logging
- **Policy Engine**: 20+ action types with AND/OR/NOT condition logic
- **Multi-Provider Support**: 10 LLM providers with automatic format translation
- **Observability**: OpenTelemetry, Prometheus, Langfuse, DataDog integrations
- **Cost Management**: Spend caps, budget alerts, pricing cache

---

## 1. Core Proxy & Request Handling

### 1.1 Request Flow Architecture

**Implementation Status**: ✅ Fully Implemented

```
Agent Request → Axum Router
  → Request ID Middleware
  → Security Headers Middleware
  → CORS Layer
  → Body Limit (25MB)
  → Proxy Handler (core.rs)
      → Token Resolution (tf_v1_* → credential lookup)
      → Policy Engine Evaluation (pre-flight)
      → Action Execution (redact, filter, route, etc.)
      → Spend Cap Check (Redis-backed)
      → Rate Limiting (Redis sliding window)
      → Load Balancer Selection
      → Model Router (provider detection, format translation)
      → Upstream Request (retries, circuit breaker)
      → SSE Streaming Passthrough
      → Policy Engine Evaluation (post-flight)
      → Cost Calculation & Audit Logging
```

**Key Files**:
- `gateway/src/main.rs` - Entry point, AppState, middleware stack
- `gateway/src/proxy/handler/core.rs` - Main orchestrator (500+ lines)
- `gateway/src/proxy/upstream.rs` - HTTP client with connection pooling

**Degree of Implementation**: Complete production-ready implementation with:
- Request ID injection via middleware
- W3C trace context support (`traceparent` header)
- Attribution headers (x-user-id, x-tenant-id, x-session-id)
- Custom properties (X-Properties JSON header)
- Test hooks (compile-time gated for integration testing)

---

## 2. Token Management

### 2.1 Virtual Tokens

**Implementation Status**: ✅ Fully Implemented

Virtual tokens (`tf_v1_*`) replace real API keys for agent authentication.

**Features**:
| Feature | Status | Details |
|---------|--------|---------|
| Token Creation | ✅ Complete | CLI + API endpoints |
| Token Listing | ✅ Complete | Paginated with filters |
| Token Revocation | ✅ Complete | Soft delete with audit trail |
| Token Expiration | ✅ Complete | Configurable expiry dates |
| Policy Binding | ✅ Complete | Multiple policies per token |
| Credential Mapping | ✅ Complete | Token → Credential resolution |
| Circuit Breaker Config | ✅ Complete | Per-token CB settings |
| Allowed Models | ✅ Complete | Model whitelist enforcement |
| MCP Tool Restrictions | ✅ Complete | allowed/blocked tool lists |
| Guardrail Header Mode | ✅ Complete | disabled/append/override modes |

**Database Schema** (from migrations):
- `tokens` table with 40+ columns
- Policy binding via `token_policy_bindings`
- Spend caps via `token_spend_caps`

---

## 3. Credential Management & Vault

### 3.1 Encrypted Credential Storage

**Implementation Status**: ✅ Fully Implemented

**Vault Implementation** (`gateway/src/vault/builtin.rs`):
- AES-256-GCM envelope encryption
- Per-credential Data Encryption Keys (DEK)
- Master Key Encryption Key (KEK) from environment
- Zeroization of plaintext DEKs after use
- Nonce: 12-byte random per encryption

**Credential Injection Modes**:
| Mode | Status | Use Case |
|------|--------|----------|
| Bearer | ✅ | OpenAI-style Authorization header |
| Basic | ✅ | HTTP Basic Auth |
| Header | ✅ | Custom header injection |
| Query | ✅ | URL query parameter |

**Security Fixes Applied**:
- CRIT-4: Zeroize KEK on drop
- H1: Block dangerous headers (host, content-length, etc.)

---

## 4. Policy Engine

### 4.1 Condition Evaluation

**Implementation Status**: ✅ Fully Implemented

**Condition Types**:
| Type | Description | Example |
|------|-------------|---------|
| `Always` | Catch-all | `{"always": true}` |
| `Check` | Field comparison | `{"field": "body.model", "op": "eq", "value": "gpt-4"}` |
| `All` | AND logic | `{"all": [condition1, condition2]}` |
| `Any` | OR logic | `{"any": [condition1, condition2]}` |
| `Not` | Negation | `{"not": condition}` |

**Operators** (12 total):
`Eq`, `Neq`, `Gt`, `Gte`, `Lt`, `Lte`, `In`, `Glob`, `Regex`, `Contains`, `StartsWith`, `EndsWith`, `Exists`

**Field Paths Supported**:
- `request.method`, `request.path`, `request.headers.*`, `request.body.*`
- `response.status`, `response.body.*`
- `token.id`, `token.name`
- `context.client_ip`
- `usage.spend_today_usd`, `usage.requests_today`

**Security Features**:
- MED-5: Empty conditions treated as denial
- ReDoS protection (1MB regex size limit)
- Glob DoS protection (100K iteration limit)
- MAX_RECURSION_DEPTH = 100

### 4.2 Action Types

**Implementation Status**: ✅ 20+ Actions Implemented

| Action | Status | Description |
|--------|--------|-------------|
| `Allow` | ✅ | Explicit allow (no-op) |
| `Deny` | ✅ | Block with custom status/message |
| `RateLimit` | ✅ | Sliding window rate limiting |
| `Throttle` | ✅ | Artificial delay |
| `Redact` | ✅ | PII detection and masking |
| `Transform` | ✅ | Header/system prompt injection |
| `Override` | ✅ | Force body field values |
| `Log` | ✅ | Custom log message |
| `Tag` | ✅ | Add audit log metadata |
| `Webhook` | ✅ | Fire external webhook |
| `ContentFilter` | ✅ | 14 check categories |
| `RequireApproval` | ✅ | Human-in-the-loop |
| `Split` | ✅ | A/B traffic splitting |
| `DynamicRoute` | ✅ | Smart model selection |
| `ConditionalRoute` | ✅ | Condition-based routing |
| `ValidateSchema` | ✅ | JSON Schema validation |
| `ExternalGuardrail` | ✅ | Azure/AWS/LlamaGuard |
| `ToolScope` | ✅ | MCP tool filtering |
| `BudgetAlert` | ✅ | Spend threshold alerts |

**Evaluation Features**:
- HIGH-4: Deny short-circuit (stops processing on deny)
- Shadow mode (log violations without blocking)
- Phase-based evaluation (Pre/Post)
- Async check support (non-blocking evaluation)

---

## 5. Security Features

### 5.1 PII Detection & Redaction

**Implementation Status**: ✅ Fully Implemented

**Built-in Patterns** (`gateway/src/middleware/redact.rs`):
| Pattern | Status | Details |
|---------|--------|---------|
| SSN | ✅ | With MED-14 fix for dashed-only |
| Email | ✅ | RFC 5322 compliant |
| Credit Card | ✅ | With Luhn validation (MED-15) |
| Phone | ✅ | International formats |
| API Key | ✅ | Common key prefixes |
| IBAN | ✅ | International bank accounts |
| DOB | ✅ | Date of birth formats |
| IPv4 | ✅ | IP address detection |

**NLP Backend Integration**:
- Presidio support for unstructured PII (names, addresses, multilingual)

**Streaming Support**:
- SSE chunk processing
- UTF-8 boundary handling
- Known limitation: Split-across-chunk PII not detected

### 5.2 Content Filtering

**Implementation Status**: ✅ Fully Implemented

**14 Check Categories** (`gateway/src/middleware/guardrail/`):
1. Jailbreak detection (DAN, prompt injection)
2. Harmful content (CSAM, suicide, violence)
3. Code injection (SQL, shell, XSS)
4. Profanity and slurs
5. Bias and discrimination
6. Sensitive topics (medical/legal advice)
7. Gibberish and encoding smuggling
8. Contact information
9. IP leakage
10. Competitor mentions
11. Topic allowlist
12. Topic denylist
13. Custom patterns
14. Content length

**Pattern Library**: 100+ compiled regex patterns

### 5.3 External Guardrails

**Implementation Status**: ✅ Fully Implemented

| Vendor | Status | Integration |
|--------|--------|-------------|
| Azure Content Safety | ✅ | Full API integration |
| AWS Comprehend | ✅ | Full API integration |
| LlamaGuard | ✅ | Ollama/vLLM host |
| Palo Alto AIRS | ✅ | Enterprise integration |
| Prompt Security | ✅ | Enterprise integration |

**Security Features**:
- SSRF protection
- Threshold validation (MED-13)
- Configurable timeouts
- Fail-open/fail-closed modes

### 5.4 Authentication & Authorization

**Implementation Status**: ✅ Fully Implemented

**Auth Methods**:
| Method | Status | Details |
|--------|--------|---------|
| SuperAdmin (Env Key) | ✅ | X-Admin-Key header |
| API Keys (ak_*) | ✅ | SHA-256 hashed in DB |
| OIDC JWT | ✅ | JWKS crypto verification |

**Role-Based Access Control**:
| Role | Permissions |
|------|-------------|
| SuperAdmin | Full access (env key only) |
| Admin | Full access within org |
| Member | Read/write, no delete |
| ReadOnly | Read-only access |

**Security Fixes**:
- SEC-01: OIDC cannot grant SuperAdmin
- SEC-06: Reject empty admin keys
- SEC-07: Constant-time key comparison
- SEC-08: Block insecure default key
- SEC-14: Wildcard scope support
- SEC-15: Unknown role handling
- MED-4: Weak entropy warning

---

## 6. Provider Integration

### 6.1 Supported Providers

**Implementation Status**: ✅ 10 Providers Supported

| Provider | Detection | Format Translation | Streaming |
|----------|-----------|-------------------|-----------|
| OpenAI | `gpt-*` | Native | ✅ |
| Anthropic | `claude-*` | OpenAI ↔ Anthropic | ✅ |
| Google Gemini | `gemini-*` | OpenAI ↔ Gemini | ✅ |
| Azure OpenAI | `azure-*` | Native | ✅ |
| AWS Bedrock | `bedrock-*` | OpenAI ↔ Bedrock | ✅ |
| Cohere | `command-*` | Native | ✅ |
| Mistral | `mistral-*` | Native | ✅ |
| Groq | `groq-*` | Native | ✅ |
| Together AI | `together-*` | Native | ✅ |
| Ollama | `ollama-*` | Native | ✅ |

**Key Files**:
- `gateway/src/proxy/model_router/mod.rs` - Provider detection
- `gateway/src/proxy/model_router/request.rs` - Request translation
- `gateway/src/proxy/model_router/response.rs` - Response translation
- `gateway/src/proxy/stream_bridge.rs` - Streaming translation

---

## 7. Load Balancing & Routing

### 7.1 Load Balancer

**Implementation Status**: ✅ Fully Implemented

**Strategies**:
1. **Weighted Round-Robin** - Within priority tiers
2. **Least Busy** - Track in-flight requests
3. **Lowest Latency** - p50 latency cache (5min refresh)
4. **Weighted Random** - Thundering herd prevention
5. **Health-Aware** - Skip unhealthy upstreams

### 7.2 Circuit Breaker

**Implementation Status**: ✅ Fully Implemented

**Configuration**:
```json
{
  "enabled": true,
  "failure_threshold": 3,
  "recovery_cooldown_secs": 30,
  "half_open_max_requests": 1,
  "failure_rate_threshold": 0.5,
  "min_sample_size": 10
}
```

**States**: Closed → Open → Half-Open → Closed/Open

**Distributed Support**: Redis-backed failure counters

**Known Limitation** (HIGH-9): Half-open probe limit is instance-local

### 7.3 Dynamic Routing

**Implementation Status**: ✅ Fully Implemented

**Strategies**:
- `lowest_cost` - Minimize spend
- `lowest_latency` - Minimize response time
- `round_robin` - Simple rotation
- `least_busy` - Fewest in-flight requests
- `weighted_random` - Random with weights

---

## 8. Rate Limiting & Spend Management

### 8.1 Rate Limiting

**Implementation Status**: ✅ Fully Implemented

**Implementation**:
- Redis-backed sliding window (no 2x burst vulnerability)
- Per-token, per-project, global scopes
- Configurable windows (second, minute, hour, day)

### 8.2 Spend Caps

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Daily/Monthly/Lifetime caps
- Per-token enforcement
- Single-flight pattern (MED-8) for cache misses
- Budget checker job (15min intervals)
- Webhook notifications

---

## 9. Caching Layer

### 9.1 Tiered Cache

**Implementation Status**: ✅ Fully Implemented

**Architecture**:
- L1: DashMap (in-memory)
- L2: Redis
- TTL-aware entries
- Lazy eviction (60s background cleanup)

**Use Cases**:
- Token lookups
- Policy caching
- Spend counters
- Rate limit counters
- Circuit breaker state

---

## 10. Observability

### 10.1 Logging & Metrics

**Implementation Status**: ✅ Fully Implemented

**Features**:
| Feature | Status | Details |
|---------|--------|---------|
| Structured JSON Logs | ✅ | TRUEFLOW_LOG_FORMAT=json |
| OpenTelemetry | ✅ | OTLP export to Jaeger/Tempo |
| Prometheus Metrics | ✅ | /metrics endpoint |
| Request ID | ✅ | X-Request-Id header |

### 10.2 External Exporters

**Implementation Status**: ✅ Fully Implemented

| Exporter | Status | Config |
|----------|--------|--------|
| Langfuse | ✅ | LANGFUSE_PUBLIC_KEY, LANGFUSE_SECRET_KEY |
| DataDog | ✅ | DD_API_KEY |
| Custom Webhooks | ✅ | Per-project configuration |

### 10.3 Audit Logging

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Async write with retry (3 attempts, exponential backoff)
- Payload offloading for large bodies
- Fallback to structured logging on DB failure
- GIN-indexed custom properties
- GDPR purge support (project-level)

---

## 11. Human-in-the-Loop (HITL)

### 11.1 Approval Workflow

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Configurable timeout (default 30m)
- Fallback actions (approve/reject)
- Slack/Webhook notifications
- Approval expiry job (60s intervals)
- Idempotency key support

**API Endpoints**:
- `POST /api/v1/approvals/:id/decision`
- `GET /api/v1/approvals`

---

## 12. MCP (Model Context Protocol)

### 12.1 MCP Client

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Streamable HTTP transport
- JSON-RPC 2.0 protocol
- Tool discovery and caching
- OAuth 2.0 auto-discovery (RFC 9728)

**Auth Modes**: None, Bearer, OAuth 2.0

### 12.2 MCP Tool Policies

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Tool allowlist/blocklist per token
- Glob pattern matching
- Project-isolated execution
- Schema caching

---

## 13. Prompt Management

### 13.1 Prompt Registry

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Versioned prompts
- Folder organization
- Variable templating
- Deployment workflow
- Render endpoint (GET/POST)

**API Endpoints**:
- `GET/POST /api/v1/prompts`
- `GET/PUT/DELETE /api/v1/prompts/:id`
- `GET/POST /api/v1/prompts/:id/versions`
- `GET/POST /api/v1/prompts/by-slug/:slug/render`

---

## 14. Experiment Management (A/B Testing)

### 14.1 Traffic Splitting

**Implementation Status**: ✅ Fully Implemented

**Features**:
- Deterministic variant selection (by request_id)
- Experiment tracking in audit logs
- Results analysis endpoint
- Stop experiment endpoint

**Action Example**:
```json
{
  "action": "split",
  "experiment": "model-comparison-q1",
  "variants": [
    {"weight": 70, "name": "control", "set_body_fields": {"model": "gpt-4o"}},
    {"weight": 30, "name": "experiment", "set_body_fields": {"model": "claude-3-5-sonnet"}}
  ]
}
```

---

## 15. Database Layer

### 15.1 PostgreSQL Store

**Implementation Status**: ✅ Fully Implemented

**Tables** (42 migrations):
1. `organizations` - Multi-tenant root
2. `credentials` - Encrypted API keys
3. `tokens` - Virtual tokens
4. `policies` - Policy definitions
5. `policy_versions` - Version history
6. `audit_logs` - Request/response logs
7. `approvals` - HITL requests
8. `sessions` - Session tracking
9. `api_keys` - Management API keys
10. `webhooks` - Webhook configurations
11. `mcp_servers` - MCP server registry
12. `prompts` - Prompt management
13. `experiments` - A/B test tracking
14. `teams` - Organization hierarchy
15. `model_access_groups` - RBAC for models

**Migration Files**: 42 migrations (001-042)

---

## 16. Background Jobs

### 16.1 Scheduled Tasks

**Implementation Status**: ✅ Fully Implemented

| Job | Interval | Purpose |
|-----|----------|---------|
| Cleanup | 1h | Level 2 log expiry |
| Approval Expiry | 60s | Expire stale HITL requests |
| Session Cleanup | 15min | Orphaned session removal |
| Budget Checker | 15min | Project spend alerts |
| Latency Cache Refresh | 5min | p50 latency per model |
| Cache Eviction | 60s | Remove expired L1 entries |
| Key Rotation | 1h (configurable) | DEK re-encryption |

---

## 17. CLI Commands

### 17.1 Token Management

**Implementation Status**: ✅ Fully Implemented

```bash
cargo run -- token create --name my-token --credential cred-id --upstream openai
cargo run -- token list --project-id proj_123
cargo run -- token revoke --token-id tk_abc
```

### 17.2 Credential Management

**Implementation Status**: ✅ Fully Implemented

```bash
cargo run -- credential add --name openai-key --provider openai --key sk-xxx
cargo run -- credential list --project-id proj_123
```

### 17.3 Policy Management

**Implementation Status**: ✅ Fully Implemented

```bash
cargo run -- policy create --name rate-limit --rate-limit 10/min
cargo run -- policy list --project-id proj_123
cargo run -- policy delete --id pol-uuid --project-id proj_123
```

### 17.4 Config-as-Code (IaC)

**Implementation Status**: ✅ Fully Implemented

```bash
cargo run -- config export --api-key admin-key > trueflow.yaml
cargo run -- config plan --file trueflow.yaml --api-key admin-key
cargo run -- config apply --file trueflow.yaml --api-key admin-key
```

---

## 18. Management API

### 18.1 REST Endpoints

**Implementation Status**: ✅ 80+ Endpoints Implemented

**Resource Groups**:
- `/api/v1/tokens` - Token CRUD
- `/api/v1/policies` - Policy CRUD
- `/api/v1/credentials` - Credential CRUD
- `/api/v1/projects` - Project management
- `/api/v1/approvals` - HITL workflow
- `/api/v1/audit` - Audit log access
- `/api/v1/sessions` - Session tracking
- `/api/v1/analytics` - Analytics queries
- `/api/v1/webhooks` - Webhook management
- `/api/v1/mcp/servers` - MCP server management
- `/api/v1/prompts` - Prompt management
- `/api/v1/experiments` - A/B test management
- `/api/v1/teams` - Team management
- `/api/v1/auth/keys` - API key management
- `/api/v1/billing` - Usage endpoints
- `/api/v1/settings` - System settings
- `/api/v1/system` - Cache stats, flush

---

## 19. Security Headers

### 19.1 Response Headers

**Implementation Status**: ✅ Fully Implemented

| Header | Value |
|--------|-------|
| X-Content-Type-Options | nosniff |
| X-Frame-Options | DENY |
| X-XSS-Protection | 1; mode=block |
| Cache-Control | no-store |
| Referrer-Policy | no-referrer |
| Permissions-Policy | camera=(), microphone=(), geolocation=() |
| Strict-Transport-Security | max-age=63072000 (production) |

---

## 20. Known Limitations

### 20.1 Documented Limitations

| Area | Limitation | Mitigation |
|------|------------|------------|
| Circuit Breaker | Half-open limit is instance-local | Set conservative half_open_max_requests |
| PII Streaming | Split-across-chunk PII not detected | Documented, not fixed |
| Redis | Required for operation | Readiness probe returns 503 if down |
| Precision | f64 for spend calculations | Documented precision considerations |

---

## 21. Test Coverage

### 21.1 Test Suites

**Implementation Status**: ✅ Multiple Test Suites

| Suite | Location | Purpose |
|-------|----------|---------|
| Unit Tests | `src/**/tests.rs` | Module-level testing |
| Integration | `tests/integration.rs` | API flow tests |
| Adversarial Unit | `tests/adversarial_unit.rs` | Security boundary tests |
| Full Path | `tests/full_path.rs` | End-to-end request flow |
| Load Tests | `tests/loadtest/` | k6 performance tests |

**Special Features**:
- `--features test-hooks` for integration testing
- Mock upstream support

---

## Summary

TrueFlow Gateway is a **production-ready** enterprise AI gateway with comprehensive feature coverage:

| Category | Completion | Key Features |
|----------|------------|--------------|
| Core Proxy | 100% | Multi-provider, streaming, retries |
| Security | 100% | PII, guardrails, auth, encryption |
| Policy Engine | 100% | 20+ actions, recursive conditions |
| Observability | 100% | OTLP, Prometheus, audit logs |
| Cost Management | 100% | Spend caps, budget alerts |
| API Surface | 100% | 80+ REST endpoints |
| Background Jobs | 100% | 6 scheduled tasks |
| CLI | 100% | Token, credential, policy, IaC |

**Total Implementation**: ~50,000+ lines of Rust code across 100+ source files.