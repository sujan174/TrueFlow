# TrueFlow Documentation

TrueFlow is a Rust gateway that sits between your AI agents and every LLM provider. Agents authenticate with a **virtual token** (`tf_v1_...`). The gateway enforces policies, injects real credentials, streams responses, and logs everything — without exposing a single real API key.

---

## Getting Started

| Guide | Description |
|-------|-------------|
| [Quickstart](getting-started/quickstart.md) | Zero to first proxied request in 5 minutes |
| [Self-Hosting](getting-started/self-hosting.md) | Run the full stack on your machine or server |

## SDK Reference

| SDK | Install |
|-----|---------|
| [Python SDK](sdks/python.md) | `pip install trueflow` |
| [TypeScript SDK](sdks/typescript.md) | `npm install @trueflow/sdk` |

## Guides

| Guide | Description |
|-------|-------------|
| [Policy Guide](guides/policies.md) | Conditions, actions, shadow mode, spend caps, content filters |
| [Framework Integrations](guides/framework-integrations.md) | LangChain, CrewAI, LlamaIndex |
| [Supported Providers](guides/providers.md) | All 10 LLM providers — model prefixes, auth, streaming |
| [Experiments and A/B Testing](guides/experiments.md) | Traffic splitting, model comparison, canary rollouts |
| [MCP Integration](guides/mcp.md) | Model Context Protocol server integration and tool discovery |
| [Prompt Management](guides/prompts.md) | Versioned templates, deployment, server-side rendering |

## Reference

| Doc | Description |
|-----|-------------|
| [API Reference](reference/api.md) | Every Management API endpoint, request/response format, auth |
| [Architecture](reference/architecture.md) | System design, data flow, component deep dive |
| [Security](reference/security.md) | Threat model, encryption, RBAC, SSRF protection |

## Deployment

| Guide | Description |
|-------|-------------|
| [Docker](deployment/docker.md) | Docker Compose for development and single-server production |
| [Kubernetes](deployment/kubernetes.md) | K8s manifests, health probes, resource limits |

## Feature Inventory

| Doc | Description |
|-----|-------------|
| [Gateway Features](../GATEWAY_FEATURES.md) | Complete inventory of all implemented features |
