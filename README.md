<div align="center">

# TrueFlow

**An open-source AI gateway written in Rust. Sits between your agents and the upstream models — enforcing policies, protecting credentials, redacting PII, and pausing for human review before anything sensitive ships.**

[![Rust 1.75+](https://img.shields.io/badge/rust-1.75+-D2691E?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![1,051 tests](https://img.shields.io/badge/tests-1%2C051%20passing-22d3ee)]()
[![<1ms overhead](https://img.shields.io/badge/overhead-%3C1ms-a78bfa)]()
[![Docker](https://img.shields.io/badge/docker-ready-2496ED?logo=docker&logoColor=white)](docs/deployment/docker.md)
[![License: Proprietary](https://img.shields.io/badge/license-Proprietary-f59e0b)](LICENSE)

[Quickstart](#quickstart) · [Features](#features) · [API Docs](docs/reference/api.md) · [Roadmap](ROADMAP.md) · [Cloud ↗](https://trueflow.ai)

</div>

---

## The problem

Your agents call OpenAI directly. The real API key lives in a `.env` file, there's no spend cap, no audit trail, and no way to stop a runaway prompt from doing something expensive or embarrassing.

TrueFlow intercepts every call. Your agents get a virtual token — scoped, expiring, budget-capped. The real key stays in an encrypted vault and never reaches the agent process. If a virtual token leaks, you rotate it from the dashboard in one click without touching any infrastructure.

## Migration: two lines

Change the base URL and the key. Every SDK, framework, and raw HTTP call works without further modification.

```diff
- curl https://api.openai.com/v1/chat/completions \
-   -H "Authorization: Bearer sk-prod-YOUR_REAL_KEY"
+ curl https://localhost:8443/v1/chat/completions \
+   -H "Authorization: Bearer tf_v1_YOUR_VIRTUAL_TOKEN"
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "..."}]}'
```

Compatible with any OpenAI-compatible client — **LangChain, CrewAI, LlamaIndex, Vercel AI SDK** — point `base_url` at TrueFlow and nothing else changes.

---

## Features

### Policy engine

Write rules against any field in the request or response: method, path, body, headers, model name, cost, agent identity. When a condition matches, fire one or more of 15+ action types.

```json
{
  "name": "Protect PII and cap spend",
  "conditions": {
    "and": [
      { "==": [{ "var": "request.body.model" }, "gpt-4o"] }
    ]
  },
  "actions": [
    { "action": "redact", "patterns": ["ssn", "email", "credit_card"] },
    { "action": "rate_limit", "max_requests": 100, "window": "1m" }
  ]
}
```

**Available actions:** `deny` · `rate-limit` · `throttle` · `redact` · `transform` · `route` · `split` · `shadow-log` · `webhook` · `require-approval` · `tool-scope` · `override` · `cache` · `spend-cap` · `anomaly-alert`

### Human-in-the-loop approval gates

Flag a class of requests — high-cost tool calls, prompts matching a pattern, specific agent identities — and TrueFlow pauses them before they reach the model. A Slack notification or webhook fires. A reviewer approves or rejects. The agent waits. No application code changes required; configure entirely from the dashboard.

### PII redaction and tokenization

11 built-in patterns (SSN, email, credit card, phone, IBAN, date of birth, IP address, API key, AWS key, driver's license, medical record number) are stripped before the request reaches the model. Plug in [Microsoft Presidio](https://github.com/microsoft/presidio) as an optional sidecar for names, addresses, and multilingual entities — fail-open, so Presidio downtime doesn't block your requests.

### Spend caps that block, not just alert

Set daily, monthly, or lifetime budgets per virtual token. When a token hits its cap, subsequent requests are blocked — not logged and allowed through. Real-time spend is visible per token in the dashboard.

### Credential vault

Real API keys are encrypted at rest with AES-256-GCM envelope encryption. Each key gets a unique per-credential data key wrapped by a master key that never touches the database. Virtual tokens are what agents hold; the underlying credentials are never exposed to the calling process.

### Routing, retries, and circuit breakers

Five load-balancing strategies across any mix of providers: round-robin, weighted, lowest-latency, lowest-cost, and least-busy. Automatic retries with exponential backoff. Per-endpoint circuit breakers open before failures propagate to agents and self-heal when the upstream recovers.

### Audit log

Every request produces a structured log entry: sender identity, model, policy trigger, fields redacted, latency, and cost. Partitioned by month for high-throughput writes. Export to Prometheus, Langfuse, DataDog, or any OpenTelemetry-compatible backend with a single config line.

### SaaS builder support

For SaaS platforms embedding AI capabilities, TrueFlow provides customer-level attribution:
- **External user ID**: Link tokens to customers from your billing system
- **Bulk operations**: Onboard hundreds of customers with a single API call
- **Per-customer analytics**: Track spend by customer across all their tokens
- **Flexible metadata**: Store plan tier, region, or custom attributes on each token

```json
{
  "name": "customer-acme",
  "external_user_id": "cust_abc123",
  "metadata": {"plan": "enterprise", "region": "us-east"},
  "spend_cap_daily": 100
}
```

### A/B model experiments

Split traffic across model variants by weight. Compare latency, cost, and output quality side by side in the dashboard. Promote the winner — no agent code changes required.

---

## Quickstart

```bash
# 1. Clone and configure
git clone https://github.com/your-org/trueflow && cd trueflow
cp .env.example .env   # set POSTGRES_PASSWORD, TRUEFLOW_MASTER_KEY, TRUEFLOW_ADMIN_KEY

# 2. Start the stack (gateway + dashboard + PostgreSQL + Redis)
docker compose up -d

# 3. Open the dashboard
open http://localhost:3000
```

From the dashboard: **Vault** → add a provider key → **Policies** → create a rule → **Virtual Keys** → issue a `tf_v1_...` token. Hand that token to your agent. Done.

---

## Providers

TrueFlow supports chat, streaming, vision, and embeddings across:

OpenAI · Anthropic · Google Gemini · Azure OpenAI · AWS Bedrock · Cohere · Mistral · Groq · Together AI · Ollama

Full capability matrix and authentication details: [docs/guides/providers.md](docs/guides/providers.md)

---

## Stack

| Layer | Technology |
|---|---|
| Gateway | Rust — Axum, Tower, Hyper, Tokio |
| Storage | PostgreSQL 16 + Redis 7 (tiered L1/L2 cache) |
| Encryption | AES-256-GCM envelope encryption |
| Dashboard | Next.js 16 (App Router) |
| SDKs | Python, TypeScript |
| Observability | OpenTelemetry → Jaeger / Langfuse / DataDog / Prometheus |

Hot-path overhead is under 1 ms. SSE streams are proxied word-by-word with no buffering. The test suite has 1,051 tests across unit, integration, adversarial, and E2E layers.

---

## Tests

```bash
cargo test                              # 1,051 Rust unit and integration tests
python -m pytest tests/unit/ -v        # Python SDK unit tests
python tests/e2e/test_mock_suite.py    # Full E2E — 49 phases, requires Docker
```

---

## Documentation

| | |
|---|---|
| [Quickstart](docs/getting-started/quickstart.md) | Zero to running in 5 minutes |
| [Policy Guide](docs/guides/policies.md) | Conditions, actions, shadow mode |
| [API Reference](docs/reference/api.md) | Every management endpoint |
| [Architecture](docs/reference/architecture.md) | System design and data flow |
| [Python SDK](sdk/python/README.md) | OpenAI drop-in, LangChain, async |
| [TypeScript SDK](sdk/typescript/README.md) | OpenAI / Anthropic drop-in, SSE |
| [Providers](docs/guides/providers.md) | Auth details for all 10 providers |
| [Docker](docs/deployment/docker.md) | Compose for dev and production |
| [Kubernetes](docs/deployment/kubernetes.md) | K8s manifests and health probes |
| [Framework Integrations](docs/guides/framework-integrations.md) | LangChain, CrewAI, LlamaIndex |

---

<div align="center">

Self-host with Docker or Kubernetes, or use the [cloud version](https://trueflow.ai) — free to start, no credit card required.

<sub>Proprietary — source available for evaluation. Commercial use requires a license. See <a href="LICENSE">LICENSE</a>.</sub>

</div>