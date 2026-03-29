# Provider-Derived URLs for Token Creation

> **Status:** Design Document
> **Last Updated:** 2026-03-30
> **Related:** [Policy Engine Reference](./policy-engine-complete-reference.md)

## Overview

TrueFlow simplifies token creation by deriving upstream URLs from provider selection. Instead of manually specifying URLs for standard providers, users simply select a provider (e.g., "openai", "anthropic") and the URL is automatically determined. Custom endpoints are supported via an optional `custom_url` field.

### Key Benefits

1. **Simpler Token Creation** - No need to memorize provider URLs
2. **Fewer Errors** - Automatic URL validation per provider
3. **Custom Endpoint Support** - Override defaults for proxies, enterprise gateways
4. **Backwards Compatible** - Existing `upstream_url` field still works

---

## Token Modes

TrueFlow supports two token modes, auto-detected from the request:

### Managed Mode

When `credential_id` is provided, the token operates in **managed mode**:
- Provider is derived from the credential
- URL is derived from the provider (or overridden via `custom_url`)
- Multiple providers supported via `upstreams` array

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Virtual Token  │────▶│    Credential    │────▶│    Provider     │
│  (tf_v1_...)    │     │  (vault-stored)  │     │  (e.g. openai)  │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                               ┌─────────────────┐
                                               │  Derived URL    │
                                               │  api.openai.com │
                                               └─────────────────┘
