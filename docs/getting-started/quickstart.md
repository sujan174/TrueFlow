# TrueFlow Quickstart Guide

This guide will walk you through setting up TrueFlow, configuring your first credential, creating a policy, and making your first proxied request.

## 1. Start the Stack

TrueFlow is deployed via Docker Compose. Ensure you have Docker installed.

```bash
git clone https://github.com/sujan174/trueflow.git
cd trueflow
docker compose up -d
```

This starts:
- **Dashboard** — `http://localhost:3000` (Web UI)
- **Gateway** — `http://localhost:8443` (Proxy API)
- **PostgreSQL 16** — Port 5432 (database)
- **Redis 7** — Port 6379 (cache / rate limiting)

## 2. Access the Dashboard

Navigate to [http://localhost:3000](http://localhost:3000).

You will be asked for a Dashboard Admin Key. By default (in `docker-compose.yml`), use:
```
trueflow-admin-test
```

## 3. The "Zero to Aha!" Flow

TrueFlow acts as the secure middleman between your application and AI providers (like OpenAI or Anthropic). Let’s set up your first route.

### Step A: Add a Credential
1. Go to **Vault** in the sidebar.
2. Click **Add Credential**.
3. Name it (e.g., `My OpenAI Key`).
4. Select the provider (e.g., `openai`).
5. Paste your *real* OpenAI API key (`sk-...`). 
> **Note:** This key is encrypted at rest with AES-256-GCM envelope encryption. Your application will never see this key.

### Step B: Create a Policy (Optional)
Policies define rules for the traffic passing through TrueFlow.
1. Go to **Guardrails** in the sidebar.
2. Click **Create Policy**.
3. Choose a template (e.g., **A/B Model Split**) or write a custom condition.
4. Save the policy. 

### Step C: Generate a Virtual Token
1. Go to **Agents** (Virtual Keys) in the sidebar.
2. Click **Create Token**.
3. Name it (e.g., `Dev Environment Token`).
4. Select the **Credential** you created in Step A.
5. Optionally apply the **Policy** you created in Step B.
6. Click Save and **copy the generated Token ID** (it starts with `tf_v1_...`).

## 4. Make Your First Request

Now you can use TrueFlow as a drop-in replacement for any standard AI SDK. TrueFlow intercepts your request, evaluates your policies, injects your real API key, and forwards it to the provider!

### Python Example
Install the SDK:
```bash
pip install trueflow
```

Run your code:
```python
from trueflow import TrueFlowClient

# Use the virtual token you generated in Step C
client = TrueFlowClient(api_key="tf_v1_YOUR_TOKEN_HERE")

# The TrueFlow python client acts as a drop-in wrapper around the standard OpenAI client
oai = client.openai()

response = oai.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Hello TrueFlow!"}]
)

print(response.choices[0].message.content)
```

### TypeScript Example
Install the SDK:
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

### cURL Example
```bash
curl -X POST http://localhost:8443/v1/chat/completions \
  -H "Authorization: Bearer tf_v1_YOUR_TOKEN_HERE" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Hello TrueFlow!"}]
  }'
```

## 5. Explore the Results!
Go back to your Dashboard at [http://localhost:3000](http://localhost:3000):
- **Audit Logs:** See exactly what prompt was sent, which policy approved it, and the latency.
- **Analytics:** View token usage and estimated cost charts.
- **Sessions:** Track multi-turn agent conversations with cost attribution.

## Moving to Production
When you're ready to deploy TrueFlow for real traffic:
- Change `TRUEFLOW_MASTER_KEY` and `TRUEFLOW_ADMIN_KEY` in `docker-compose.yml`.
- Set `DASHBOARD_SECRET` to a strong random value.
- See the [Deployment Guide](../deployment/docker.md) for production hosting details.
