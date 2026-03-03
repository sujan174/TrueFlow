# AILink — Test Suite

All tests live under this directory, organized by layer.

```
tests/
├── unit/                 # Pure unit tests — no gateway, no Docker
│   ├── test_unit.py      # SDK client + all resource methods (mocked)
│   └── test_features_8_10.py  # Batches & fine-tuning resources (mocked)
│
├── integration/          # Live gateway tests — requires docker compose up
│   ├── test_integration.py    # End-to-end SDK usage patterns
│   ├── test_security.py       # Auth, SSRF, RBAC, scope enforcement
│   ├── test_observability.py  # Metrics, Langfuse, Datadog export
│   ├── test_p0_fixes.py       # Rate limit enforcement, atomic spend cap
│   ├── test_phase3.py         # PII redaction, tokenization
│   ├── test_roadmap_features.py  # Framework integrations, spend tracking
│   └── run_integration.sh     # Shell runner for CI
│
├── e2e/                  # Full-stack mock E2E — 176+ tests across 49 phases
│   └── test_mock_suite.py
│
├── realworld/            # Real provider tests — needs live API keys
│   └── test_realworld_suite.py
│
├── mock-upstream/        # FastAPI mock server (OpenAI, Anthropic, Gemini, guardrails, OIDC)
│   ├── server.py         # The mock implementation (~1000 lines)
│   ├── Dockerfile        # Built by docker-compose.test.yml
│   └── requirements.txt
│
├── conftest.py           # Shared pytest fixtures (gateway_url, admin_client, etc.)
└── ci_security_check.sh  # Security gate run in CI
```

---

## Running Tests

### 1. Unit tests — no infrastructure needed

```bash
cd /path/to/ailink
python3 -m pytest tests/unit/ -v
```

### 2. Integration tests — requires a running stack

```bash
# Start the shipping stack
docker compose up -d

# Run integration tests
python3 -m pytest tests/integration/ -v

# Or use the shell runner (sets env vars)
bash tests/integration/run_integration.sh
```

### 3. E2E mock suite — 176+ tests across 49 phases

The E2E suite uses the `mock-upstream` service (no real API keys needed).
The mock upstream is defined in a **separate overlay** (`docker-compose.test.yml`).

```bash
# Start the full test stack (shipping + mock upstream)
docker compose -f docker-compose.yml -f docker-compose.test.yml up -d

# Run the suite
python3 tests/e2e/test_mock_suite.py
```

#### E2E Test Phases

| Phase | Coverage |
|-------|----------|
| 1 | Mock sanity & echo verification |
| 2 | Anthropic translation (system messages, multi-turn) |
| 3 | Gemini translation & SSE streaming (word-by-word deltas, mid-stream drops) |
| 4 | Tool / function call translation (OpenAI ↔ Anthropic ↔ Gemini) |
| 5 | Multimodal inputs (image_url, base64 images) |
| 6 | ContentFilter (keyword blocking, topic denylist, custom regex) |
| 7 | External guardrails (Azure Content Safety, AWS Comprehend, LlamaGuard) |
| 8 | Advanced policies (throttle, A/B split, validate schema, shadow, async) |
| 9 | Transform operations (append/prepend system, set/remove header/body) |
| 10 | Webhook actions (fires on policy match) |
| 11 | Circuit breaker (trip on failures, recovery after timeout) |
| 12 | Admin API CRUD (tokens, policies, credentials) |
| 13 | Non-chat passthrough (embeddings, audio, images, models) |
| 13B | Model access groups RBAC |
| 14 | Response caching (hit, bypass, opt-out) |
| 14B | Team CRUD API (create, update, spend, members lifecycle) |
| 15A | Rate limiting (per-token window, isolation between tokens) |
| 15B | Team-level model enforcement at proxy |
| 16A | Retry policy (auto-retry on 500, skip 400) |
| 16B | Tag attribution & cost tracking via audit logs |
| 17 | Dynamic routing (round-robin, lowest-cost) & conditional routing |
| 18 | Tool scope RBAC (blocked/allowed tools, allowlist enforcement) |
| 19 | Session lifecycle (auto-create, pause/complete rejection) |
| 20 | Anomaly detection (non-blocking, coexists with sessions) |
| 21 | OIDC JWT authentication (format detection, expired/bad-sig rejection) |
| 22 | Cost & token tracking (usage fields, spend caps, lifetime caps) |
| 23 | HITL approval flow (approve, reject, timeout) |
| 24 | MCP server management API |
| 25 | PII redaction (SSN, email, credit card, vault rehydrate) |
| 26 | Prometheus metrics endpoint |
| 27 | Scoped tokens RBAC (read-only keys, scope enforcement) |
| 28 | SSRF protection (private IPs, localhost rejection) |
| 29 | Additional provider translation (Groq, Mistral, Cohere, unknown models) |
| 30 | API key lifecycle (whoami, list, revoke) |
| 31 | Prompt management (CRUD, versioning, labels, render with variables) |
| 32 | A/B experiments (create, variants, results, traffic split, stop) |
| 33 | Guardrail presets (list, enable, disable, status) |
| 34 | Config-as-code (export full config, policies-only, tokens-only) |
| 35 | Condition operators (neq, contains, And/Or composition) + policy version list |
| 36 | Audit log queries (list, get-by-id, scope denial, field verification) |
| 37 | Analytics endpoints (summary, volume, status, latency, timeseries, per-token, spend breakdown) |
| 38 | Projects CRUD (create, list, update, delete, nonexistent delete) |
| 39 | Services CRUD (create, list, delete, nonexistent delete) |
| 40 | Webhooks CRUD (create, list, test delivery, delete) |
| 41 | Notifications (list, unread count, mark-all-read) |
| 42 | Config-as-code round-trip (export → import, empty import) |
| 43 | Model pricing (upsert, list, delete) |
| 44 | Settings (get, update, re-read) |
| 45 | Cache management (cache-stats, flush, stats-after-flush) |
| 46 | Health checks (healthz, readyz, upstream health) |
| 47 | Billing usage (org-level usage, cost verification) |
| 48 | Experiments with traffic (create → send traffic → per-variant results) |
| 49 | HITL edge cases (double decision idempotency, nonexistent approval 404) |

