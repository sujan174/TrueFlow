# Authentication Modes

TrueFlow Gateway supports two authentication modes for different use cases. This guide explains how to use each mode with popular SDKs.

## Overview

| Mode | Use Case | API Key Management |
|------|----------|-------------------|
| **BYOK (Passthrough)** | Single provider, user manages keys | User keeps control of keys |
| **Managed** | Multi-provider, TrueFlow manages keys | Keys stored encrypted in gateway |

---

## BYOK (Bring Your Own Key) Mode

In BYOK mode, you keep control of your API keys while using TrueFlow for policy enforcement and observability. Your keys are passed through to the upstream provider.

### When to Use

- You want to keep control of your API keys
- You use a single provider (one token = one provider)
- You want quick onboarding without storing keys

### How It Works

1. Create a token with `credential_id = NULL` and set `upstream_url` to your target provider
2. Send your real API key in the standard auth header
3. Add `X-TrueFlow-Auth` header with your virtual token

### OpenAI SDK (BYOK)

```python
from openai import OpenAI

client = OpenAI(
    api_key="sk-proj-your-real-openai-key",  # Your real OpenAI key
    base_url="https://your-gateway.com/v1",   # Point to TrueFlow
    default_headers={"X-TrueFlow-Auth": "tf_v1_xxx"}  # Virtual token
)

# All requests go to OpenAI with your key
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello"}]
)
```

### Anthropic SDK (BYOK)

```python
from anthropic import Anthropic

client = Anthropic(
    api_key="sk-ant-your-real-anthropic-key",  # Your real Anthropic key
    base_url="https://your-gateway.com/v1",     # Point to TrueFlow
    default_headers={"X-TrueFlow-Auth": "tf_v1_xxx"}  # Virtual token
)

# All requests go to Anthropic with your key
response = client.messages.create(
    model="claude-3-opus-20240229",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello"}]
)
```

### Supported Providers for BYOK

| Provider | SDK | Auth Header | Works? |
|----------|-----|-------------|--------|
| OpenAI | OpenAI SDK | `Authorization: Bearer` | ✅ |
| Anthropic | Anthropic SDK | `x-api-key` | ✅ |
| Groq | OpenAI SDK | `Authorization: Bearer` | ✅ |
| Mistral | OpenAI SDK | `Authorization: Bearer` | ✅ |
| Together AI | OpenAI SDK | `Authorization: Bearer` | ✅ |
| Cohere | OpenAI SDK | `Authorization: Bearer` | ✅ |
| Ollama | OpenAI SDK | `Authorization: Bearer` | ✅ |
| OpenRouter | OpenAI SDK | `Authorization: Bearer` | ✅ |

### BYOK Limitations

- **One token = one provider**: The token's `upstream_url` is fixed
- **No multi-provider routing**: Cannot use one token for multiple providers
- **No Gemini/AWS Bedrock**: These require special auth (query params, SigV4)

### Token Setup for BYOK

In the dashboard:

1. Go to **Tokens** → **Create Token**
2. Set **Name** for identification
3. Leave **Credential** as "Default" (no credential selected)
4. Set **Upstream URL** to your target (e.g., `https://api.openai.com/v1`)
5. Click **Create Token**
6. Copy the token ID for `X-TrueFlow-Auth` header

---

## Managed Mode

In Managed mode, TrueFlow stores your API keys securely and handles provider-specific authentication automatically. One virtual token works with any provider.

### When to Use

- You want one token that works with all providers
- You want automatic auth header translation (Bearer → x-api-key, etc.)
- You want to use Gemini or AWS Bedrock
- You want credential rotation without changing tokens

### How It Works

1. Store your API keys in TrueFlow dashboard (Credentials)
2. Create a token linked to a stored credential
3. Use only the virtual token in your code

### Setup Steps

#### 1. Add Credentials

In the dashboard:

1. Go to **Credentials** → **Add Credential**
2. Select provider (OpenAI, Anthropic, etc.)
3. Enter your API key
4. Keys are encrypted with AES-256-GCM

#### 2. Create Token

1. Go to **Tokens** → **Create Token**
2. Select a **Credential** from the dropdown
3. The token will use that credential's key

### OpenAI SDK (Managed)

