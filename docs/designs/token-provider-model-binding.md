# Token-Level Provider/Model Binding Design

**Status**: Approved
**Date**: 2026-03-27
**Author**: Design Discussion

---

## Design Decisions (Final)

| Question | Decision |
|----------|----------|
| Model Override UI | Manual - user adds, no auto-suggest |
| Priority Assignment | Drag and drop to reorder |
| Backward Compatibility | None needed - clean break |
| Policy Scope | **Restricted to token's allowed set** - policy CANNOT expand beyond what token allows |

---

## Security Boundary: Token Config is Authoritative

**Critical Rule**: The token's configured providers/models are the **source of truth** for what's allowed.

```
Token Config: Anthropic [P1]: claude-sonnet-4

Request: gpt-4o (with OpenAI passthrough)
Result: BLOCKED - OpenAI not in token's allowed providers

Policy: dynamic_route to gpt-4o
Result: INVALID - Policy cannot route outside token's allowed set
```

This applies to:
- Direct requests with model in body
- Passthrough mode (BYOK) with provider-specific libraries
- Policy routing actions

---

## The Core Idea

Simplify token creation by letting users think in terms of **"Which providers? Which models?"** instead of upstreams, weights, and glob patterns.

---

## User Mental Model

### Simple Flow

```
Token Creation:
┌─────────────────────────────────────────┐
│ Name: Production Token                  │
│                                         │
│ Providers & Models:                     │
│ ┌─────────────────────────────────────┐ │
│ │ ☑ OpenAI                    [P1]   │ │
│ │   Models: gpt-4o, gpt-4o-mini       │ │
│ │   Credential: openai-prod           │ │
│ └─────────────────────────────────────┘ │
│ ┌─────────────────────────────────────┐ │
│ │ ☑ Anthropic                 [P2]   │ │
│ │   Models: claude-sonnet-4           │ │
│ │   Credential: anthropic-prod        │ │
│ └─────────────────────────────────────┘ │
│ ┌─────────────────────────────────────┐ │
│ │ ☑ Google Gemini              [P3]   │ │
│ │   Models: gemini-2.0-flash          │ │
│ │   Credential: gemini-prod           │ │
│ └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

---

## Data Model

### New Token Structure

```typescript
interface TokenProviderConfig {
  provider: string           // "openai", "anthropic", "gemini", etc.
  models: string[]           // ["gpt-4o", "gpt-4o-mini"]
  credential_id?: string     // Optional credential override
  priority: number           // 1 = highest (primary), 2 = first backup, etc.
  model_overrides?: Record<string, string>  // Optional: {"gpt-4o": "claude-sonnet-4"}
}

interface Token {
  // ... existing fields
  providers: TokenProviderConfig[]  // NEW: replaces upstreams for simple cases
  upstreams?: UpstreamTarget[]       // ADVANCED: for custom load balancing
}
```

---

## Default Behavior (No Policy)

### Rule 1: Model-First Priority

When a request comes in with model `gpt-4o`:

1. **Find matching provider** - Look for provider with `gpt-4o` in models list
2. **Use that provider** - Route to OpenAI (priority 1)
3. **On failure** - Find next provider with fallback model OR same model at lower priority

### Rule 2: Same Model Fallback

```
User configured:
  OpenAI [P1]: gpt-4o, gpt-4o-mini
  OpenAI [P2]: gpt-4o, gpt-4o-mini  (different credential, maybe backup account)

Request: gpt-4o
Flow: OpenAI P1 → (fail) → OpenAI P2
```

### Rule 3: Cross-Provider Fallback (with model override)

```
User configured:
  OpenAI [P1]: gpt-4o, gpt-4o-mini
  Anthropic [P2]: claude-sonnet-4
    model_overrides: {"gpt-4o": "claude-sonnet-4"}

Request: gpt-4o
Flow: OpenAI P1 → (fail) → Anthropic P2 (with model rewrite to claude-sonnet-4)
```

### Rule 4: No Matching Provider

```
User configured:
  OpenAI [P1]: gpt-4o
  Anthropic [P2]: claude-sonnet-4

Request: gemini-pro
Result: ERROR - No provider configured for model "gemini-pro"
```

---

## Policy Override Behavior

### Priority: Policy > Token Config

When a policy with routing actions is attached to a token:

```
Token Config:     OpenAI [P1], Anthropic [P2], Gemini [P3]
Policy Attached:  Dynamic Route with cost-based strategy

Result: Policy routing takes precedence
```

### Use Cases

| Scenario | Token Config | Policy | Behavior |
|----------|-------------|--------|----------|
| Simple failover | Providers + models | None | Default priority-based |
| Cost optimization | Providers + models | `lowest_cost` strategy | Policy routes to cheapest |
| Latency optimization | Providers + models | `lowest_latency` strategy | Policy routes to fastest |
| A/B testing | Providers + models | `weighted_random` | Policy does weighted split |
| Conditional routing | Providers + models | `conditional_route` | Policy evaluates conditions |

### Token Config as "Allowed List"

Even when policy is active, token config acts as an **allowlist**:

```
Token Config: OpenAI [P1]: gpt-4o, gpt-4o-mini
              Anthropic [P2]: claude-sonnet-4

