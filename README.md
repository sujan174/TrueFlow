<p align="center">
  <img src="https://img.shields.io/badge/A-TrueFlow-49111c?style=for-the-badge&labelColor=49111c&color=8e2137" alt="TrueFlow" height="40"/>
</p>

<h1 align="center">TrueFlow</h1>
<h3 align="center">The Enterprise AI Agent Gateway</h3>

<p align="center">
  Route, govern, and observe every AI call — from any agent, to any model, through one secure layer.
</p>

<p align="center">
  <a href="docs/getting-started/quickstart.md"><strong>Quickstart</strong></a> ·
  <a href="docs/FEATURES.md"><strong>Features</strong></a> ·
  <a href="docs/reference/api.md"><strong>API Reference</strong></a> ·
  <a href="docs/reference/architecture.md"><strong>Architecture</strong></a> ·
  <a href="ROADMAP.md"><strong>Roadmap</strong></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75+-D2691E?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/license-Proprietary-red" alt="License" />
  <img src="https://img.shields.io/badge/tests-1%2C051%20passing-brightgreen" alt="Tests" />
  <img src="https://img.shields.io/badge/latency-%3C1ms%20overhead-8e2137" alt="Latency" />
  <img src="https://img.shields.io/badge/docker-ready-2496ED?logo=docker&logoColor=white" alt="Docker" />
</p>

---

## Why TrueFlow?

Your AI agents need API keys to do anything useful — OpenAI, Anthropic, Gemini, AWS.
Most teams hardcode them in `.env` files with **zero governance**.

**TrueFlow changes that.** Instead of handing agents real keys (`sk_live_...`), you issue **virtual tokens** (`tf_v1_...`). The gateway enforces your policies, injects the real key server-side, and the agent never sees it.

```
Agent (virtual token) ──▶ TrueFlow Gateway (policy + inject) ──▶ Provider (real key)
```

> **"You manage the Intelligence. We manage the Access."**

---

## Quickstart

### 1. Start the stack

```bash
git clone https://github.com/sujan174/trueflow.git && cd trueflow
docker compose up -d
```

