<p align="center">
  <img src="https://img.shields.io/badge/TrueFlow-AI%20Gateway-0a0a0f?style=for-the-badge&labelColor=0a0a0f&color=22d3ee" alt="TrueFlow" height="36"/>
</p>

<h1 align="center">TrueFlow</h1>

<p align="center"><strong>The open-source AI gateway that sits between your agents and the models —<br/>enforcing policies, guarding secrets, and keeping a human in the loop.</strong></p>

<p align="center">
  <a href="#quickstart">Quickstart</a> ·
  <a href="#features">Features</a> ·
  <a href="docs/reference/api.md">API Docs</a> ·
  <a href="ROADMAP.md">Roadmap</a> ·
  <a href="https://trueflow.ai">Cloud ↗</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75+-D2691E?logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/tests-1%2C051%20passing-22d3ee" alt="Tests"/>
  <img src="https://img.shields.io/badge/latency-%3C1ms%20overhead-a78bfa" alt="Latency"/>
  <img src="https://img.shields.io/badge/docker-ready-2496ED?logo=docker&logoColor=white" alt="Docker"/>
  <img src="https://img.shields.io/badge/license-Proprietary-f59e0b" alt="License"/>
</p>

---

## Two-line migration. Zero trust required.

Your agents call OpenAI directly — real API keys in `.env` files, no spend limits, no audit trail, no way to stop them. TrueFlow intercepts every call: enforcing your policies, stripping PII, capping spend, and pausing for human review before anything sensitive ships.

**Before** — your agent holds the real key:

```bash
curl https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer sk-prod-YOUR_REAL_KEY_HERE" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "..."}]}'
```

**After** — change the URL and the key. Everything else is identical:

```bash
curl https://localhost:8443/v1/chat/completions \
  -H "Authorization: Bearer tf_v1_YOUR_VIRTUAL_TOKEN" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "..."}]}'
```

The real key never leaves TrueFlow's encrypted vault. Your agent holds a virtual token that expires when you say, costs what you allow, and can't do anything your policy doesn't permit. If it leaks, you rotate it in one click — no key rotation across every service.

Works with any OpenAI-compatible SDK — **LangChain, CrewAI, LlamaIndex, Vercel AI SDK** — just point `base_url` at TrueFlow.

---

## Features

### Human-in-the-Loop approval gates
Before a high-stakes tool call or sensitive prompt reaches the model, TrueFlow pauses and notifies your team via Slack or webhook. A reviewer approves or rejects in real time. The agent waits. No code changes required — configure it entirely from the dashboard.

### Policy engine with 15+ action types
Write conditions against any field in the request or response — method, path, body, headers, model, cost, agent name. When a rule matches, fire any combination of actions:

```json
{
  "name": "Protect PII and cap spend",
  "conditions": { "and": [
    { "==": [{ "var": "request.body.model" }, "gpt-4o"] }
  ]},
  "actions": [
    { "action": "redact", "patterns": ["ssn", "email", "credit_card"] },
    { "action": "rate_limit", "max_requests": 100, "window": "1m" }
  ]
}
```

**Actions:** deny · rate-limit · throttle · redact · transform · route · split · shadow-log · webhook · require-approval · tool-scope · override · cache · spend-cap · anomaly-alert

