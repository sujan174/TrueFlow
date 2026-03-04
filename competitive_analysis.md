# TrueFlow — Competitive Analysis

> Conducted March 2026. Based on codebase audit + current competitor docs/features.

---

## Step 1 — TrueFlow Feature Map

### CORE
- Rust reverse-proxy gateway (high-performance, single binary)
- OpenAI-compatible `/v1/chat/completions` endpoint
- Multi-provider upstream routing (OpenAI, Anthropic, Azure, Google, Mistral, etc.)
- Fallback chains across providers
- Request/response streaming (SSE)
- Configurable retry logic with exponential backoff
- Latency-based routing
- Conditional routing (route by body fields, headers, metadata — supports eq/neq/contains/starts_with/ends_with/regex/exists)
- Dynamic routing strategies (lowest-cost, lowest-latency, round-robin, least-busy, weighted-random)
- Token-based rate limiting (per-key, per-model)
- Request/response caching (Redis-backed, exact-match)
- Cost estimation per request (token × pricing table)
- Budget enforcement with hard spend caps

### AUTH
- Virtual keys with scoped permissions
- API key management (create, revoke, rotate)
- OIDC / Google SSO login for dashboard
- RBAC middleware (role-based access control)
- Team-based key hierarchy

### SAFETY
- Policy engine (allow/deny/flag rules on request content)
- Guardrails (regex-based input/output validation)
- External guardrail integration (webhook-based)
- PII vault (detect + tokenize sensitive data, reversible)
- PII redaction middleware (strip before forwarding)
- Content sanitization middleware
- Model access groups (restrict which keys can use which models)
- Human-in-the-loop approval workflow (HITL)
- Anomaly detection middleware

### OBSERVABILITY
- Full audit log with request/response capture
- Session tracking (group related requests)
- Latency trend charts (7-day)
- Spend analytics per key/model
- Real-time dashboard (KPI strip, status strip, trace list)
- Langfuse integration (tracing export)
- DataDog metrics integration
- Anomaly flagging with alert banner

### DEVELOPER
- Python SDK
- TypeScript SDK
- Prompt management (versioning, templates)
- Prompt playground (test prompts against models)
- A/B experiment framework
- MCP tool registry + bridging
- Config export/import (YAML)

### OPS
- Docker Compose deployment (gateway + dashboard + Postgres + Redis)
- Standalone all-in-one Docker image
- Webhook notifications (configurable events)
- Background jobs (budget checker, cleanup)
- Health check endpoint
- Key rotation automation

### UX
- Full Next.js dashboard with 20+ pages
- Guided onboarding modal (3-step flow)
- Dark mode + light mode
- Mobile-responsive layout
- Slide-in mobile nav drawer
- Command palette (keyboard nav)

---

## Step 2 — Competitor Feature Maps

### Portkey (`portkey.ai`)
**Positioning:** "AI Gateway for production AI — the control plane for LLM ops."

| Category | Features |
|----------|----------|
| CORE | Unified API for 250+ models, loadbalancing, fallbacks, retries, conditional routing, circuit breakers, streaming, simple + semantic caching, cost tracking |
| AUTH | Virtual keys with vault, key rotation, usage quotas, team-scoped access, SSO (Okta/Google/GitHub) |
| SAFETY | 60+ built-in guardrails, PII detection, prompt injection blocking, content moderation, guardrail-based routing decisions |
| OBSERVABILITY | Detailed logs, tracing, cost analytics, real-time dashboard, alert rules |
| DEVELOPER | Python/Node/REST SDKs, prompt playground, prompt management, A/B testing |
| OPS | Managed cloud (primary), self-hosted enterprise option, webhooks |
| UX | Polished dashboard (category reference per industry), onboarding, team management |
| COMPLIANCE | SOC2 Type 2, ISO 27001, GDPR, HIPAA (Enterprise tier) |

### LiteLLM (`litellm.ai`)
**Positioning:** "Open-source LLM proxy — drop-in OpenAI replacement for 100+ models."

| Category | Features |
|----------|----------|
| CORE | Python reverse proxy, 100+ model support, fallbacks, loadbalancing, streaming, caching (in-memory, Redis, S3, semantic via Qdrant), budget management |
| AUTH | Virtual keys with spend caps, team/project hierarchy, SSO, RBAC |
| SAFETY | Guardrails (prompt injection, PII masking), custom code guardrails, guardrail policies per team/key |
| OBSERVABILITY | Request logging, spend tracking, admin UI with metrics, Langfuse/DataDog integrations |
| DEVELOPER | Python SDK, OpenAI SDK compatible, model management UI, config via YAML |
| OPS | Self-hosted (primary), Docker + Helm charts, model hot-reload without restart |
| UX | Admin UI (functional but basic), SSO login |
| COMPLIANCE | None publicly certified |

