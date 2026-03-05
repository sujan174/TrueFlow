# Quickstart

TrueFlow proxies LLM and API requests through a policy-enforcing gateway. Your agents authenticate with a **virtual token** (`tf_v1_...`). The gateway injects the real provider credential, enforces policies, and logs the request.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose installed
- Your real OpenAI (or other provider) API key
- At least 4 GB RAM available for Docker

---

## 1. Start the Stack

```bash
git clone https://github.com/trueflow/trueflow.git
cd trueflow
docker compose up -d --build
```

This starts:

| Service | URL | Description |
|---------|-----|-------------|
| Dashboard | `http://localhost:3000` | Web UI for tokens, policies, credentials, and audit logs |
| Gateway | `http://localhost:8443` | The proxy — point your agents here |
| PostgreSQL | `localhost:5432` | Database |
| Redis | `localhost:6379` | Cache, rate limiting, HITL queues |

The first build compiles the Rust gateway and Next.js dashboard. It takes 2–5 minutes. Subsequent builds use Docker layer caching and finish in under 30 seconds.

---

## 2. Log In to the Dashboard

Open [http://localhost:3000](http://localhost:3000).

The default admin key (set in `docker-compose.yml`) is:
```
trueflow-admin-test
```

---

## 3. Set Up Your First Route

### Step A — Add a Credential

1. Go to **Vault** in the sidebar.
2. Click **Add Credential**.
3. Name it (for example, `openai-prod`).
4. Select the provider (`openai`).
5. Paste your real OpenAI API key (`sk-...`).

The key is encrypted at rest with AES-256-GCM envelope encryption. It is never returned by any API endpoint.

### Step B — Create a Policy (optional)

Policies control what agents can do. Skip this step to start with no restrictions.

1. Go to **Guardrails** in the sidebar.
2. Click **Create Policy**.
3. Choose a template or write a custom condition.
4. Save the policy.

See [Policy Guide](../guides/policies.md) for full details.

### Step C — Create a Virtual Token

1. Go to **Agents** in the sidebar.
2. Click **Create Token**.
3. Name it (for example, `dev-token`).
4. Select the credential you created in Step A.
5. Optionally attach the policy from Step B.
6. Click **Save** and copy the generated token (it starts with `tf_v1_...`).

---

## 4. Make Your First Request

Use the virtual token exactly where you would use a real API key.

### Python

```bash
pip install trueflow
```

```python
from trueflow import TrueFlowClient

# Replace with your virtual token from Step C
client = TrueFlowClient(
    api_key="tf_v1_YOUR_TOKEN_HERE",
    gateway_url="http://localhost:8443",
)

oai = client.openai()  # Returns a configured openai.Client
response = oai.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Hello TrueFlow!"}],
)
print(response.choices[0].message.content)
```

`TrueFlowClient` reads `TRUEFLOW_API_KEY` and `TRUEFLOW_GATEWAY_URL` from environment variables if not passed directly.

### TypeScript

```bash
npm install @trueflow/sdk
```

```typescript
import { TrueFlowClient } from '@trueflow/sdk';

const client = new TrueFlowClient({
  apiKey: 'tf_v1_YOUR_TOKEN_HERE',
  gatewayUrl: 'http://localhost:8443',
});

const openai = client.openai();
const response = await openai.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [{ role: 'user', content: 'Hello TrueFlow!' }],
});
console.log(response.choices[0].message.content);
```

### cURL

```bash
curl -X POST http://localhost:8443/v1/chat/completions \
  -H "Authorization: Bearer tf_v1_YOUR_TOKEN_HERE" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Hello TrueFlow!"}]
  }'
```

---

## 5. Explore the Results

Back in the Dashboard:
- **Audit Logs** — the full request, policy decisions, latency, and cost
- **Analytics** — token usage and spend charts
- **Sessions** — multi-turn agent conversations with cost attribution

---

## Moving to Production

Before deploying to production, set strong values for the secrets in `docker-compose.yml`:

| Variable | Action |
|----------|--------|
| `TRUEFLOW_MASTER_KEY` | Generate a 32-byte hex key: `openssl rand -hex 32` |
| `TRUEFLOW_ADMIN_KEY` | Set to a strong random string |
| `DASHBOARD_SECRET` | Set to a strong random string |

See [Docker Deployment](../deployment/docker.md) for the full production checklist.

---

## Troubleshooting

### "Connection refused" when making requests

The gateway is not running or not listening on port 8443.

```bash
docker compose ps          # All containers should show "healthy"
curl http://localhost:8443/healthz  # Should return 200
```

If containers are restarting, check logs:
```bash
docker logs trueflow-gateway-1
```

### "401 Unauthorized"

The virtual token is invalid, revoked, or the `Authorization: Bearer` header is missing.

Confirm the token starts with `tf_v1_` and was copied in full. Use `GET /api/v1/tokens` to verify the token exists.

### "403 Forbidden"

A policy denied the request. Check the audit log for the specific policy and rule that triggered.

### "Dashboard shows Network Error"

The dashboard makes browser requests to `http://localhost:8443/api/v1`. Ensure:
1. The gateway container is running and healthy.
2. Port 8443 is not blocked by a firewall or other process.
3. `DASHBOARD_ORIGIN` in `docker-compose.yml` matches the URL you are accessing the dashboard from.

### "Build fails with out-of-memory error"

Increase Docker's memory limit to at least 4 GB in Docker Desktop → Settings → Resources.
