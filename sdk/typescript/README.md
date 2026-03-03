# @ailink/sdk

Official TypeScript SDK for the [AILink Gateway](https://github.com/ailink-dev/ailink) — the secure API proxy for AI agents.

[![npm](https://img.shields.io/npm/v/@ailink/sdk)](https://www.npmjs.com/package/@ailink/sdk)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.5+-blue)](https://www.typescriptlang.org/)
[![Node.js](https://img.shields.io/badge/Node.js-18+-green)](https://nodejs.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> **Zero dependencies** · ESM + CJS · Complete type definitions · Retry with backoff

---

## Install

```bash
npm install @ailink/sdk
```

## Quick Start

```typescript
import { AILinkClient } from "@ailink/sdk";

// Initialize with your virtual token
const client = new AILinkClient({
  apiKey: "ailink_v1_...",       // or set AILINK_API_KEY env var
  gatewayUrl: "http://localhost:8443", // or set AILINK_GATEWAY_URL
});

// Drop-in OpenAI wrapper — all requests go through the gateway
const openai = client.openai(); // requires: npm install openai
const response = await openai.chat.completions.create({
  model: "gpt-4o",
  messages: [{ role: "user", content: "Hello from AILink!" }],
});
```

## Features

| Feature | Description |
|---------|-------------|
| **OpenAI / Anthropic drop-in** | `client.openai()` / `client.anthropic()` — use familiar SDKs, routed through AILink |
| **18 resource groups** | Tokens, Credentials, Policies, Approvals (HITL), Audit, Services, Webhooks, Guardrails, Analytics, Config-as-Code, Batches, Fine-tuning, Realtime, Billing, Projects, API Keys, Experiments, Prompts |
| **10 typed errors** | Catch `RateLimitError`, `PolicyDeniedError`, `ContentBlockedError`, etc. with typed properties |
| **BYOK passthrough** | `client.withUpstreamKey("sk-...")` — bring your own key |
| **Session tracing** | `client.trace({ sessionId: "agent-run-42" })` — correlate multi-step workflows |
| **Per-request guardrails** | `client.withGuardrails(["pii_redaction"])` — attach guardrails on the fly |
| **Health check + fallback** | `client.isHealthy()` / `client.withFallback(fallbackClient)` |
| **Background health poller** | `new HealthPoller(client)` — zero-cost health checks on the hot path |
| **SSE streaming** | `streamSSE<T>(response)` — parse Server-Sent Events into typed `AsyncIterable` |
| **Retry with backoff** | Automatic exponential backoff on 429 / 5xx with configurable retries |
| **Zero dependencies** | Built on native `fetch` — works in Node 18+, Deno, Bun, Cloudflare Workers |
| **Dual ESM + CJS** | Tree-shakeable ESM with CJS fallback, full `.d.ts` declarations |

## OpenAI Drop-in

Route all OpenAI requests through the gateway with zero code changes:

```typescript
import { AILinkClient } from "@ailink/sdk";

const client = new AILinkClient({ apiKey: "ailink_v1_..." });
const openai = client.openai();

// Streaming works too
const stream = await openai.chat.completions.create({
  model: "gpt-4o",
  messages: [{ role: "user", content: "Tell me a joke" }],
  stream: true,
});

for await (const chunk of stream) {
  process.stdout.write(chunk.choices[0]?.delta?.content ?? "");
}
```

## Anthropic Drop-in

```typescript
const anthropic = client.anthropic();

const msg = await anthropic.messages.create({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Hello from AILink!" }],
});
```

## Admin Operations

Manage tokens, credentials, policies, and guardrails:

```typescript
const admin = AILinkClient.admin({ adminKey: "your-admin-key" });

// Create a virtual token
const token = await admin.tokens.create({
  name: "research-agent",
  upstreamUrl: "https://api.openai.com",
  policyIds: ["pol_abc"],
  logLevel: "redacted",
});
console.log(token.tokenId); // ailink_v1_... (shown once!)

// Create a policy
await admin.policies.create({
  name: "no-gpt4-turbo",
  rules: [{ when: { model: "gpt-4-turbo" }, then: { action: "deny" } }],
  mode: "enforce",
});

// Enable guardrails on a token
await admin.guardrails.enable("tok_abc", ["pii_redaction", "prompt_injection"]);
```

## HITL (Human-in-the-Loop) Approvals

```typescript
// List pending approvals
const pending = await admin.approvals.list({ status: "pending" });

// Approve or reject
await admin.approvals.approve(pending[0].id);
await admin.approvals.reject(pending[1].id);
```

## Session Tracing

Tag multi-step agent workflows for audit and cost tracking:

```typescript
const traced = client.trace({
  sessionId: "agent-run-42",
  properties: { env: "prod", customer: "acme" },
});

await traced.post("/v1/chat/completions", { model: "gpt-4o", messages: [...] }); // step 1
await traced.post("/v1/chat/completions", { model: "gpt-4o", messages: [...] }); // step 2
```

## Per-Request Guardrails

```typescript
import { PRESET_PII_REDACTION, PRESET_PROMPT_INJECTION } from "@ailink/sdk";

const guarded = client.withGuardrails([PRESET_PII_REDACTION, PRESET_PROMPT_INJECTION]);
await guarded.post("/v1/chat/completions", { ... });
```

## BYOK (Bring Your Own Key)

```typescript
const byok = client.withUpstreamKey("sk-my-openai-key");
await byok.post("/v1/chat/completions", { model: "gpt-4o", messages: [...] });
```

## Health Check & Fallback

```typescript
import OpenAI from "openai";

// Automatic fallback when gateway is down
const fallback = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
const openai = await client.withFallback(fallback);

// Background health poller (zero-latency health checks)
import { HealthPoller } from "@ailink/sdk";

const poller = new HealthPoller(client, { intervalMs: 10_000 });
poller.start();

if (poller.isHealthy) {
  // use gateway
} else {
  // use fallback
}
```

## Error Handling

Every gateway failure maps to a typed error with structured properties:

```typescript
import { RateLimitError, PolicyDeniedError, ContentBlockedError } from "@ailink/sdk";

try {
  await openai.chat.completions.create({ ... });
} catch (e) {
  if (e instanceof RateLimitError) {
    console.log(`Retry after ${e.retryAfter}s`);
  }
  if (e instanceof PolicyDeniedError) {
    console.log(`Blocked by policy: ${e.message}`);
  }
  if (e instanceof ContentBlockedError) {
    console.log(`Matched: ${e.matchedPatterns.join(", ")} (${e.confidence})`);
  }
}
```

### Error Hierarchy

```
AILinkError (base)
├── AuthenticationError      (401)
├── AccessDeniedError        (403)
│   ├── PolicyDeniedError    (403, code=policy_denied)
│   └── ContentBlockedError  (403, code=content_blocked)
├── NotFoundError            (404)
├── RateLimitError           (429, retryAfter)
├── ValidationError          (422)
├── PayloadTooLargeError     (413)
├── SpendCapError            (402)
└── GatewayError             (5xx)
```

## Config-as-Code

```typescript
// Export your config as YAML
const yaml = await admin.config.export({ format: "yaml" });

// Import config (upserts policies + tokens)
await admin.config.importConfig(yamlString);
```

## Prompt Management

Version, deploy, and render prompt templates with `{{variable}}` substitution:

```typescript
const admin = AILinkClient.admin({ adminKey: "..." });

// Create
const prompt = await admin.prompts.create({ name: "Support Agent", folder: "/support" });

// Version
await admin.prompts.createVersion(prompt.id, {
  model: "gpt-4o",
  messages: [
    { role: "system", content: "You help {{user_name}} with {{topic}}." },
    { role: "user",   content: "{{question}}" },
  ],
  commitMessage: "Initial version",
});

// Deploy to production
await admin.prompts.deploy(prompt.id, { version: 1, label: "production" });

// Render — cached client-side for 60s by default
const payload = await admin.prompts.render("support-agent", {
  variables: { user_name: "Alice", topic: "billing", question: "Where is my invoice?" },
  label: "production",
});
// spread into OpenAI: openai.chat.completions.create({ ...payload })

// Cache management
admin.prompts.invalidate("support-agent");  // clear one slug
admin.prompts.clearCache();                  // clear all
```

## A/B Experiments

Compare models, prompts, or routing strategies with weighted traffic splitting:

```typescript
const admin = AILinkClient.admin({ adminKey: "..." });

// Create experiment
const exp = await admin.experiments.create({
  name: "gpt4o-vs-claude",
  variants: [
    { name: "control",   weight: 50, model: "gpt-4o" },
    { name: "treatment", weight: 50, model: "claude-3-5-sonnet-20241022" },
  ],
});

// Check per-variant results
const results = await admin.experiments.results(exp.id);
console.log(results.variants);
// [
//   { variant: "control",   total_requests: 1240, avg_latency_ms: 342, error_rate: 0.01 },
//   { variant: "treatment", total_requests: 1238, avg_latency_ms: 289, error_rate: 0.00 }
// ]

// Shift traffic mid-experiment
await admin.experiments.update(exp.id, {
  variants: [
    { name: "control",   weight: 20, model: "gpt-4o" },
    { name: "treatment", weight: 80, model: "claude-3-5-sonnet-20241022" },
  ],
});

// Stop when done
await admin.experiments.stop(exp.id);
```

## SSE Streaming

```typescript
import { streamSSE } from "@ailink/sdk";

const response = await client.post("/v1/chat/completions", {
  model: "gpt-4o",
  messages: [{ role: "user", content: "Hello" }],
  stream: true,
});

for await (const chunk of streamSSE(response)) {
  process.stdout.write(chunk.choices?.[0]?.delta?.content ?? "");
}
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AILINK_API_KEY` | Virtual token for authentication | — |
| `AILINK_GATEWAY_URL` | Gateway base URL | `http://localhost:8443` |
| `AILINK_ADMIN_KEY` | Admin key for management API | — |

## Requirements

- Node.js 18+ (uses native `fetch`)
- TypeScript 5.5+ (for best type inference)
- `openai` package for `client.openai()` (optional peer dep)
- `@anthropic-ai/sdk` for `client.anthropic()` (optional peer dep)

## License

MIT
