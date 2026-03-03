# AIlink — Product Vision & Strategy

> **"You manage the Intelligence. We manage the Access."**

---

## Why AIlink Exists

Current AI Agent deployments are insecure.
When a developer builds an agent using LangChain, CrewAI, AutoGen, or vanilla Python, they hand that agent real API keys — Stripe, GitHub, AWS, Slack, OpenAI — to do useful work. These keys usually live in `.env` files, hardcoded variables, or scattered across systems with no oversight.

### Core Risks

1. **Data Leak**: Prompt injection can reveal environment variables (`os.environ`).
2. **Runaway Cost**: Infinite loops can drain API budgets rapidly.
3. **Accidental Damage**: Agents with broad permissions can delete or corrupt production data.

**AIlink makes AI Agents secure by default.**

---

## What AIlink Is

AIlink is a **Secure API Gateway** purpose-built for AI Agents. It sits between the agent and every external API, acting as a security and governance layer.

### The Architecture

Instead of giving agents real API keys, you issue **virtual tokens**. The agent sends requests to AIlink, which enforces policies and injects the real key on the backend.

**This enables:**

| Capability | Benefit |
|---|---|
| **Key Isolation** | Real keys never leave the vault. Prompt injection cannot exfiltrate credentials. |
| **Policy Enforcement** | Control endpoints, methods, rates, and spend per agent. |
| **Human-in-the-Loop** | Pause high-stakes operations for approval. |
| **Shadow Mode** | Deploy and observe policies without breaking existing workflows. |
| **Auto-Rotation** | Automatically rotate provider keys every 24h. |
| **Audit Trail** | Complete log of every request and policy decision. |

---

## Who It's For

### Primary: AI Agent Developers

Developers building agents with LangChain, CrewAI, AutoGen, or custom Python/TypeScript that call external APIs.

**Their pain today:**
- Managing `.env` files across local machines, CI/CD, and production
- No way to limit what an agent can do with a key
- No visibility into agent API activity
- Fear of prompt injection key theft

**What AIlink gives them:**
- `pip install ailink` + one line of code  
- Never touch `.env` files again
- Per-agent, per-API access control
- Sleep at night

### Secondary: Enterprise Platform Teams

Companies deploying 10+ agents across teams, needing governance and compliance.

**Their pain today:**
- No centralized management of agent credentials
- Compliance gaps — no audit trail for agent actions
- CISO blocking AI adoption due to security concerns

**What AIlink gives them:**
- Centralized credential vault with envelope encryption
- Organization-wide policy enforcement
- Comprehensive audit logs for compliance
- Human-in-the-loop approvals for sensitive operations
- Auto-rotation of credentials meeting security policy requirements

---

## Market Position

AIlink occupies a unique intersection that no existing product covers:

```
                    ┌───────────────────────────┐
                    │     AI-Agent Specific      │
                    │                           │
                    │      ★ AIlink ★           │
                    │   (credential + policy    │
                    │    + HITL + audit)        │
                    │                           │
         ┌──────────┤                           ├──────────┐
         │          └───────────────────────────┘          │
         │                                                 │
  ┌──────▼──────┐                                  ┌───────▼──────┐
  │  Secrets     │                                  │  API Gateway  │
  │  Management  │                                  │  / Proxy      │
  │              │                                  │               │
  │ HashiCorp    │                                  │ Kong, Cloud-  │
  │ Vault, AWS   │                                  │ flare, NGINX  │
  │ Secrets Mgr  │                                  │               │
  └──────────────┘                                  └───────────────┘

  "Stores secrets,         "Routes requests,
   but doesn't proxy        but doesn't understand
   agent requests"           agent security needs"
```

### Competitive Differentiators

1. **Agent-native** — SDK built for AI frameworks (LangChain, CrewAI), not generic HTTP clients
2. **Policy-first** — Declarative JSON policies with shadow mode for safe rollout
3. **HITL built-in** — Approval workflows are first-class, not bolted on
4. **Developer-first** — `pip install ailink` and one line of code
5. **Auto-rotation** — Real API keys rotate automatically on a schedule

---

## Strategic Timing

1. **AI Agent Adoption**: Frameworks like LangChain and AutoGen are driving agent deployment. Someone needs to secure the credentials these agents use.

2. **Enterprise Governance**: Security teams block agent adoption because there's no visibility or control. AIlink fills that gap.

3. **MCP Support**: The Model Context Protocol opens a new attack surface for tool use. AIlink can sit in front of MCP tool calls.

---

## Business Model

### Open Core

| Tier | Price | Features |
|---|---|---|
| **Community** | Free (self-hosted) | Gateway + vault + policies + Python/TS SDKs + audit logs |
| **Team** | $49/mo per project | Slack HITL + shadow mode + spend tracking + 90-day log retention |
| **Enterprise** | $299/mo per project | Dashboard + auto-rotation + SSO/RBAC + HashiCorp Vault / AWS KMS + priority support |
| **Custom** | Contact sales | Dedicated deployment + SLA + compliance certifications + custom rotation adapters |

### Revenue Drivers

1. **HITL** — Teams that need approval workflows upgrade to Team tier
2. **Auto-rotation** — Security-conscious organizations upgrade to Enterprise
3. **Compliance** — SOC 2 / HIPAA requirements drive Enterprise adoption
4. **Volume** — SaaS metered by proxied request volume at scale

---

## Product Roadmap

### ✅ Shipped

- **Rust Gateway** — High-performance reverse proxy (Axum), policy engine, circuit breaker, response caching, MCP tool injection, guardrail presets (22 categories, 100+ patterns), session tracing, spend caps, HITL approvals, multi-upstream load balancing
- **Next.js Dashboard** — Full management UI, analytics, audit logs, sessions, playground, project switcher, command palette, mobile nav
- **Python SDK** — OpenAI drop-in, async, HITL, fallback patterns, health polling, session tracing, guardrails, BYOK passthrough, LangChain/CrewAI/LlamaIndex integrations
- **TypeScript SDK** — Full parity: OpenAI/Anthropic drop-in, admin management API, health polling, guardrail presets, realtime WebSocket, SSE streaming
- **Universal Model Router** — Auto-detect and translate between OpenAI, Anthropic Claude, and Google Gemini formats
- **Provider Breadth** — 10 providers: OpenAI, Anthropic, Gemini, Azure OpenAI, Bedrock, Groq, Mistral, Together AI, Cohere, Ollama
- **MCP Integration** — Register MCP servers, auto-discover tools, autonomous tool execution loop (up to 10 iterations)
- **Docker Compose** — One-command self-hosted deployment with PostgreSQL 16, Redis 7, optional Jaeger tracing
- **Prompt Management** — Create, version, deploy, and render prompt templates with variable substitution
- **A/B Experiments** — Compare models, prompts, or routing strategies with weighted traffic splitting

### 🔜 Next

- **MCP Auto-Discovery + OAuth 2.0** — Auto-initialize from URL + OAuth 2.0 token refresh
- **Helm Charts** — Production Kubernetes deployment
- **Terraform Provider** — Policy-as-code and GitOps workflows
- **Go SDK** — For Go-native agent frameworks
- **HashiCorp Vault / AWS KMS** — External master key management
- **SOC 2 Type II** — Compliance certification

---

## The One-Liner

**AIlink is what happens when you put an API gateway, a secrets vault, and a policy engine in a box — and design it specifically for AI agents.**

For developers: *"Stop managing `.env` files. Get secure agents with one line of code."*

For enterprises: *"Deploy AI agents without firing your CISO. We provide the governance layer."*