### Kong AI Gateway (`konghq.com`)
**Positioning:** "Enterprise API gateway extended for AI — governance-first."

| Category | Features |
|----------|----------|
| CORE | Pluggable AI proxy, multi-LLM routing, semantic routing, load balancing, prompt compression, rate limiting (token-based, model-based, calendar windows) |
| AUTH | OAuth 2.0, JWT, API key management, RBAC, consumer groups with tiered access |
| SAFETY | PII sanitization (NLP, 20+ categories, 12 languages), automated RAG pipelines, semantic prompt guards, centralized governance policies |
| OBSERVABILITY | Kong Manager dashboard, analytics, logging plugins (Datadog, Splunk, etc.) |
| DEVELOPER | Plugin ecosystem (Lua/Go/Python/JS), no prompt management, no playground |
| OPS | Konnect cloud platform, self-hosted (on-prem), Kubernetes-native, multi-cloud |
| UX | Kong Manager (enterprise-grade but complex) |
| COMPLIANCE | SOC2, ISO 27001, HIPAA, FedRAMP-ready |

### OpenRouter (`openrouter.ai`)
**Positioning:** "Model router — one API for 500+ models, zero infrastructure."

| Category | Features |
|----------|----------|
| CORE | API router for 500+ models from 60+ providers, automatic model routing (cost/speed/reliability), fallbacks, streaming, BYOK, multimodal (text/image/PDF) |
| AUTH | API keys, usage-based billing, credit system, BYOK |
| SAFETY | None (no guardrails, no policies, no PII detection) |
| OBSERVABILITY | Usage tracking, cost dashboard, per-request billing |
| DEVELOPER | OpenAI SDK compatible, structured outputs, tool calling, web search via API |
| OPS | Managed cloud only, no self-hosted option |
| UX | Simple model browser + API docs |
| COMPLIANCE | None publicly certified |

---

## Step 3 — Master Comparison Table