```python
from openai import OpenAI

# Just use the virtual token - TrueFlow handles the rest
client = OpenAI(
    api_key="tf_v1_xxx",  # Virtual token only
    base_url="https://your-gateway.com/v1"
)

# Works with any model - TrueFlow auto-detects provider
response = client.chat.completions.create(
    model="gpt-4o",  # → Uses stored OpenAI key
    messages=[{"role": "user", "content": "Hello"}]
)

response = client.chat.completions.create(
    model="claude-3-opus-20240229",  # → Uses stored Anthropic key
    messages=[{"role": "user", "content": "Hello"}]
)
```

### Provider Support for Managed Mode

| Provider | Auth Handling |
|----------|---------------|
| OpenAI | `Authorization: Bearer <key>` |
| Anthropic | `x-api-key: <key>` (auto-translated) |
| Gemini | `?key=<key>` query param (auto-injected) |
| AWS Bedrock | AWS SigV4 signing (auto-applied) |
| Azure OpenAI | `api-key` header |
| Groq, Mistral, Together, Cohere | `Authorization: Bearer <key>` |

---

## Comparison Table

| Feature | BYOK Mode | Managed Mode |
|---------|-----------|--------------|
| **API key control** | User keeps keys | TrueFlow stores keys |
| **Multi-provider** | No (1 token = 1 provider) | Yes (1 token = all providers) |
| **Setup effort** | Minimal | Add credentials first |
| **Key rotation** | User manages | Re-create credential |
| **Policy enforcement** | ✅ Yes | ✅ Yes |
| **Observability** | ✅ Yes | ✅ Yes |
| **Cost tracking** | ✅ Yes | ✅ Yes |
| **Gemini/Bedrock** | ❌ No | ✅ Yes |

---

## Features Available Without TrueFlow SDK

Users not using the TrueFlow SDK still get core functionality:

### ✅ Available

| Feature | How to Use |
|---------|-----------|
| Policy enforcement | Automatic with virtual token |
| Rate limiting | Configured on token/policy |
| Spend caps | Configured on token |
| Audit logging | Automatic |
| Cost tracking | Automatic |
| External user tracking | Set `external_user_id` on token |

### ❌ Requires SDK

| Feature | Why SDK Required |
|---------|-----------------|
| Session tracing | Requires `X-Session-Id` header |
| Distributed tracing | Requires `X-Parent-Span-Id` header |
| Agent name tagging | Requires `X-TrueFlow-Agent-Name` header |
| Custom properties | Requires SDK `trace()` context manager |

---

## Auth Header Reference

### Headers Sent by SDKs

| SDK | Header Sent |
|-----|-------------|
| OpenAI SDK | `Authorization: Bearer <api_key>` |
| Anthropic SDK | `x-api-key: <api_key>` |

### Headers Accepted by TrueFlow

| Header | Purpose |
|--------|---------|
| `Authorization` | Virtual token (`tf_v1_xxx`) or real API key (BYOK) |
| `X-TrueFlow-Auth` | Virtual token for BYOK mode |
| `X-Real-Authorization` | Explicit passthrough header (legacy) |
| `x-api-key` | Anthropic SDK's API key (BYOK) |

### Header Priority for Token Extraction

1. `X-TrueFlow-Auth` (recommended for BYOK)
2. `Authorization` with `Bearer tf_v1_xxx`

### Header Priority for Real API Key (Passthrough)

1. `X-Real-Authorization`
2. `X-Upstream-Authorization`
3. `Authorization` (if not a virtual token)
4. `x-api-key` (Anthropic SDK)

---

## Security Notes

- All credential-bearing headers are **redacted in audit logs**
- Credentials are **encrypted with AES-256-GCM** envelope encryption
- The gateway uses an **allowlist** for header forwarding (prevents credential leakage)
- Virtual tokens are validated before any upstream request

---

## Quick Reference

### BYOK Mode Checklist

- [ ] Create token with `credential_id = NULL`
- [ ] Set `upstream_url` to target provider
- [ ] Use real API key in SDK
- [ ] Add `X-TrueFlow-Auth` header with virtual token

### Managed Mode Checklist

- [ ] Add credential(s) in dashboard
- [ ] Create token linked to a credential
- [ ] Use virtual token as `api_key` in SDK
- [ ] Use any model - routing is automatic