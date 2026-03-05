# TrueFlow TypeScript SDK

> Zero-dependency TypeScript client for the TrueFlow Gateway — OpenAI/Anthropic drop-in, SSE streaming, typed errors.

```bash
npm install @trueflow/sdk
```

### Requirements

- Node.js 18+ (uses native `fetch`)
- TypeScript 5.5+ (for best type inference)
- `openai` package for `client.openai()` (optional peer dep)
- `@anthropic-ai/sdk` for `client.anthropic()` (optional peer dep)

---

## Quick Start — OpenAI Drop-In

```typescript
import { TrueFlowClient } from '@trueflow/sdk';

const client = new TrueFlowClient({
  apiKey: 'tf_v1_...',
  gatewayUrl: 'http://localhost:8443',
});

// Get a configured OpenAI client — works with openai@4+
const openai = client.openai();

const response = await openai.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'Hello from TypeScript!' }],
});
console.log(response.choices[0].message.content);
```

---

## Streaming

```typescript
const stream = await openai.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'Write a haiku' }],
  stream: true,
});

for await (const chunk of stream) {
  process.stdout.write(chunk.choices[0]?.delta?.content ?? '');
}
```

---

## Anthropic Drop-In

```typescript
const anthropic = client.anthropic();
const message = await anthropic.messages.create({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello Claude via TrueFlow!' }],
});
```

---

## Action Gateway (API Proxy)

```typescript
const client = new TrueFlowClient({
  apiKey: 'tf_v1_proj_abc123_tok_def456',
  gatewayUrl: 'http://localhost:8443',
  agentName: 'billing-agent',
});

// GET request
const customers = await client.get('/v1/customers');

// POST request
const charge = await client.post('/v1/charges', {
  body: { amount: 5000, currency: 'usd' },
});

// With HITL approval
const result = await client.post('/v1/charges', {
  body: { amount: 50000, currency: 'usd' },
  waitForApproval: true,
  approvalTimeout: 300,
  idempotencyKey: 'charge-order-12345',
});
```

---

## Admin Mode (Management API)

```typescript
const admin = TrueFlowClient.admin({
  adminKey: 'trueflow-admin-test',
  gatewayUrl: 'http://localhost:8443',
});

// ── Tokens ──
const tokens = await admin.tokens.list();
const newToken = await admin.tokens.create({
  name: 'prod-agent',
  credentialId: 'cred-uuid',
  upstreamUrl: 'https://api.openai.com',
  circuitBreaker: { enabled: true, failureThreshold: 5 },
});
await admin.tokens.delete(newToken.tokenId);

// ── Credentials ──
const creds = await admin.credentials.list();
const newCred = await admin.credentials.create({
  name: 'openai-prod',
  provider: 'openai',
  secret: 'sk-...',
  injectionMode: 'header',
  injectionHeader: 'Authorization',
});

// ── Policies ──
const policies = await admin.policies.list();
const policy = await admin.policies.create({
  name: 'rate-limit-60rpm',
  mode: 'enforce',
  rules: [
    { when: { always: true }, then: { action: 'rate_limit', window: '1m', max_requests: 60 } },
  ],
});

// ── Guardrails ──
await admin.guardrails.enable('tf_v1_...', ['pii_redaction', 'prompt_injection']);
const status = await admin.guardrails.status('tf_v1_...');
await admin.guardrails.disable('tf_v1_...');

// ── Analytics ──
const summary = await admin.analytics.summary();
const timeseries = await admin.analytics.timeseries();

// ── Config-as-Code ──
const yamlConfig = await admin.config.export();
await admin.config.importYaml(yamlConfig);
```

---

## Health Polling & Fallback

```typescript
import { TrueFlowClient, HealthPoller } from '@trueflow/sdk';
import OpenAI from 'openai';

const client = new TrueFlowClient({ apiKey: 'tf_v1_...' });
const fallback = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });

// One-shot health check
if (await client.isHealthy()) {
  const openai = client.openai();
} else {
  // use fallback
}

// Background polling (long-running services)
const poller = new HealthPoller(client, { interval: 15 });
poller.start();

const openai = poller.isHealthy ? client.openai() : fallback;
poller.stop();
```

---

## Per-Request Guardrails

```typescript
import { PRESET_PII_REDACTION, PRESET_PROMPT_INJECTION } from '@trueflow/sdk';

const guarded = client.withGuardrails([PRESET_PII_REDACTION, PRESET_PROMPT_INJECTION]);
await guarded.post('/v1/chat/completions', { ... });
```

---

## BYOK (Bring Your Own Key)

