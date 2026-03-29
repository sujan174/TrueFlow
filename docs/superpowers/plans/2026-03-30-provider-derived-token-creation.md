# Provider-Derived Token Creation & Policy Scope Validation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify token creation by deriving upstream URLs from provider selection, and ensure policy routing targets are validated against token scope at binding time.

**Architecture:** Provider-based URL derivation with custom_url override for non-standard endpoints. Auto-detect token mode (managed vs BYOK) from credential_id presence. Policy scope validation uses derived provider from model names.

**Tech Stack:** Rust (Axum), TypeScript (Next.js), Python (SDK)

---

## Part 1: Provider-to-URL Mapping

### Provider Defaults

| Provider | Default URL | Notes |
|----------|-------------|-------|
| openai | `https://api.openai.com/v1` | Standard OpenAI API |
| anthropic | `https://api.anthropic.com/v1` | Claude models |
| google | `https://generativelanguage.googleapis.com/v1beta` | Gemini models |
| groq | `https://api.groq.com/openai/v1` | Fast inference |
| mistral | `https://api.mistral.ai/v1` | Mistral models |
| cohere | `https://api.cohere.ai/v1` | Command models |
| together | `https://api.together.xyz/v1` | Open-source models |
| openrouter | `https://openrouter.ai/api/v1` | Unified API |
| azure | *(custom_url required)* | Region-specific |
| bedrock | *(derived from credential)* | AWS regional |
| ollama | `http://localhost:11434/v1` | Local inference |
| custom | *(custom_url required)* | User-defined |

---

## Part 2: Token Creation API Redesign

### Task 1: Update Backend CreateTokenRequest DTO

**Files:**
- Modify: `gateway/src/api/handlers/dtos.rs:1-60`

- [ ] **Step 1: Add new fields to CreateTokenRequest**

Replace the current `upstream_url` with `provider` and `custom_url`:

```rust
#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
    /// If Some → Managed mode (provider from credential)
    /// If None → BYOK mode (provider required)
    pub credential_id: Option<Uuid>,
    /// Provider name: "openai", "anthropic", "custom", etc.
    /// Required for BYOK, optional for managed (derived from credential)
    pub provider: Option<String>,
    /// Custom endpoint URL (for non-standard providers or BYOK)
    /// If None, uses standard provider URL
    pub custom_url: Option<String>,
    pub project_id: Option<Uuid>,
    // ... keep all other existing fields
    pub allowed_models: Option<serde_json::Value>,
    pub allowed_providers: Option<Vec<String>>,
    // ... rest of fields unchanged
}
```

- [ ] **Step 2: Keep upstream_url field for backwards compatibility**

```rust
    /// DEPRECATED: Use `provider` + `custom_url` instead.
    /// If provided, takes precedence over derived URL for backwards compatibility.
    #[serde(default)]
    pub upstream_url: Option<String>,
```

- [ ] **Step 3: Commit the DTO changes**

```bash
git add gateway/src/api/handlers/dtos.rs
git commit -m "feat(tokens): add provider and custom_url fields to CreateTokenRequest"
```

---

### Task 2: Create Provider URL Derivation Module

**Files:**
- Create: `gateway/src/utils/provider_url.rs`
- Modify: `gateway/src/utils/mod.rs`

- [ ] **Step 1: Create the provider URL mapping module**

