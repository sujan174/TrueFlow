---
name: BYOK Passthrough Simplification
description: Simplify BYOK mode with single header and single provider constraint
type: project
---

# BYOK Passthrough Simplification

## Problem Statement

The current BYOK/passthrough implementation has complexity from supporting multiple ways to pass real API keys:
- `X-Real-Authorization`
- `X-Upstream-Authorization`
- `Authorization` (if not a virtual token)
- `x-api-key` (Anthropic SDK)

This creates confusion and implementation complexity.

## Proposed Solution

### Token Modes

| Mode | credential_id | Providers | Key Storage | Real Key Header |
|------|--------------|-----------|-------------|-----------------|
| **Managed** | UUID | Multiple | Vault (internal/external KMS) | N/A |
| **Passthrough (BYOK)** | NULL | Single | User's system | `X-TF-Real-Auth` |

### Request Flow

**Managed Mode:**
```
Authorization: Bearer tf_v1_xxx

Gateway:
1. Extract virtual token from Authorization
2. Look up credential_id from token
3. Retrieve credential from vault
4. Inject key and forward to provider
```

**Passthrough Mode:**
```
X-TrueFlow-Auth: Bearer tf_v1_xxx
X-TF-Real-Auth: Bearer sk-real-api-key

Gateway:
1. Extract virtual token from X-TrueFlow-Auth
2. Verify token exists and credential_id == NULL
3. Extract real key from X-TF-Real-Auth
4. Validate real key is present (required for passthrough)
5. Forward real key to upstream_url provider
```

### Token Creation (Dashboard)

```
┌─────────────────────────────────────────────────────────────────┐
│ Create Token                                                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│ Mode: ┌─────────────────┐  ┌────────────────────────────────┐   │
│       │ ● Managed       │  │ ○ Passthrough (BYOK)           │   │
│       │   Multiple      │  │   Single provider              │   │
│       │   providers     │  │   You provide the API key      │   │
│       │   Keys stored   │  │   with each request            │   │
│       │   in vault      │  │                                │   │
│       └─────────────────┘  └────────────────────────────────┘   │
│                                                                  │
│ If Passthrough selected:                                         │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │ ⚠️ Passthrough Mode                                       │  │
│   │ • Select ONE provider                                     │  │
│   │ • Send your API key in X-TF-Real-Auth header              │  │
│   │ • Example: X-TF-Real-Auth: Bearer sk-xxx                  │  │
│   └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│   Provider: [OpenAI ▼]  ← Single select dropdown                │
│                                                                  │
│ If Managed selected:                                             │
│   • Multi-provider selection (current behavior)                  │
│   • Credential dropdown appears                                  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### SDK Usage

```python
from trueflow import TrueFlowClient

# Passthrough mode
client = TrueFlowClient.byok(
    virtual_token="tf_v1_xxx",
    real_api_key="sk-my-openai-key",
    base_url="https://gateway.example.com"
)

# SDK automatically sends:
#   X-TrueFlow-Auth: Bearer tf_v1_xxx
#   X-TF-Real-Auth: Bearer sk-my-openai-key

# Managed mode
client = TrueFlowClient(
    api_key="tf_v1_xxx",
    base_url="https://gateway.example.com"
)

# SDK sends:
#   Authorization: Bearer tf_v1_xxx
```

### Gateway Changes

1. **Token Extraction** (`src/proxy/handler/core.rs`):
   - Always check `X-TrueFlow-Auth` first for virtual token
   - Fallback to `Authorization` header for managed mode

2. **Passthrough Validation**:
   - If `credential_id == NULL`, require `X-TF-Real-Auth` header
   - Return clear error if missing: "Passthrough token requires X-TF-Real-Auth header"

3. **Single Provider Enforcement**:
   - Validate token creation: passthrough mode allows only one provider
   - Validate policy routing: dynamic routes must not reference other providers

### Dashboard Changes

1. **Token Creation Form**:
   - Add TokenModeSelector component (managed/passthrough)
   - Single-provider dropdown for passthrough mode
   - Multi-provider selector for managed mode

2. **Token List**:
   - Show BYOK badge for passthrough tokens

3. **Token Detail**:
   - Show upstream URL for passthrough tokens
   - Show provider info for managed tokens

### Python SDK Changes

1. **`TrueFlowClient.byok()` method**:
   - Send `X-TrueFlow-Auth` header with virtual token
   - Send `X-TF-Real-Auth` header with real API key

2. **Documentation**:
   - Clear examples for BYOK usage
   - Migration guide from old headers

## Trade-offs

### Pros
- **Simplicity**: One standard way to pass real keys
- **Clarity**: Explicit separation of virtual token and real key
- **Compatibility**: Matches LiteLLM/Portkey approach
- **Security**: Clear audit trail of which header contains real key

### Cons
- **Breaking change**: Existing BYOK users need to update headers
- **Single provider**: BYOK users cannot use multi-provider routing
- **SDK required**: For best experience, users should use TrueFlow SDK

## Migration Plan

1. **Phase 1**: Add new header support alongside existing headers
2. **Phase 2**: Deprecation warning for old headers
3. **Phase 3**: Remove old header support

## Implementation Checklist

- [ ] Gateway: Update `extract_virtual_token()` to check `X-TrueFlow-Auth` first
- [ ] Gateway: Add `X-TF-Real-Auth` header extraction for passthrough
- [ ] Gateway: Validate single provider for passthrough tokens
- [ ] Gateway: Return clear error messages for missing headers
- [ ] Dashboard: Update token creation form with mode selector
- [ ] Dashboard: Single-provider dropdown for passthrough
- [ ] Dashboard: BYOK badge in token list
- [ ] SDK: Update `TrueFlowClient.byok()` to use new headers
- [ ] Tests: Add integration tests for new flow
- [ ] Docs: Update authentication-modes.md

## Why: User Preference

Users prefer keeping credentials in their own KMS (AWS KMS, HashiCorp Vault) for compliance and security reasons. BYOK mode allows them to use TrueFlow's policy engine and observability while maintaining control of their API keys.

## How to Apply

This design simplifies the mental model for BYOK users while maintaining the powerful multi-provider capabilities for managed mode users. The single-header approach reduces implementation complexity and matches industry standards (LiteLLM, Portkey).