```typescript
const byok = client.withUpstreamKey('sk-my-openai-key');
await byok.post('/v1/chat/completions', { model: 'gpt-4o', messages: [...] });
```

---

## Session Tracing

```typescript
const traced = client.trace({
  sessionId: 'agent-run-42',
  properties: { env: 'prod', customer: 'acme' },
});

await traced.post('/v1/chat/completions', { model: 'gpt-4o', messages: [...] });
```

---

## Error Handling

Every gateway failure maps to a typed error:

```typescript
import {
  TrueFlowError,
  PolicyDeniedError,
  RateLimitError,
  SpendCapError,
  ContentBlockedError,
  AuthenticationError,
} from '@trueflow/sdk';

try {
  const response = await client.post('/v1/charges', { body: { amount: 5000 } });
} catch (error) {
  if (error instanceof PolicyDeniedError) {
    console.error(`Blocked: ${error.policyName} — ${error.reason}`);
  } else if (error instanceof RateLimitError) {
    console.error(`Rate limited — retry after ${error.retryAfter}s`);
  } else if (error instanceof SpendCapError) {
    console.error('Spend cap exceeded');
  } else if (error instanceof ContentBlockedError) {
    console.error('Content blocked by guardrail');
  }
}
```

### Error Hierarchy

```
TrueFlowError (base)
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

---

## Response Caching

TrueFlow caches LLM responses semantically. By default, duplicate requests return the cached response.

```typescript
// Bypass cache for this request. Requires `cache:bypass` scope on the token.
const response = await client.post('/v1/chat/completions', {
  body: { ... },
  headers: { 'x-trueflow-no-cache': 'true' }
});
```

Cache hits are indicated by `X-TrueFlow-Cache: HIT` in the response headers.

---

## Realtime API (WebSocket)

```typescript
const session = await client.realtime.connect('gpt-4o-realtime-preview');
await session.send({ type: 'session.update', /* ... */ });
const event = await session.recv();
await session.close();
```

---

## SSE Streaming

```typescript
import { streamSSE } from '@trueflow/sdk';

const response = await client.post('/v1/chat/completions', {
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'Hello' }],
  stream: true,
});

for await (const chunk of streamSSE(response)) {
  process.stdout.write(chunk.choices?.[0]?.delta?.content ?? '');
}
```

---

## Prompt Management

Create, version, deploy, and render prompt templates. Rendered prompts are cached client-side (default 60s) to reduce latency.

```typescript
const admin = TrueFlowClient.admin({ adminKey: 'trueflow-admin-...' });

// Create and version
const prompt = await admin.prompts.create({ name: 'Support Agent', folder: '/support' });
await admin.prompts.createVersion(prompt.id, {
  model: 'gpt-4o',
  messages: [
    { role: 'system', content: 'You help {{user_name}} with {{topic}}.' },
    { role: 'user',   content: '{{question}}' },
  ],
  temperature: 0.7,
  commitMessage: 'Initial version',
});

// Deploy to production
await admin.prompts.deploy(prompt.id, { version: 1, label: 'production' });

// Render — cached for 60s by default
const payload = await admin.prompts.render('support-agent', {
  variables: { user_name: 'Alice', topic: 'billing', question: 'Where is my invoice?' },
  label: 'production',
});
// openai.chat.completions.create({ ...payload })

// Cache control
admin.prompts.invalidate('support-agent'); // clear one slug
admin.prompts.clearCache();                 // clear all
```

---

## A/B Experiments

Compare models, prompts, or routing strategies with weighted traffic splitting:

```typescript
const admin = TrueFlowClient.admin({ adminKey: 'trueflow-admin-...' });

// Create
const exp = await admin.experiments.create({
  name: 'gpt4o-vs-claude',
  variants: [
    { name: 'control',   weight: 50, model: 'gpt-4o' },
    { name: 'treatment', weight: 50, model: 'claude-3-5-sonnet-20241022' },
  ],
});

// Results
const results = await admin.experiments.results(exp.id);
// results.variants → per-variant metrics (requests, latency, cost, error_rate)

// Adjust weights mid-experiment
await admin.experiments.update(exp.id, {
  variants: [
    { name: 'control',   weight: 20, model: 'gpt-4o' },
    { name: 'treatment', weight: 80, model: 'claude-3-5-sonnet-20241022' },
  ],
});

// Stop
await admin.experiments.stop(exp.id);
```

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TRUEFLOW_API_KEY` | Virtual token for authentication | — |
| `TRUEFLOW_GATEWAY_URL` | Gateway base URL | `http://localhost:8443` |
| `TRUEFLOW_ADMIN_KEY` | Admin key for management API | — |