| Feature | TrueFlow | Portkey | LiteLLM | Kong | OpenRouter |
|---------|--------|---------|---------|------|------------|
| **CORE** | | | | | |
| Unified multi-provider API | ✅ | 🏆 250+ | ✅ 100+ | ✅ | 🏆 500+ |
| Fallback chains | ✅ | ✅ | ✅ | ✅ | ✅ |
| Load balancing | ✅ | ✅ | ✅ | 🏆 | 🔶 auto |
| Latency-based routing | ✅ | ✅ | ✅ | 🏆 semantic | ✅ auto |
| Conditional routing | ✅ | 🏆 | 🔶 | ✅ | ❌ |
| Circuit breakers | ✅ | ✅ | ❌ | 🏆 | ❌ |
| Streaming (SSE) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cached response streaming | ❌ | ✅ | ❌ | ❌ | ❌ |
| Simple caching (exact match) | ✅ | ✅ | ✅ | ❌ | ❌ |
| Semantic caching | ❌ | 🏆 | ✅ Qdrant | ❌ | ❌ |
| Token-based rate limiting | ✅ | ✅ | ✅ | 🏆 model-lvl | ❌ |
| Budget / spend caps | ✅ | ✅ | 🏆 hierarchical | ❌ | 🔶 credits |
| Cost estimation per request | ✅ | 🏆 | ✅ | ❌ | ✅ |
| Prompt compression | ❌ | ❌ | ❌ | 🏆 | ❌ |
| **AUTH** | | | | | |
| Virtual keys | ✅ | 🏆 | ✅ | ❌ | ❌ |
| Key rotation | ✅ | ✅ | 🔶 | ❌ | ❌ |
| SSO (OIDC) | ✅ Google | 🏆 Okta/Google/GitHub | ✅ | ✅ | ❌ |
| RBAC | ✅ | ✅ | ✅ | 🏆 | ❌ |
| Team hierarchy | 🔶 | ✅ | 🏆 | ✅ | ❌ |
| **SAFETY** | | | | | |
| Policy engine (rules) | ✅ | ✅ | ✅ | 🏆 | ❌ |
| Input/output guardrails | ✅ regex | 🏆 60+ | ✅ | ✅ NLP | ❌ |
| PII detection + redaction | ✅ regex | ✅ | ✅ | 🏆 NLP 12-lang | ❌ |
| PII vault (tokenize + retrieve) | 🏆 | ❌ | ❌ | ❌ | ❌ |
| External guardrail webhooks | ✅ | 🔶 | ✅ | ❌ | ❌ |
| Human-in-the-loop approvals | 🏆 | ❌ | ❌ | ❌ | ❌ |
| Anomaly detection | 🏆 | ❌ | ❌ | ❌ | ❌ |
| Model access groups | ✅ | ✅ | ✅ | ✅ | ❌ |
| Content sanitization | ✅ | ✅ | 🔶 | ✅ | ❌ |
| RAG pipeline (built-in) | ❌ | ❌ | ❌ | 🏆 | ❌ |
| Prompt guard (semantic) | ❌ | 🔶 | ❌ | 🏆 | ❌ |
| **OBSERVABILITY** | | | | | |
| Audit logs | ✅ | ✅ | ✅ | 🏆 | ❌ |
| Session tracking | 🏆 | 🔶 | ❌ | ❌ | ❌ |
| Spend analytics | ✅ | 🏆 | ✅ | 🔶 | ✅ |
| Latency trends / charting | ✅ | 🏆 | 🔶 | 🔶 | ❌ |
| Real-time dashboard | ✅ | 🏆 | 🔶 | ✅ | ❌ |
| Langfuse integration | ✅ | ❌ | ✅ | ❌ | ❌ |
| DataDog integration | ✅ | ❌ | ✅ | 🏆 plugin | ❌ |
| Alert rules / anomaly banners | ✅ | ✅ | ❌ | ✅ | ❌ |
| **DEVELOPER** | | | | | |
| Python SDK | ✅ | 🏆 | ✅ | ❌ | ❌ |
| TypeScript/Node SDK | ✅ | 🏆 | ❌ | ❌ | ❌ |
| Prompt management | ✅ | 🏆 | ❌ | ❌ | ❌ |
| Prompt playground | ✅ | 🏆 | ❌ | ❌ | ❌ |
| A/B experiments | ✅ | ✅ | ❌ | ❌ | ❌ |
| MCP tool bridging | ✅ | 🔶 | 🔶 | ❌ | ❌ |
| Config export/import | 🏆 | ❌ | ✅ YAML | ❌ | ❌ |
| Plugin/extension system | ❌ | ❌ | ❌ | 🏆 | ❌ |
| **OPS** | | | | | |
| Self-hosted | ✅ | 🔶 enterprise | ✅ | ✅ | ❌ |
| Docker single-command deploy | 🏆 | ❌ | ✅ | 🔶 | ❌ |
| Managed cloud | ❌ | 🏆 | 🔶 | ✅ | 🏆 |
| Webhooks | ✅ | ✅ | ❌ | ✅ | ❌ |
| Kubernetes-native | ❌ | ❌ | ✅ Helm | 🏆 | ❌ |
| **UX** | | | | | |
| Full dashboard | ✅ | 🏆 | 🔶 basic | ✅ | ❌ |
| Onboarding flow | ✅ | 🏆 | ❌ | ❌ | ❌ |
| Dark mode | ✅ | ✅ | ❌ | ✅ | ✅ |
| Mobile responsive | ✅ | ✅ | ❌ | 🔶 | ❌ |
| Command palette | 🏆 | ❌ | ❌ | ❌ | ❌ |
| **COMPLIANCE** | | | | | |
| SOC2 | ❌ | 🏆 Type 2 | ❌ | ✅ | ❌ |
| ISO 27001 | ❌ | ✅ | ❌ | ✅ | ❌ |
| HIPAA | ❌ | ✅ enterprise | ❌ | ✅ | ❌ |
| GDPR | ❌ | ✅ | ❌ | ✅ | ❌ |

---

## Step 4 — Gap Analysis

### Features we have that NO competitor has

| Feature | Moat Strength | Why |
|---------|---------------|-----|
| **PII Vault** (tokenize + retrieve) | **STRONG MOAT** | No competitor offers reversible PII tokenization. Portkey/Kong redact permanently. This is a genuine differentiator for regulated industries. |
| **Human-in-the-loop approvals** | **STRONG MOAT** | No competitor has a built-in HITL approval workflow. Enterprises in regulated sectors (finance, healthcare) need human sign-off on certain AI outputs. Hard to bolt on. |
| **Anomaly detection** | **WEAK MOAT** | Useful but straightforward to replicate — it's statistical thresholding on latency/error rate. |
| **Command palette** | **WEAK MOAT** | Nice DX but trivial to copy. |
| **Config export/import** | **WEAK MOAT** | Useful for GitOps workflows. LiteLLM has YAML config but not a UI-driven export. |
| **Rust gateway** | **STRONG MOAT** | Performance advantage over Python (LiteLLM) and Node proxies. Kong is Go/Lua (comparable). Matters at scale. |