### PII redaction & tokenization
11 built-in patterns (SSN, email, credit card, phone, IBAN, DOB, IP, API key, AWS key, driver's license, MRN) stripped automatically before the request reaches the model. Add Microsoft Presidio as a sidecar for names, addresses, and multilingual entities — fully optional, fail-open.

### Spend caps that actually stop spending
Daily, monthly, and lifetime budgets per virtual token. When a token hits its cap, requests block — not just alert. Track real-time spend across every token from the dashboard.

### Credential vault with envelope encryption
Real API keys are encrypted at rest with AES-256-GCM. Each key gets a unique per-credential data key wrapped by a master key that never touches the database. Your agents hold virtual tokens — if one leaks, rotate it in one click without touching any infrastructure.

### Routing, retries, and circuit breakers
Five load-balancing strategies (round-robin, weighted, lowest-latency, lowest-cost, least-busy) across any provider mix. Automatic retries with exponential backoff. Circuit breakers per endpoint open before your agents start seeing failures — and self-heal when the upstream recovers.

### Full audit trail, always
Every request logged: who sent it, which model, which policy triggered, what was redacted, how long it took, and what it cost. Partitioned by month for high-throughput writes. Export to **Prometheus, Langfuse, DataDog, or OpenTelemetry** in one config line.

### A/B model experiments
Split traffic across model variants by weight. Compare latency, cost, and quality side by side. Promote the winner from the dashboard — zero agent code changes.

---

## Quickstart

```bash
# 1. Clone and configure
git clone https://github.com/your-org/trueflow && cd trueflow
cp .env.example .env        # add POSTGRES_PASSWORD, TRUEFLOW_MASTER_KEY, TRUEFLOW_ADMIN_KEY

# 2. Start the full stack (gateway + dashboard + postgres + redis)
docker compose up -d

# 3. Open the dashboard
open http://localhost:3000
```

In the dashboard: **Vault** → add your provider key → **Policies** → create a rule → **Virtual Keys** → issue a `tf_v1_...` token.

That token is what your agents use. You own everything else.

---

## Supported providers

| Provider | Chat | Streaming | Vision | Embeddings |
|---|:---:|:---:|:---:|:---:|
| OpenAI | ✅ | ✅ | ✅ | ✅ |
| Anthropic | ✅ | ✅ | ✅ | — |
| Google Gemini | ✅ | ✅ | ✅ | ✅ |
| Azure OpenAI | ✅ | ✅ | ✅ | ✅ |
| AWS Bedrock | ✅ | ✅ | ✅ | ✅ |
| Cohere | ✅ | ✅ | — | ✅ |
| Mistral | ✅ | ✅ | — | ✅ |
| Groq | ✅ | ✅ | — | — |
| Together AI | ✅ | ✅ | — | ✅ |
| Ollama | ✅ | ✅ | ✅ | ✅ |

---

## Stack

| Layer | Technology |
|---|---|
| Gateway | Rust — Axum, Tower, Hyper, Tokio |
| Storage | PostgreSQL 16 + Redis 7 (tiered L1/L2 cache) |
| Encryption | AES-256-GCM envelope encryption |
| Dashboard | Next.js 16 (App Router) |
| SDKs | Python · TypeScript |
| Observability | OpenTelemetry → Jaeger / Langfuse / DataDog / Prometheus |

**< 1 ms** overhead on the hot path. 1,051 tests across unit, integration, adversarial, and E2E layers. Word-by-word SSE streaming proxied with no buffering.

---

## Documentation

| | |
|---|---|
| [Quickstart](docs/getting-started/quickstart.md) | Zero to running in 5 minutes |
| [Policy Guide](docs/guides/policies.md) | Conditions, actions, shadow mode |
| [API Reference](docs/reference/api.md) | Every management endpoint |
| [Python SDK](sdk/python/README.md) | OpenAI drop-in, LangChain, async |
| [TypeScript SDK](sdk/typescript/README.md) | OpenAI / Anthropic drop-in, SSE |
| [Providers](docs/guides/providers.md) | Auth details for all 10 providers |
| [Architecture](docs/reference/architecture.md) | System design and data flow |
| [Docker](docs/deployment/docker.md) | Compose for dev and production |
| [Kubernetes](docs/deployment/kubernetes.md) | K8s manifests and health probes |
| [Frameworks](docs/guides/framework-integrations.md) | LangChain · CrewAI · LlamaIndex |

---

## Tests

```bash
cargo test                              # 1,051 Rust tests
python -m pytest tests/unit/ -v        # Python SDK unit tests
python tests/e2e/test_mock_suite.py    # Full E2E — 49 phases, requires Docker
```

---

<p align="center">
  <br/>
  <strong>Cloud version at <a href="https://trueflow.ai">trueflow.ai</a> — free to start, no credit card required.</strong>
  <br/><br/>
  <sub>Proprietary — source available for evaluation. Commercial use requires a license. See <a href="LICENSE">LICENSE</a>.</sub>
</p>