### 4. Real-world suite — requires live API keys

```bash
export GEMINI_API_KEY="..."
export FIRECRAWL_API_KEY="..."
python3 tests/realworld/test_realworld_suite.py
```

### 5. Rust tests (gateway)

```bash
cd gateway
cargo test
```

---

## Mock Upstream Server

The mock upstream (`tests/mock-upstream/server.py`) simulates multiple LLM providers and supporting services:

| Endpoint | Simulates |
|----------|-----------|
| `POST /v1/chat/completions` | OpenAI chat (streaming + non-streaming, tool calls) |
| `POST /v1/messages` | Anthropic messages API (tool calls from body detection) |
| `POST /v1/models/*/generateContent` | Gemini content generation (FUNCTION_CALL finishReason) |
| `POST /v1/models/*/streamGenerateContent` | Gemini streaming |
| `POST /v1/embeddings` | OpenAI embeddings (batch support) |
| `POST /v1/audio/transcriptions` | Whisper audio transcription |
| `POST /v1/images/generations` | DALL-E image generation |
| `GET /v1/models` | Model listing |
| `POST /contentsafety/text:analyze` | Azure Content Safety |
| `POST /comprehend` | AWS Comprehend |
| `POST /webhook/receive` | Webhook receiver with history |
| `GET /.well-known/openid-configuration` | OIDC discovery |
| `POST /oidc/mint` | JWT token minting (RS256) |

**Control headers** for test scenarios: `x-mock-latency`, `x-mock-flaky`, `x-mock-status`, `x-mock-drop-mid-stream`, `x-mock-tool-call`, `x-mock-content`.

---

## Test Layers

| Layer | File(s) | Gateway? | API Keys? | Speed |
|-------|---------|----------|-----------|-------|
| **Unit** | `tests/unit/` | ❌ | ❌ | ~2s |
| **Integration** | `tests/integration/` | ✅ Docker | ❌ | ~30s |
| **E2E (mock)** | `tests/e2e/` | ✅ Docker | ❌ | ~90s |
| **Real-world** | `tests/realworld/` | ✅ Docker | ✅ Required | ~5min |
| **Rust** | `gateway/` cargo test | ❌ | ❌ | ~10s |

---

## Configuration

Integration and E2E tests are configured via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `GATEWAY_URL` | `http://127.0.0.1:8443` | Gateway base URL |
| `ADMIN_KEY` | `ailink-admin-test` | Admin API key |
| `MOCK_UPSTREAM_URL` | `http://mock-upstream:80` | Mock upstream (Docker internal) |
| `AILINK_MOCK_URL` | `http://host.docker.internal:9000` | Mock URL the gateway uses |
| `AILINK_MOCK_LOCAL` | `http://localhost:9000` | Mock URL the test runner uses |