### Features competitors have that we are MISSING

| Feature | Gap Severity | Leader |
|---------|--------------|--------|
| **Semantic caching** | **IMPORTANT GAP** | Portkey (best), LiteLLM (Qdrant). Saves significant cost on near-duplicate queries. |
| **NLP-based PII detection** (multi-language) | **IMPORTANT GAP** | Kong (20+ categories, 12 languages via NLP). Our regex covers English only. |
| **Managed cloud offering** | **CRITICAL GAP** | Portkey, Kong, OpenRouter. Many buyers want zero-ops. Self-hosted-only limits TAM significantly. |
| **SOC2 / ISO 27001 / HIPAA** | **CRITICAL GAP** | Portkey (SOC2 Type 2), Kong (SOC2 + ISO + HIPAA). Hard procurement blocker for enterprise deals >$50K. |
| **RAG pipeline** (built-in) | **NICE TO HAVE** | Kong only. Niche — most teams do RAG in app code. |
| **Semantic prompt guards** | **NICE TO HAVE** | Kong. Regex-based guards cover 80% of needs. |
| **Plugin/extension system** | **NICE TO HAVE** | Kong (massive plugin ecosystem). High investment, low priority for focused AI gateway. |
| **Prompt compression** | **NICE TO HAVE** | Kong only. Marginal cost savings. |
| **Kubernetes Helm chart** | **IMPORTANT GAP** | LiteLLM, Kong. Enterprise ops teams expect Helm. Docker Compose is dev-friendly but not prod-grade. |
| **Cached response streaming** | **NICE TO HAVE** | Portkey only. Small UX improvement. |

### Features we have in an INFERIOR version

| Feature | Best | What they do better |
|---------|------|---------------------|
| **Guardrails** | Portkey (60+ built-in) | Our guardrails are regex-based. Portkey ships 60+ pre-built detectors for prompt injection, toxicity, PII, off-topic, etc. out of the box. |
| **Dashboard UX** | Portkey | Cited as category reference. Better empty states, smoother onboarding, more polished animations. We've improved but still behind. |
| **SSO providers** | Portkey (Okta/Google/GitHub) | We only support Google OIDC. Enterprise needs Okta/SAML. |
| **Team hierarchy** | LiteLLM | LiteLLM has project → team → key hierarchy with per-level budgets. Ours is flatter. |

---

## Step 5 — Positioning Verdict

### 1. Where do we win clearly right now?

**Safety-first self-hosted deployments.** The combination of PII vault (reversible tokenization), HITL approval workflows, and anomaly detection is unique across all competitors. For regulated enterprises that must self-host and need human oversight of AI outputs, no competitor matches this stack. The Rust gateway also gives us a legitimate performance edge over LiteLLM (Python) at high throughput.

### 2. Where are we most at risk of losing deals?

**Enterprise procurement and zero-ops buyers.** No SOC2, no managed cloud, no Okta SSO. Any enterprise deal >$50K with a procurement team will ask for these, and we can't check those boxes today. Portkey can, Kong can, and buyers will walk. The second risk is teams who want plug-and-play guardrails — Portkey's 60+ built-in detectors vs. our regex-based system is a clear win for them.

### 3. What is the single highest-leverage feature to build next?

**Managed cloud offering.** It unblocks the entire "build vs. buy" segment that self-hosted can't reach. SOC2 is the close second — but SOC2 is a 3-6 month process, while a basic managed tier (hosted gateway + dashboard behind auth) could ship in 2-4 weeks. Semantic caching is third — it has immediate, measurable ROI for every customer.

### 4. Which competitor is our most dangerous rival and why?

**Portkey.** They target the exact same buyer (AI teams shipping to production) with a broader feature set, better UX, compliance certifications, managed cloud, *and* they're making their enterprise gateway free. They have every feature we have except PII vault and HITL approvals, plus 60+ guardrails, semantic caching, circuit breakers, and SOC2. LiteLLM is open-source competition but has weaker UX and no compliance. Kong plays in a different league (general API gateway extended for AI — more ops-heavy, less AI-native). OpenRouter is a model router, not a governance gateway — different category.