Policy: lowest_cost

Request: claude-opus-4
Result: ERROR - Model not in token's allowed providers
```

This prevents policy from routing to models the user didn't authorize.

---

## Backend Implementation

### Migration from Current Design

**Current**:
```
upstreams: [
  {url, weight, priority, allowed_models, model}
]
```

**New**:
```
providers: [
  {provider, models, credential_id, priority, model_overrides}
]
```

**Conversion** (internal):
```rust
fn providers_to_upstreams(providers: &[TokenProviderConfig]) -> Vec<UpstreamTarget> {
    providers.iter().flat_map(|p| {
        let url = PROVIDER_URLS[p.provider];
        p.models.iter().map(|m| UpstreamTarget {
            url: url.clone(),
            credential_id: p.credential_id,
            weight: 100,
            priority: p.priority,
            model: p.model_overrides.get(m).cloned(),
            allowed_models: Some(vec![m.clone()]),
        })
    }).collect()
}
```

---

## UI Design

### Token Creation - Simple Mode

```
┌─────────────────────────────────────────────────────────┐
│ Create Token                                            │
├─────────────────────────────────────────────────────────┤
│ Name: [Production Token           ]                     │
│                                                         │
│ ─────────────────────────────────────────────────────── │
│ Providers & Models                                     │
│                                                         │
│ ┌───────────────────────────────────────────────────┐  │
│ │ OpenAI                                    [▲][✕] │  │
│ │ Models: [gpt-4o    ✕] [+ Add model]              │  │
│ │         [gpt-4o-mini ✕]                           │  │
│ │ Credential: [Default            ▼]                │  │
│ │ Priority: 1 (Primary)                             │  │
│ │                                                   │  │
│ │ Cross-provider failover:                          │  │
│ │ ☑ If gpt-4o fails, rewrite to: [claude-sonnet-4] │  │
│ └───────────────────────────────────────────────────┘  │
│                                                         │
│ ┌───────────────────────────────────────────────────┐  │
│ │ Anthropic                                 [▲][✕] │  │
│ │ Models: [claude-sonnet-4 ✕]                      │  │
│ │ Credential: [Default            ▼]                │  │
│ │ Priority: 2 (Backup)                              │  │
│ └───────────────────────────────────────────────────┘  │
│                                                         │
│ [+ Add Provider]                                        │
│                                                         │
│ ℹ️ Default behavior: Requests route to Priority 1,     │
│    failover to Priority 2, etc. Attach a policy for    │
│    advanced routing (cost-based, weighted, etc.)       │
│                                                         │
│                    [Cancel] [Create Token]              │
└─────────────────────────────────────────────────────────┘
```

---

## Edge Cases & Questions

### Q1: What if user adds same model to multiple providers?

```
OpenAI [P1]: gpt-4o
Azure [P2]: gpt-4o

Request: gpt-4o
```

**Answer**: Both are valid. Use priority. Route to OpenAI P1, failover to Azure P2. No model rewrite needed (same model name).

### Q2: What about model variants?

```
OpenAI [P1]: gpt-4o
  (but user requests gpt-4o-2024-11-20)
```

**Answer**: Use glob matching or exact match. Config stores patterns, not just exact models.

### Q3: How does this interact with existing `allowed_models` on tokens?

**Answer**: `providers[].models` replaces `allowed_models`. They're equivalent - just different UX.

### Q4: What about the existing `upstreams` array?

**Answer**:
- If `providers` is set: use providers (generate upstreams internally)
- If `upstreams` is set: use upstreams (advanced mode)
- Both set: error or providers wins (simpler config)

---

## Implementation Priority

### Phase 1: UI Improvements (Current)
- Keep `upstreams` backend unchanged
- Add UI that builds upstreams from provider/model selections
- Hide `allowed_models` and `model` override behind "Advanced"

### Phase 2: Backend Simplification
- Add `providers` field to tokens table
- Auto-generate `upstreams` from `providers` on request
- Migrate existing tokens

### Phase 3: Policy Integration
- Ensure policies respect `providers` as allowlist
- Add UI warning when policy routes outside allowed providers

---

## Comparison: Current vs. Proposed

| Aspect | Current | Proposed |
|--------|---------|----------|
| User configures | Upstreams, URLs, weights, patterns | Providers, models |
| Mental model | Infrastructure | Business intent |
| Failover | Manual priority setup | Automatic by priority order |
| Model matching | Glob patterns | Explicit list (or patterns) |
| Policy interaction | Independent | Token config = allowlist |
| Learning curve | High | Low |