```

### BYOK (Bring Your Own Key) Mode

When `credential_id` is **not** provided, the token operates in **BYOK mode**:
- `provider` field is **required**
- `custom_url` is required for providers without defaults
- Single provider only (no `upstreams` array)

```
┌─────────────────┐     ┌──────────────────┐
│  Virtual Token  │────▶│    Provider      │
│  (tf_v1_...)    │     │  (user-specified)│
└─────────────────┘     └──────────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │  X-Real-Auth    │
                     │  (user's key)   │
                     └─────────────────┘
```

---

## Provider URL Mapping

### Default URLs

| Provider | Default URL | Requires Custom URL? |
|----------|-------------|---------------------|
| `openai` | `https://api.openai.com/v1` | No |
| `anthropic` | `https://api.anthropic.com/v1` | No |
| `google` / `gemini` | `https://generativelanguage.googleapis.com/v1beta` | No |
| `groq` | `https://api.groq.com/openai/v1` | No |
| `mistral` | `https://api.mistral.ai/v1` | No |
| `cohere` | `https://api.cohere.ai/v1` | No |
| `together` | `https://api.together.xyz/v1` | No |
| `openrouter` | `https://openrouter.ai/api/v1` | No |
| `ollama` | `http://localhost:11434/v1` | No |
| `azure` | — | **Yes** (region-specific) |
| `bedrock` | — | **Yes** (AWS regional) |
| `custom` | — | **Yes** |

### URL Derivation Logic

```rust
fn derive_upstream_url(provider: &str, custom_url: Option<&str>) -> Result<String, Error> {
    // 1. Custom URL takes precedence
    if let Some(url) = custom_url {
        return Ok(url.to_string());
    }

    // 2. Try default URL for provider
    match get_default_url(provider) {
        Some(url) => Ok(url.to_string()),
        None => Err(Error::CustomUrlRequired(provider)),
    }
}
```

---

## API Reference

### Create Token

**Endpoint:** `POST /api/v1/tokens`

#### Request Body (New Format)

```json
{
  "name": "Production Token",

  // Mode selection
  "credential_id": "uuid-of-credential",  // Optional: managed mode if set

  // Provider configuration
  "provider": "openai",                   // Required for BYOK, optional for managed
  "custom_url": "https://proxy.company.com/v1",  // Optional override

  // Scope controls
  "allowed_models": ["gpt-4*", "gpt-3.5*"],
  "allowed_providers": ["openai"],

  // Other fields
  "team_id": "uuid",
  "external_user_id": "customer-123",
  "purpose": "llm"
}
```

#### Request Body (Legacy Format - Still Supported)

```json
{
  "name": "Production Token",
  "credential_id": "uuid-of-credential",
  "upstream_url": "https://api.openai.com/v1",
  "allowed_models": ["gpt-4*"],
  "allowed_providers": ["openai"]
}
```

#### Response

```json
{
  "token_id": "tf_v1_abc123_tok_def456",
  "name": "Production Token",
  "message": "Use: Authorization: Bearer tf_v1_abc123_tok_def456"
}
```

### Token Object

```typescript
interface Token {
  id: string
  name: string
  project_id: string

  // Mode indicators
  credential_id: string | null   // null = BYOK mode
  provider: string | null        // Derived from credential or user-specified
  custom_url: string | null      // Custom endpoint if provided

  // Derived at creation
  upstream_url: string           // Final URL used for requests

  // Scope
  allowed_models: string[] | null
  allowed_providers: string[] | null

  // Metadata
  team_id: string | null
  external_user_id: string | null
  purpose: 'llm' | 'tool' | 'both'
  is_active: boolean
  created_at: string
}
```

---

## Usage Examples

### Python SDK

#### Managed Mode (Recommended)

```python
from trueflow import TrueFlow

client = TrueFlow(api_key="admin-key")

# Create token with credential - URL derived automatically
token = client.tokens.create(
    name="Production Token",
    credential_id="cred-uuid",
    # provider and URL derived from credential
    allowed_models=["gpt-4*", "gpt-3.5*"],
)
print(f"Token: {token.token_id}")
print(f"Will route to: {token.upstream_url}")  # https://api.openai.com/v1
```

#### Managed Mode with Custom URL

```python
# Use a custom proxy endpoint
token = client.tokens.create(
    name="Proxy Token",
    credential_id="cred-uuid",
    custom_url="https://api-proxy.company.com/v1",
    allowed_models=["gpt-4*"],
)
```

#### BYOK Mode

```python
# Bring your own key - provider required
token = client.tokens.create(
    name="BYOK Token",
    provider="openai",
    # Agents will provide their own API key via X-Real-Authorization
    allowed_models=["gpt-4*"],
)
```

#### BYOK with Custom Endpoint

```python
# Custom endpoint (e.g., Azure, private deployment)
token = client.tokens.create(
    name="Azure Token",
    provider="azure",
    custom_url="https://my-instance.openai.azure.com",
    allowed_models=["gpt-4*"],
)
```

### TypeScript/Next.js

```typescript
import { createToken, deriveUpstreamUrl } from '@/lib/api'

// Managed mode
const token = await createToken({
  name: "Production Token",
  credential_id: "cred-uuid",
  // URL derived from credential's provider
})

// BYOK mode
const token = await createToken({
  name: "BYOK Token",
  provider: "anthropic",
  // URL derived: https://api.anthropic.com/v1
})

// Custom endpoint
const token = await createToken({
  name: "Custom Endpoint",
  provider: "openai",
  custom_url: "https://openai-proxy.company.com/v1",
})

// Check if custom URL is required
const needsCustomUrl = !deriveUpstreamUrl("azure")  // true
```

### cURL

```bash
# Managed mode
curl -X POST https://gateway.example.com/api/v1/tokens \
  -H "Authorization: Bearer admin-key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Production Token",
    "credential_id": "cred-uuid",
    "allowed_models": ["gpt-4*"]
  }'

# BYOK mode
curl -X POST https://gateway.example.com/api/v1/tokens \
  -H "Authorization: Bearer admin-key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "BYOK Token",
    "provider": "anthropic",
    "allowed_models": ["claude-*"]
  }'

# Azure with custom URL
curl -X POST https://gateway.example.com/api/v1/tokens \
  -H "Authorization: Bearer admin-key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Azure Token",
    "provider": "azure",
    "custom_url": "https://my-instance.openai.azure.com",
    "credential_id": "azure-cred-uuid"
  }'
```

---

## Policy Scope Validation

### Overview

When a policy is bound to a token, TrueFlow validates that all routing targets in the policy are within the token's allowed scope. This prevents misconfiguration where a policy routes to models/providers that the token cannot access.

### How It Works

1. **Extract routing targets** from policy rules (`DynamicRoute`, `ConditionalRoute`)
2. **Derive provider from model name** using pattern matching
3. **Validate against token scope**:
   - Check provider against `allowed_providers`
   - Check model pattern against `allowed_models`
4. **Return detailed violations** if any targets are out of scope

### Provider Detection from Model

```rust
fn detect_provider_from_model(model: &str) -> String {
    let model_lower = model.to_lowercase();

    if model_lower.starts_with("gpt-") || model_lower.starts_with("o1-") || model_lower.starts_with("o3-") {
        return "openai".to_string();
    }
    if model_lower.starts_with("claude-") {
        return "anthropic".to_string();
    }
    if model_lower.starts_with("gemini-") {
        return "google".to_string();
    }
    if model_lower.starts_with("command-") {
        return "cohere".to_string();
    }
    if model_lower.starts_with("mistral-") || model_lower.starts_with("mixtral-") {
        return "mistral".to_string();
    }
    // ... more patterns

    "unknown".to_string()
}
```

### Example Validation

**Token Scope:**
```json
{
  "allowed_providers": ["openai"],
  "allowed_models": ["gpt-4*", "gpt-3.5*"]
}
```

**Policy with Routing:**
```json
{
  "rules": [{
    "then": [{
      "dynamic_route": {
        "pool": [
          { "model": "gpt-4o-mini" },
          { "model": "claude-3-haiku" }
        ]
      }
    }]
  }]
}
```

**Validation Result:**
```json
{
  "error": "policy_scope_violation",
  "message": "Policy routing targets exceed token scope",
  "violations": [{
    "model": "claude-3-haiku",
    "detected_provider": "anthropic",
    "violation_type": {
      "type": "provider_not_allowed",
      "allowed": ["openai"]
    }
  }]
}
```

### RouteTarget Simplification

With provider-derived URLs, `RouteTarget` in policies becomes simpler:

**Before (explicit URL required):**
```json
{
  "model": "gpt-4o-mini",
  "upstream_url": "https://api.openai.com/v1"
}
```

**After (URL derived from model):**
```json
{
  "model": "gpt-4o-mini"
  // upstream_url optional - derived from "openai" provider
}
```

**Custom URL still supported:**
```json
{
  "model": "gpt-4o-mini",
  "upstream_url": "https://custom-proxy.company.com/v1"
}
```

---

## Error Handling

### Token Creation Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| `provider_required_for_byok` | BYOK token without `provider` field | Add `provider` field |
| `custom_url_required` | Provider requires custom URL (azure, bedrock, custom) | Add `custom_url` field |
| `invalid_upstream_url` | URL is not valid | Fix URL format |
| `ssrf_blocked` | URL points to private/internal IP | Use public URL or enable `TRUEFLOW_ALLOW_PRIVATE_UPSTREAMS` |
| `credential_not_found` | `credential_id` doesn't exist | Use valid credential ID |

### Policy Scope Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| `provider_not_allowed` | Model's provider not in `allowed_providers` | Add provider to token scope or change policy routing |
| `model_not_allowed` | Model doesn't match `allowed_models` patterns | Update patterns or change routing target |

### Error Response Format

```json
{
  "error": "validation_error",
  "message": "Provider 'azure' requires a custom_url (no default URL available)",
  "code": "custom_url_required",
  "details": {
    "provider": "azure",
    "providers_requiring_custom_url": ["azure", "bedrock", "custom"]
  }
}
```

---

## Migration Guide

### From `upstream_url` to `provider` + `custom_url`

The old `upstream_url` field is **deprecated but still supported**. Here's how to migrate:

#### Before (Old Format)

```json
{
  "name": "My Token",
  "credential_id": "cred-uuid",
  "upstream_url": "https://api.openai.com/v1",
  "allowed_providers": ["openai"]
}
```

#### After (New Format)

```json
{
  "name": "My Token",
  "credential_id": "cred-uuid",
  "provider": "openai",
  "allowed_providers": ["openai"]
}
```

#### Migration Timeline

| Version | Status |
|---------|--------|
| v1.x | Both formats supported |
| v2.0 | `upstream_url` deprecated, warning logged |
| v3.0 | `upstream_url` removed |

### Code Migration

**Python SDK:**
```python
# Old
token = client.tokens.create(
    name="Token",
    credential_id="cred-uuid",
    upstream_url="https://api.openai.com/v1",
)

# New
token = client.tokens.create(
    name="Token",
    credential_id="cred-uuid",
    # provider derived from credential
)
```

**TypeScript:**
```typescript
// Old
const token = await createToken({
  name: "Token",
  upstream_url: "https://api.openai.com/v1",
})

// New
const token = await createToken({
  name: "Token",
  provider: "openai",
})
```

---

## Best Practices

### 1. Use Managed Mode When Possible

Managed mode provides:
- Centralized credential management
- Automatic provider/URL derivation
- Multi-provider support via `upstreams`

### 2. Leverage Scope Controls

```json
{
  "allowed_providers": ["openai", "anthropic"],
  "allowed_models": ["gpt-4*", "claude-3-*"]
}
```

This prevents:
- Accidental routing to unauthorized providers
- Cost overruns from expensive models
- Policy configuration errors

### 3. Use Custom URLs for Proxies

```python
token = client.tokens.create(
    name="Proxied OpenAI",
    credential_id="cred-uuid",
    custom_url="https://openai-proxy.company.com/v1",
)
```

Benefits:
- Centralized logging/monitoring
- Request transformation
- Rate limiting before hitting provider

### 4. BYOK for Multi-Tenant SaaS

```python
# Create token per customer
token = client.tokens.create(
    name=f"Customer {customer_id}",
    provider="openai",
    external_user_id=customer_id,
)
# Customer uses their own API key via X-Real-Authorization
```

---

## Troubleshooting

### Token routes to wrong URL

**Symptom:** Requests go to unexpected endpoint

**Check:**
1. Is `custom_url` set correctly?
2. Is `credential_id` pointing to correct credential?
3. Check credential's `provider` field

### BYOK token creation fails

**Symptom:** `provider_required_for_byok` error

**Fix:**
```json
{
  "credential_id": null,
  "provider": "openai",  // Add this!
  "custom_url": "https://..."  // If needed
}
```

### Azure/Bedrock token creation fails

**Symptom:** `custom_url_required` error

**Fix:**
```json
{
  "provider": "azure",
  "custom_url": "https://my-instance.openai.azure.com"  // Required!
}
```

### Policy binding fails with scope violation

**Symptom:** `policy_scope_violation` error when creating policy

**Fix:**
1. Check token's `allowed_providers`
2. Check policy routing targets
3. Either expand token scope or change policy routing

```json
// Option 1: Expand token scope
{
  "allowed_providers": ["openai", "anthropic"]  // Add anthropic
}

// Option 2: Change policy routing
{
  "model": "gpt-4o-mini"  // Change from claude-* to gpt-*
}
```

---

## Related Documentation

- [Policy Engine Complete Reference](./policy-engine-complete-reference.md)
- [Token Management Guide](./token-management.md)
- [Credential Vault](./credential-vault.md)
- [API Reference](./api-reference.md)

---

## Changelog

### 2026-03-30
- Initial design document
- Added provider-derived URL concept
- Defined token modes (managed vs BYOK)
- Documented policy scope validation