```rust
//! Provider-to-default-URL mapping for token creation.

/// Default base URLs for each provider.
/// Returns None for providers that require custom URLs (azure, bedrock, custom).
pub fn get_provider_default_url(provider: &str) -> Option<&'static str> {
    match provider.to_lowercase().as_str() {
        "openai" => Some("https://api.openai.com/v1"),
        "anthropic" => Some("https://api.anthropic.com/v1"),
        "google" | "gemini" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "groq" => Some("https://api.groq.com/openai/v1"),
        "mistral" => Some("https://api.mistral.ai/v1"),
        "cohere" => Some("https://api.cohere.ai/v1"),
        "together" | "togetherai" => Some("https://api.together.xyz/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "ollama" => Some("http://localhost:11434/v1"),
        // These require custom URLs
        "azure" | "bedrock" | "custom" => None,
        _ => None,
    }
}

/// Derive the upstream URL from provider and optional custom URL.
/// Returns an error if the provider requires a custom URL but none was provided.
pub fn derive_upstream_url(provider: &str, custom_url: Option<&str>) -> Result<String, String> {
    // Custom URL takes precedence
    if let Some(url) = custom_url {
        return Ok(url.to_string());
    }

    // Try default URL
    if let Some(default) = get_provider_default_url(provider) {
        return Ok(default.to_string());
    }

    // Provider requires custom URL
    Err(format!(
        "Provider '{}' requires a custom_url (no default URL available)",
        provider
    ))
}

/// Check if a provider has a default URL.
pub fn provider_has_default_url(provider: &str) -> bool {
    get_provider_default_url(provider).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_default_url() {
        assert_eq!(
            get_provider_default_url("openai"),
            Some("https://api.openai.com/v1")
        );
    }

    #[test]
    fn test_custom_url_override() {
        let result = derive_upstream_url("openai", Some("https://custom.openai.com/v1"));
        assert_eq!(result.unwrap(), "https://custom.openai.com/v1");
    }

    #[test]
    fn test_azure_requires_custom_url() {
        let result = derive_upstream_url("azure", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_azure_with_custom_url() {
        let result = derive_upstream_url("azure", Some("https://my-instance.openai.azure.com"));
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Add module to utils/mod.rs**

```rust
pub mod provider_url;
```

- [ ] **Step 3: Commit the new module**

```bash
git add gateway/src/utils/provider_url.rs gateway/src/utils/mod.rs
git commit -m "feat: add provider URL derivation module"
```

---

### Task 3: Update Token Creation Handler

**Files:**
- Modify: `gateway/src/api/handlers/tokens.rs:65-215`

- [ ] **Step 1: Add URL derivation logic to create_token handler**

After the auth check, add the provider/URL derivation:

```rust
/// POST /api/v1/tokens — create a new virtual token
pub async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreateTokenResponse>), AppError> {
    auth.require_role("admin")?;
    auth.require_scope("tokens:write")?;
    let project_id = payload.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // ── Determine token mode and derive upstream URL ─────────────────────
    let (provider, upstream_url) = if let Some(cred_id) = payload.credential_id {
        // Managed mode: get provider from credential
        let cred = state.db.get_credential_by_id(&cred_id).await?
            .ok_or_else(|| AppError::NotFound("Credential not found".into()))?;
        let provider = cred.provider.clone();

        // Derive URL
        let url = if let Some(ref custom_url) = payload.custom_url {
            custom_url.clone()
        } else if let Some(ref url) = payload.upstream_url {
            // Backwards compatibility
            url.clone()
        } else {
            crate::utils::provider_url::derive_upstream_url(&provider, None)
                .map_err(|e| AppError::ValidationError { message: e })?
        };

        (Some(provider), url)
    } else {
        // BYOK mode: provider is required
        let provider = payload.provider.clone()
            .ok_or_else(|| AppError::ValidationError {
                message: "provider is required for BYOK tokens (when credential_id is not provided)".into()
            })?;

        let url = if let Some(ref custom_url) = payload.custom_url {
            custom_url.clone()
        } else if let Some(ref url) = payload.upstream_url {
            // Backwards compatibility
            url.clone()
        } else {
            crate::utils::provider_url::derive_upstream_url(&provider, None)
                .map_err(|e| AppError::ValidationError { message: e })?
        };

        (Some(provider), url)
    };

    // Validate the derived URL
    let url = reqwest::Url::parse(&upstream_url).map_err(|_| {
        AppError::ValidationError {
            message: format!("Invalid upstream URL: {}", upstream_url)
        }
    })?;
    // ... rest of validation
```

- [ ] **Step 2: Update the NewToken struct creation**

```rust
    let new_token = crate::store::postgres::NewToken {
        id: token_id.clone(),
        project_id,
        name: payload.name.clone(),
        credential_id: payload.credential_id,
        upstream_url,
        // ... rest of fields
    };
```

- [ ] **Step 3: Commit the handler changes**

```bash
git add gateway/src/api/handlers/tokens.rs
git commit -m "feat(tokens): derive upstream URL from provider in create_token"
```

---

### Task 4: Update Frontend Token Types

**Files:**
- Modify: `dashboard/src/lib/types/token.ts`

- [ ] **Step 1: Update CreateTokenRequest type**

```typescript
export interface CreateTokenRequest {
  name: string
  credential_id?: string  // If set = managed mode
  provider?: string       // Required for BYOK, optional for managed
  custom_url?: string     // Override default provider URL
  project_id?: string
  // DEPRECATED: Use provider + custom_url
  upstream_url?: string   // Backwards compatibility
  allowed_models?: string[]
  allowed_providers?: string[]
  team_id?: string
  external_user_id?: string
  tags?: string[]
  purpose?: 'llm' | 'tool' | 'both'
  metadata?: Record<string, unknown>
}
```

- [ ] **Step 2: Add helper function for URL derivation**

```typescript
/**
 * Provider default URLs.
 */
export const PROVIDER_DEFAULT_URLS: Record<string, string> = {
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com/v1',
  google: 'https://generativelanguage.googleapis.com/v1beta',
  groq: 'https://api.groq.com/openai/v1',
  mistral: 'https://api.mistral.ai/v1',
  cohere: 'https://api.cohere.ai/v1',
  together: 'https://api.together.xyz/v1',
  openrouter: 'https://openrouter.ai/api/v1',
  ollama: 'http://localhost:11434/v1',
  // These require custom URLs
  azure: '',
  bedrock: '',
  custom: '',
}

/**
 * Derive upstream URL from provider and optional custom URL.
 */
export function deriveUpstreamUrl(provider: string, customUrl?: string): string | null {
  if (customUrl) return customUrl
  return PROVIDER_DEFAULT_URLS[provider.toLowerCase()] || null
}
```

- [ ] **Step 3: Commit the type changes**

```bash
git add dashboard/src/lib/types/token.ts
git commit -m "feat(tokens): add provider and custom_url to frontend token types"
```

---

### Task 5: Update Frontend Token Creation Form

**Files:**
- Modify: `dashboard/src/app/(dashboard)/tokens/page.tsx:124-300`

- [ ] **Step 1: Simplify the token creation modal**

Replace the complex upstream URL management with provider-based selection:

```tsx
// In CreateTokenModal, simplify state:
const [provider, setProvider] = useState<string>("openai")
const [customUrl, setCustomUrl] = useState<string>("")

// Derive URL from provider
const derivedUrl = deriveUpstreamUrl(provider, customUrl || undefined)

// In handleSubmit:
const response = await createToken({
  name,
  credential_id: tokenMode === "managed" ? selectedCredentialId : undefined,
  provider: tokenMode === "byok" ? provider : undefined,
  custom_url: customUrl || undefined,
  allowed_providers: [provider],
  // ... other fields
})
```

- [ ] **Step 2: Update the form UI**

Show URL derivation hint:

```tsx
<div className="space-y-2">
  <Label>Provider</Label>
  <Select value={provider} onValueChange={setProvider}>
    {PROVIDER_PRESETS.map(p => (
      <SelectItem key={p.name} value={p.name.toLowerCase()}>
        {p.name}
      </SelectItem>
    ))}
  </Select>
</div>

{!PROVIDER_DEFAULT_URLS[provider] && (
  <div className="space-y-2">
    <Label>Custom URL *</Label>
    <Input
      value={customUrl}
      onChange={e => setCustomUrl(e.target.value)}
      placeholder="https://your-endpoint.com/v1"
    />
    <p className="text-xs text-muted-foreground">
      {provider} requires a custom endpoint URL
    </p>
  </div>
)}

{derivedUrl && (
  <div className="rounded-md bg-muted p-2 text-xs">
    <span className="text-muted-foreground">Upstream URL: </span>
    <code className="text-foreground">{derivedUrl}</code>
  </div>
)}
```

- [ ] **Step 3: Commit the form changes**

```bash
git add dashboard/src/app/\(dashboard\)/tokens/page.tsx
git commit -m "feat(tokens): simplify token creation with provider-based URL derivation"
```

---

### Task 6: Update Python SDK Token Types

**Files:**
- Modify: `sdk/python/trueflow/types.py`
- Modify: `sdk/python/trueflow/resources/tokens.py`

- [ ] **Step 1: Update Token type in types.py**

```python
@dataclass
class Token:
    id: str
    name: str
    project_id: str
    credential_id: Optional[str]
    provider: Optional[str]  # New field
    custom_url: Optional[str]  # New field
    upstream_url: str  # Derived URL
    allowed_models: Optional[List[str]]
    allowed_providers: Optional[List[str]]
    # ... other fields
```

- [ ] **Step 2: Update tokens.py create method signature**

```python
    def create(
        self,
        name: str,
        provider: Optional[str] = None,
        custom_url: Optional[str] = None,
        credential_id: Optional[str] = None,
        # DEPRECATED: Use provider + custom_url
        upstream_url: Optional[str] = None,
        # ... other params
    ) -> TokenCreateResponse:
        """
        Create a new virtual token.

        Args:
            name: Human-readable name for the token.
            provider: Provider name (e.g., "openai", "anthropic").
                Required for BYOK mode.
            custom_url: Custom endpoint URL. Required for providers
                without default URLs (azure, bedrock, custom).
            credential_id: Optional vault credential ID. If provided,
                operates in managed mode with provider from credential.
            upstream_url: DEPRECATED. Use provider + custom_url instead.
        """
        payload: Dict[str, Any] = {"name": name}

        if provider:
            payload["provider"] = provider
        if custom_url:
            payload["custom_url"] = custom_url
        if credential_id:
            payload["credential_id"] = credential_id
        if upstream_url:
            payload["upstream_url"] = upstream_url
        # ... rest of payload
```

- [ ] **Step 3: Commit SDK changes**

```bash
git add sdk/python/trueflow/types.py sdk/python/trueflow/resources/tokens.py
git commit -m "feat(sdk): add provider and custom_url to Python SDK token API"
```

---

## Part 3: Policy Scope Validation

### Task 7: Update RouteTarget in Policy Model

**Files:**
- Modify: `gateway/src/models/policy.rs:560-580`

- [ ] **Step 1: Make upstream_url optional in RouteTarget**

```rust
/// A single entry in a `DynamicRoute` pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTarget {
    /// Model name to use (e.g., `"gpt-4o-mini"`).
    pub model: String,
    /// Optional custom upstream URL override.
    /// If None, URL is derived from the model's provider at request time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_url: Option<String>,
    /// Optional credential override for this target.
    #[serde(default)]
    pub credential_id: Option<Uuid>,
    /// Weight for weighted random selection (default: 100).
    #[serde(default = "default_route_weight")]
    pub weight: u32,
}
```

- [ ] **Step 2: Update serialization tests**

```rust
#[test]
fn test_route_target_optional_url() {
    let target = RouteTarget {
        model: "gpt-4o".to_string(),
        upstream_url: None,
        credential_id: None,
        weight: 100,
    };
    let json = serde_json::to_string(&target).unwrap();
    assert!(json.contains("\"model\":\"gpt-4o\""));
    assert!(!json.contains("upstream_url"));
}
```

- [ ] **Step 3: Commit the model changes**

```bash
git add gateway/src/models/policy.rs
git commit -m "feat(policy): make upstream_url optional in RouteTarget"
```

---

### Task 8: Update Policy Scope Validation

**Files:**
- Modify: `gateway/src/middleware/policy_scope.rs`

- [ ] **Step 1: Update extract_routing_models_from_json to handle optional URL**

```rust
/// Extract routing models from raw JSON rules.
/// Returns (model, optional_url, action_type) tuples.
pub fn extract_routing_models_from_json(rules: &Value) -> Vec<(String, Option<String>, String)> {
    let mut models = Vec::new();

    if let Some(arr) = rules.as_array() {
        for rule in arr {
            if let Some(actions) = rule.get("then").and_then(|a| a.as_array()) {
                for action in actions {
                    // Check dynamic_route action
                    if let Some(pool) = action.get("dynamic_route")
                        .and_then(|dr| dr.get("pool"))
                        .and_then(|p| p.as_array())
                    {
                        for entry in pool {
                            if let Some(model) = entry.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    let url = entry.get("upstream_url")
                                        .and_then(|u| u.as_str())
                                        .map(|s| s.to_string());
                                    models.push((model.to_string(), url, "dynamic_route".to_string()));
                                }
                            }
                        }
                    }
                    // Check conditional_route action
                    // ... similar updates
                }
            }
        }
    }

    models
}
```

- [ ] **Step 2: Update validate_policy_scope_detailed to derive provider from model**

The existing `detect_provider_from_model` function already handles this. Update the validation to work with optional URLs:

```rust
pub fn validate_policy_scope_detailed(
    routing_models: &[(String, Option<String>, String)],
    allowed_providers: Option<&[String]>,
    allowed_models: Option<&Value>,
) -> Result<(), Vec<DetailedScopeViolation>> {
    let mut violations = Vec::new();

    let has_provider_restriction = allowed_providers.map_or(false, |p| !p.is_empty());
    let has_model_restriction = allowed_models.map_or(false, |v| v.as_array().map_or(false, |arr| !arr.is_empty()));

    for (model, _url, _action_type) in routing_models {
        // Detect provider from model name
        let detected_provider = detect_provider_from_model(model);

        // Check provider restriction
        if has_provider_restriction {
            if let Some(allowed) = allowed_providers {
                let provider_lower = detected_provider.to_lowercase();
                let is_allowed = allowed.iter().any(|p| p.to_lowercase() == provider_lower);

                if !is_allowed {
                    violations.push(DetailedScopeViolation {
                        model: model.clone(),
                        detected_provider: detected_provider.clone(),
                        violation_type: DetailedViolationType::ProviderNotAllowed {
                            allowed: allowed.to_vec(),
                        },
                    });
                    continue;
                }
            }
        }

        // Check model restriction
        // ... existing logic
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}
```

- [ ] **Step 3: Commit the validation changes**

```bash
git add gateway/src/middleware/policy_scope.rs
git commit -m "feat(policy): update scope validation for optional upstream_url"
```

---

### Task 9: Update Frontend Policy Form

**Files:**
- Modify: `dashboard/src/components/policies/policy-form.tsx`

- [ ] **Step 1: Update routing target UI to show derived URL**

When user enters a model, show the derived provider and URL:

```tsx
// In RouteTargetInput component:
const model = watch("model")
const detectedProvider = detectProviderFromModel(model)
const derivedUrl = derivedProvider ? PROVIDER_DEFAULT_URLS[derivedProvider.toLowerCase()] : null

// Show in UI:
<div className="space-y-2">
  <Label>Model</Label>
  <Input {...register("model")} placeholder="gpt-4o-mini" />
  {detectedProvider && (
    <p className="text-xs text-muted-foreground">
      Provider: <Badge variant="outline">{detectedProvider}</Badge>
      {derivedUrl && <span className="ml-2">→ {derivedUrl}</span>}
    </p>
  )}
</div>

<div className="space-y-2">
  <Label>Custom URL (optional)</Label>
  <Input {...register("upstream_url")} placeholder="Leave empty for default" />
  <p className="text-xs text-muted-foreground">
    Override the default provider URL if needed
  </p>
</div>
```

- [ ] **Step 2: Commit the form changes**

```bash
git add dashboard/src/components/policies/policy-form.tsx
git commit -m "feat(policy): simplify routing target UI with derived URLs"
```

---

### Task 10: Add Integration Tests

**Files:**
- Create: `gateway/tests/token_provider_url.rs`

- [ ] **Step 1: Write test for managed mode token creation**

```rust
#[tokio::test]
async fn test_managed_token_derives_url_from_credential_provider() {
    let (state, _) = setup_test_state().await;

    // Create a credential with provider "anthropic"
    let cred = create_test_credential(&state, "anthropic").await;

    // Create token with the credential
    let token = create_token(&state, CreateTokenRequest {
        name: "test".into(),
        credential_id: Some(cred.id),
        provider: None,
        custom_url: None,
        // ...
    }).await.unwrap();

    // Verify URL was derived
    assert!(token.upstream_url.contains("anthropic.com"));
}
```

- [ ] **Step 2: Write test for BYOK mode token creation**

```rust
#[tokio::test]
async fn test_byok_token_requires_provider() {
    let (state, _) = setup_test_state().await;

    // Create token without credential (BYOK mode)
    let result = create_token(&state, CreateTokenRequest {
        name: "test".into(),
        credential_id: None,
        provider: None, // Missing!
        custom_url: None,
        // ...
    }).await;

    // Should fail with validation error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_byok_token_with_custom_url() {
    let (state, _) = setup_test_state().await;

    let token = create_token(&state, CreateTokenRequest {
        name: "test".into(),
        credential_id: None,
        provider: Some("openai".into()),
        custom_url: Some("https://my-proxy.openai.com/v1".into()),
        // ...
    }).await.unwrap();

    assert_eq!(token.upstream_url, "https://my-proxy.openai.com/v1");
}
```

- [ ] **Step 3: Write test for policy scope validation**

```rust
#[tokio::test]
async fn test_policy_scope_validation_with_derived_provider() {
    // Create a token limited to openai provider
    let token = create_token_with_scope(
        allowed_providers: vec!["openai"],
        allowed_models: None,
    ).await;

    // Create a policy routing to claude-3-opus
    let policy = Policy {
        rules: vec![Rule {
            then: vec![Action::DynamicRoute {
                pool: vec![RouteTarget {
                    model: "claude-3-opus".into(),
                    upstream_url: None, // Will derive from model
                    // ...
                }],
                // ...
            }],
        }],
    };

    // Binding should fail - claude is anthropic, not openai
    let result = bind_policy_to_token(&policy, &token).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("anthropic"));
    assert!(err.to_string().contains("not allowed"));
}
```

- [ ] **Step 4: Commit the tests**

```bash
git add gateway/tests/token_provider_url.rs
git commit -m "test: add integration tests for provider-derived URLs and scope validation"
```

---

## Summary

### Files Modified/Created

**Backend (Rust):**
- `gateway/src/api/handlers/dtos.rs` - Add provider, custom_url fields
- `gateway/src/utils/provider_url.rs` - New module for URL derivation
- `gateway/src/utils/mod.rs` - Export new module
- `gateway/src/api/handlers/tokens.rs` - Update token creation logic
- `gateway/src/models/policy.rs` - Make upstream_url optional in RouteTarget
- `gateway/src/middleware/policy_scope.rs` - Update validation logic

**Frontend (TypeScript):**
- `dashboard/src/lib/types/token.ts` - Update types
- `dashboard/src/app/(dashboard)/tokens/page.tsx` - Simplify token creation form
- `dashboard/src/components/policies/policy-form.tsx` - Update routing target UI

**SDK (Python):**
- `sdk/python/trueflow/types.py` - Update Token dataclass
- `sdk/python/trueflow/resources/tokens.py` - Update create method signature

**Tests:**
- `gateway/tests/token_provider_url.rs` - New integration tests

### Breaking Changes

1. **CreateTokenRequest**: `upstream_url` is now optional (backwards compatible via deprecation)
2. **RouteTarget**: `upstream_url` is now optional (backwards compatible)
3. **BYOK tokens**: `provider` field is now required when `credential_id` is None

### Migration Path

1. Backend accepts both old (`upstream_url`) and new (`provider` + `custom_url`) formats
2. Frontend uses new format, sends both for backwards compatibility
3. SDK deprecates `upstream_url` parameter with warning
4. Remove old format support in next major version