> **Dashboard** → [localhost:3000](http://localhost:3000) &nbsp;|&nbsp; **Gateway** → [localhost:8443](http://localhost:8443)

### 2. Store a credential, create a policy, issue a token

1. **Vault** → Add your OpenAI / Anthropic / Gemini API key
2. **Policies** → Create a content filter or spend cap
3. **Virtual Keys** → Generate an `tf_v1_...` token

### 3. Use it — change 2 lines of code

```python
# pip install trueflow
from trueflow import TrueFlowClient

client = TrueFlowClient(
    api_key="tf_v1_...",
    gateway_url="http://localhost:8443"
)

# Drop-in replacement for OpenAI
oai = client.openai()
resp = oai.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello from TrueFlow!"}]
)
print(resp.choices[0].message.content)
```

Works with **any OpenAI-compatible SDK** — LangChain, CrewAI, LlamaIndex, Vercel AI SDK — just point `base_url` at TrueFlow.

---

## Features

<table>
<tr>
<td width="50%">

### 🔐 Security & Access Control
- **Key Isolation** — Real keys never leave the vault
- **AES-256-GCM** envelope encryption at rest
- **OIDC / SSO** — Okta, Auth0, Entra ID with JWKS
- **RBAC** — Teams, model access groups, scoped tokens
- **Human-in-the-Loop** — Approval gates for high-stakes ops

</td>
<td width="50%">

### 🛡️ Guardrails & Safety
- **100+ safety patterns** with 22 presets
- **5 vendor integrations** — Azure, AWS, LlamaGuard, Palo Alto, Prompt Security
- **PII redaction** — SSN, email, CC, phone auto-stripped
- **PII tokenization** — Deterministic vault tokens
- **Content filters** — Jailbreak, injection, topic deny/allow

</td>
</tr>
<tr>
<td>

### ⚙️ Policy Engine
- **15+ action types** — deny, throttle, transform, split, shadow, webhook
- **A/B Traffic Splitting** — weighted variants with analytics
- **Nested AND/OR conditions** on method, path, body, headers
- **Shadow mode** — Test policies without blocking traffic
- **Config-as-Code** — Export/import via YAML or JSON

</td>
<td>

### 📊 Observability & Cost
- **Full audit trail** — Who, what, when, which policy, cost
- **Spend caps** — Daily / monthly / lifetime per token
- **Team budgets** — Per-team spend tracking and enforcement
- **Anomaly detection** — Sigma-based velocity spike alerts
- **Export** — Prometheus, Langfuse, DataDog, OpenTelemetry

</td>
</tr>
<tr>
<td>

### 🔄 Routing & Resilience
- **5 load-balancing strategies** — Round-robin, weighted, latency, cost, least-busy
- **Smart retries** — Exponential backoff with Retry-After
- **Circuit breakers** — Per-token failure tracking & recovery
- **Response caching** — Deterministic cache keys
- **Multi-provider translation** — OpenAI ↔ Anthropic ↔ Gemini

</td>
<td>

### 🤖 AI-Native Features
- **Prompt Management** — CRUD, versioning, label-based deploy
- **A/B Experiments** — Model comparison with traffic splitting
- **MCP integration** — Auto-discover & inject MCP tools
- **SSE streaming** — Word-by-word delta proxying
- **Multimodal** — Vision, audio transcription, embeddings

</td>
</tr>
</table>

**[View all features →](docs/FEATURES.md)**

---

## Architecture

```
                              ┌─────────────────────────────────────────────────────────┐
                              │                    TrueFlow Gateway (Rust)                 │
                              │                                                         │
  Agent / SDK                 │   Token Auth ──▶ Policy Engine ──▶ Guardrails            │        Providers
 ─────────────▶               │       │              │                │                  │    ──────────────▶
  tf_v1_...               │       ▼              ▼                ▼                  │      OpenAI
                              │   AES Vault     Transform        PII Redact              │      Anthropic
                              │       │          (headers,        (SSN, CC,              │      Gemini
                              │       ▼          body, system)     email)                │      Azure
                              │   Credential                          │                  │      Bedrock
                              │   Injection ──────────────────────────┘                  │      Cohere
                              │       │                                                  │      Ollama
                              │       ▼                                                  │
                              │   Load Balancer ──▶ Circuit Breaker ──▶ Retry            │
                              │       │                                   │              │
                              │       ▼                                   ▼              │
                              │   Audit Log + Spend Tracking + Anomaly Detection         │
                              └─────────────────────────────────────────────────────────┘
                                         │                  │                │
                                    PostgreSQL           Redis           Jaeger
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| **Gateway** | Rust — Axum, Tower, Hyper, Tokio |
| **Data** | PostgreSQL 16 + Redis 7 (tiered cache) |
| **Encryption** | AES-256-GCM envelope encryption |
| **Dashboard** | Next.js 16 (App Router, ShadCN) |
| **SDKs** | Python & TypeScript |
| **Observability** | OpenTelemetry → Jaeger / Langfuse / DataDog / Prometheus |
| **Deployment** | Docker Compose / Kubernetes |

---

## Supported Providers

| Provider | Chat | Streaming | Vision | Embeddings |
|---|:---:|:---:|:---:|:---:|
| **OpenAI** | ✅ | ✅ | ✅ | ✅ |
| **Anthropic** | ✅ | ✅ | ✅ | — |
| **Google Gemini** | ✅ | ✅ | ✅ | ✅ |
| **Azure OpenAI** | ✅ | ✅ | ✅ | ✅ |
| **AWS Bedrock** | ✅ | ✅ | ✅ | ✅ |
| **Cohere** | ✅ | ✅ | — | ✅ |
| **Mistral** | ✅ | ✅ | — | ✅ |
| **Groq** | ✅ | ✅ | — | — |
| **Together AI** | ✅ | ✅ | — | ✅ |
| **Ollama** | ✅ | ✅ | ✅ | ✅ |

**[View full provider details →](docs/guides/providers.md)**

---

## Project Structure

```
trueflow/
├── gateway/                  # Rust gateway — the core (39k lines)
│   ├── src/
│   │   ├── middleware/       # Policy engine, guardrails, PII, audit, MCP
│   │   ├── proxy/            # Upstream proxy, retry, model router, streaming
│   │   ├── vault/            # AES-256-GCM credential storage
│   │   ├── api/              # Management REST API
│   │   └── mcp/              # MCP client, registry, types
│   └── migrations/           # SQL migrations (001–036)
├── dashboard/                # Next.js admin UI (26k lines)
├── sdk/python/               # Python SDK — pip install trueflow
├── sdk/typescript/           # TypeScript SDK — npm install @trueflow/sdk
├── tests/                    # 1,051 tests across all layers
└── docker-compose.yml
```

---

## Documentation

| Doc | Description |
|---|---|
| **[Quickstart](docs/getting-started/quickstart.md)** | Zero to running in 5 minutes |
| **[All Features](docs/FEATURES.md)** | Comprehensive feature reference |
| **[API Reference](docs/reference/api.md)** | Every endpoint, request/response |
| **[Policy Guide](docs/guides/policies.md)** | Conditions, actions, shadow mode |
| **[Python SDK](sdk/python/README.md)** | OpenAI drop-in, LangChain, async |
| **[TypeScript SDK](sdk/typescript/README.md)** | OpenAI/Anthropic drop-in, SSE streaming |
| **[Providers](docs/guides/providers.md)** | 10 LLM providers — auth, models, features |
| **[Architecture](docs/reference/architecture.md)** | System design, data flow |
| **[Docker](docs/deployment/docker.md)** | Docker Compose for dev and production |
| **[Kubernetes](docs/deployment/kubernetes.md)** | K8s manifests, health probes |
| **[Frameworks](docs/guides/framework-integrations.md)** | LangChain, CrewAI, LlamaIndex |

---

## Tests

```bash
cargo test                                    # 1,051 Rust unit + integration tests
python3 -m pytest tests/unit/ -v             # Python unit tests
python3 tests/e2e/test_mock_suite.py          # Full E2E (49 phases, requires Docker)
```

---

## Contributing

See **[CONTRIBUTING.md](.github/CONTRIBUTING.md)** for development setup and PR guidelines.

---

## License

**Proprietary** — Source available for evaluation only. Commercial use requires a license. See [LICENSE](LICENSE).
