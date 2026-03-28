# Policy-Token Binding Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure every policy is bound to a token, and validate that all routing targets are within the token's allowed scope at binding time.

**Architecture:** Three-layer validation:
1. **Database layer:** Enforce binding via schema (policy must have token_id)
2. **API layer:** Validate scope at binding time, return structured errors
3. **UI/SDK layer:** Display actionable error messages

**Tech Stack:** Rust (Axum), TypeScript (Next.js), PostgreSQL

---

## Current State Analysis

### What Exists
| Component | Status | Location |
|-----------|--------|----------|
| Token `allowed_providers` | ✅ | `tokens` table, `model_access.rs` |
| Token `allowed_models` | ✅ | `tokens` table, `model_access.rs` |
| Policy scope validation | ✅ | `policy_scope.rs` |
| Validation at token creation | ✅ | `handlers/tokens.rs:180` |
| Validation at bulk creation | ✅ | `handlers/tokens.rs:489` |

### What's Missing
| Component | Issue |
|-----------|-------|
| Policy-token binding enforcement | Policies can exist without being bound to any token |
| Validation at policy attachment | `guardrails/enable` and `set_token_policy_ids` skip validation |
| Structured error responses | Backend returns generic 400, no details for UI |
| SDK validation | No pre-flight validation in Python SDK |
| UI binding flow | No guided flow to bind policies to tokens |

---

## Proposed Changes

### Data Model Change

**Current:**
```
policies: id, project_id, name, rules, ...
tokens: id, project_id, policy_ids[], ...
```

**Proposed:**
```
policies: id, project_id, name, rules, token_id (FK), ...
tokens: id, project_id, ... (policy_ids removed, derived from policies)
```

**Why:** This enforces at the database level that every policy belongs to a token. It also simplifies the relationship - policies point to tokens, not the other way around.

---

## File Structure

### Files to Modify

```
gateway/
├── src/
│   ├── models/
│   │   └── policy.rs              # Add token_id field
│   ├── api/handlers/
│   │   ├── policies.rs            # Require token_id on create, validate scope
│   │   └── tokens.rs              # Remove policy_ids handling
│   ├── store/postgres/
│   │   ├── policies.rs            # Add token_id to queries
│   │   ├── tokens.rs              # Remove policy_ids from token CRUD
│   │   └── schema.sql             # Add token_id FK to policies
│   ├── middleware/
│   │   └── policy_scope.rs        # Enhanced validation with structured errors
│   └── api/
│       ├── guardrail_presets.rs   # Validate before attaching
│       └── dtos.rs                # Update DTOs

dashboard/
├── src/
│   ├── lib/types/
│   │   ├── policy.ts              # Add token_id to types
│   │   └── token.ts               # Update policy relationship
│   ├── components/policies/
│   │   └── policy-form.tsx        # Add token selector, show scope warnings
│   └── app/(dashboard)/
│       └── policies/
│           └── new/page.tsx       # Require token selection

sdk/python/
└── src/trueflow/
    └── client.py                  # Add validation helpers
```

### Files to Create

```
gateway/
└── migrations/
    └── 041_policy_token_binding.sql  # Migration for token_id FK

dashboard/
└── src/components/policies/
    └── token-scope-warning.tsx       # Scope violation warning component
```

---

## Task 1: Database Migration - Add token_id to Policies

**Files:**
- Create: `gateway/migrations/041_policy_token_binding.sql`
- Test: `cargo test --test migration`

- [ ] **Step 1: Create the migration file**

```sql
-- Migration: Add token_id to policies for binding enforcement
-- Every policy must be bound to a token

-- Add token_id column to policies (nullable initially for migration)
ALTER TABLE policies ADD COLUMN token_id UUID;

-- Create index for fast lookup of policies by token
CREATE INDEX idx_policies_token_id ON policies(token_id);

-- Add foreign key constraint
ALTER TABLE policies
ADD CONSTRAINT fk_policies_token
FOREIGN KEY (token_id) REFERENCES tokens(id) ON DELETE CASCADE;

-- Migrate existing policy-token relationships
-- Policies referenced in tokens.policy_ids will be linked to those tokens
-- For policies referenced by multiple tokens, we need to duplicate them
DO $$
DECLARE
    token_rec RECORD;
    policy_uuid UUID;
    new_policy_id UUID;
    existing_policy JSONB;
BEGIN
    -- Iterate through all tokens with policy_ids
    FOR token_rec IN
        SELECT id, policy_ids, project_id
        FROM tokens
        WHERE policy_ids IS NOT NULL AND array_length(policy_ids, 1) > 0
    LOOP
        -- For each policy_id in the token's policy_ids
        FOREACH policy_uuid IN ARRAY token_rec.policy_ids
        LOOP
            -- Check if this policy is already bound to a token
            SELECT jsonb_build_object(
                'id', id,
                'project_id', project_id,
                'name', name,
                'mode', mode,
                'phase', phase,
                'rules', rules,
                'retry', retry
            ) INTO existing_policy
            FROM policies WHERE id = policy_uuid;

            IF existing_policy IS NOT NULL THEN
                -- Check if already bound
                IF (SELECT token_id FROM policies WHERE id = policy_uuid) IS NULL THEN
                    -- Bind to this token
                    UPDATE policies SET token_id = token_rec.id WHERE id = policy_uuid;
                ELSE
                    -- Already bound to another token, create a copy
                    INSERT INTO policies (project_id, name, mode, phase, rules, retry, token_id)
                    SELECT project_id, name, mode, phase, rules, retry, token_rec.id
                    FROM policies WHERE id = policy_uuid
                    RETURNING id INTO new_policy_id;

                    -- Update any references if needed (audit logs, etc.)
                END IF;
            END IF;
        END LOOP;
    END LOOP;
END $$;

-- Now make token_id NOT NULL
ALTER TABLE policies ALTER COLUMN token_id SET NOT NULL;

-- Remove policy_ids from tokens (now derived from policies.token_id)
ALTER TABLE tokens DROP COLUMN policy_ids;

-- Create view for backward compatibility
CREATE VIEW token_policies AS
SELECT t.id AS token_id, p.id AS policy_id
FROM tokens t
JOIN policies p ON p.token_id = t.id;
```

- [ ] **Step 2: Run migration tests**

```bash
cd gateway
cargo test --test migration
```

- [ ] **Step 3: Commit migration**

```bash
git add gateway/migrations/041_policy_token_binding.sql
git commit -m "feat(db): add token_id FK to policies for binding enforcement

- Every policy must be bound to a token
- Migrates existing policy_ids from tokens to policies.token_id
- Removes tokens.policy_ids (derived from policies.token_id)
- Creates token_policies view for backward compatibility"
```

---

## Task 2: Update Backend Policy Model and DTOs

**Files:**
- Modify: `gateway/src/models/policy.rs`
- Modify: `gateway/src/api/handlers/dtos.rs`

- [ ] **Step 1: Update policy model**

In `gateway/src/models/policy.rs`, add `token_id`:

```rust
// Add to Policy struct
pub struct Policy {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub mode: String,
    pub phase: String,
    pub rules: Value,
    pub retry: Option<Value>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub token_id: Uuid,  // NEW: Required binding to token
}
```

- [ ] **Step 2: Update CreatePolicyRequest DTO**

In `gateway/src/api/handlers/dtos.rs`:

```rust
pub struct CreatePolicyRequest {
    pub name: String,
    pub mode: Option<String>,
    pub phase: Option<String>,
    pub rules: Value,
    pub retry: Option<Value>,
    pub project_id: Option<Uuid>,
    pub token_id: Uuid,  // NEW: Required - policy must bind to a token
}
```

- [ ] **Step 3: Commit model changes**

```bash
git add gateway/src/models/policy.rs gateway/src/api/handlers/dtos.rs
git commit -m "feat(policy): add token_id to policy model and DTOs

- Policy.token_id is required for binding enforcement
- CreatePolicyRequest requires token_id"
```

---

## Task 3: Update Policy Creation Handler with Scope Validation

**Files:**
- Modify: `gateway/src/api/handlers/policies.rs`
- Modify: `gateway/src/middleware/policy_scope.rs`

- [ ] **Step 1: Update create_policy handler**

In `gateway/src/api/handlers/policies.rs`:

```rust
pub async fn create_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreatePolicyRequest>,
) -> impl IntoResponse {
    // ... existing validation ...

    // NEW: Fetch the token to get allowed_providers and allowed_models
    let token = state.db.get_token(&payload.token_id.to_string()).await
        .map_err(|e| {
            tracing::error!("create_policy: failed to fetch token: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "code": "token_not_found",
                        "message": "Token not found"
                    }
                }))
            )
        })?;

    // NEW: Validate policy routing targets against token's allowed scope
    if let Err(violations) = crate::middleware::policy_scope::validate_policies_against_token_scope_detailed(
        &[extract_routing_models_from_rules(&payload.rules)],
        token.allowed_providers.as_deref(),
        token.allowed_models.as_ref(),
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "policy_scope_violation",
                    "message": "Policy routing targets exceed token's allowed scope",
                    "violations": violations
                }
            }))
        ).into_response();
    }

    // Create policy with token binding
    match state.db.insert_policy_with_token(
        project_id,
        &payload.name,
        &mode,
        &phase,
        payload.rules,
        payload.retry,
        payload.token_id,  // NEW
    ).await {
        // ... existing response handling ...
    }
}
```

- [ ] **Step 2: Add detailed validation function**

In `gateway/src/middleware/policy_scope.rs`, add:

```rust
/// Detailed validation result for UI display
#[derive(Debug, Serialize)]
pub struct ScopeViolation {
    pub model: String,
    pub detected_provider: String,
    pub violation_type: ViolationTypeDetail,
}

#[derive(Debug, Serialize)]
pub enum ViolationTypeDetail {
    ProviderNotAllowed { allowed: Vec<String> },
    ModelNotAllowed { allowed_patterns: Vec<String> },
}

/// Validate routing models against token scope, returning detailed violations.
pub fn validate_policies_against_token_scope_detailed(
    routing_models: &[(String, String)],  // (model, action_type)
    allowed_providers: Option<&[String]>,
    allowed_models: Option<&Value>,
) -> Result<(), Vec<ScopeViolation>> {
    let mut violations = Vec::new();

    let has_provider_restriction = allowed_providers.map_or(false, |p| !p.is_empty());
    let has_model_restriction = allowed_models.map_or(false, |v| {
        v.as_array().map_or(false, |arr| !arr.is_empty())
    });

    for (model, _action_type) in routing_models {
        let detected_provider = detect_provider_from_model(model);

        // Check provider restriction
        if has_provider_restriction {
            if let Some(allowed) = allowed_providers {
                let provider_lower = detected_provider.to_lowercase();
                let is_allowed = allowed.iter().any(|p| p.to_lowercase() == provider_lower);

                if !is_allowed {
                    violations.push(ScopeViolation {
                        model: model.clone(),
                        detected_provider: detected_provider.clone(),
                        violation_type: ViolationTypeDetail::ProviderNotAllowed {
                            allowed: allowed.to_vec(),
                        },
                    });
                    continue;
                }
            }
        }

        // Check model restriction
        if has_model_restriction {
            if let Some(models_value) = allowed_models {
                if let Some(patterns) = models_value.as_array() {
                    let model_allowed = patterns.iter().any(|p| {
                        p.as_str().map_or(false, |pattern| {
                            crate::utils::glob_match(pattern, model)
                        })
                    });

                    if !model_allowed {
                        let pattern_strs: Vec<String> = patterns
                            .iter()
                            .filter_map(|p| p.as_str().map(|s| s.to_string()))
                            .collect();

                        violations.push(ScopeViolation {
                            model: model.clone(),
                            detected_provider,
                            violation_type: ViolationTypeDetail::ModelNotAllowed {
                                allowed_patterns: pattern_strs,
                            },
                        });
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}
```

- [ ] **Step 3: Commit handler changes**

```bash
git add gateway/src/api/handlers/policies.rs gateway/src/middleware/policy_scope.rs
git commit -m "feat(policy): validate scope at policy creation time

- Fetch token to get allowed scope
- Return structured violations for UI display
- Policy must be bound to a token"
```

---

## Task 4: Update Policy Store for Token Binding

**Files:**
- Modify: `gateway/src/store/postgres/policies.rs`

- [ ] **Step 1: Update insert_policy to include token_id**

```rust
pub async fn insert_policy_with_token(
    &self,
    project_id: Uuid,
    name: &str,
    mode: &str,
    phase: &str,
    rules: Value,
    retry: Option<Value>,
    token_id: Uuid,  // NEW
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO policies (id, project_id, name, mode, phase, rules, retry, token_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
        id,
        project_id,
        name,
        mode,
        phase,
        rules,
        retry,
        token_id,
    )
    .fetch_one(&self.pool)
    .await
    .map(|row| row.id)
}
```

- [ ] **Step 2: Add get_policies_by_token function**

```rust
/// Get all policies bound to a specific token
pub async fn get_policies_by_token(
    &self,
    token_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<PolicyRow>, sqlx::Error> {
    sqlx::query_as!(
        PolicyRow,
        r#"
        SELECT id, project_id, name, mode, phase, rules, retry, is_active, created_at, token_id
        FROM policies
        WHERE token_id = $1 AND project_id = $2 AND is_active = true
        ORDER BY created_at DESC
        "#,
        token_id,
        project_id,
    )
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 3: Commit store changes**

```bash
git add gateway/src/store/postgres/policies.rs
git commit -m "feat(store): add token_id to policy insertion

- insert_policy_with_token requires token binding
- Add get_policies_by_token for token-specific queries"
```

---

## Task 5: Update Token Store and Handlers

**Files:**
- Modify: `gateway/src/store/postgres/tokens.rs`
- Modify: `gateway/src/api/handlers/tokens.rs`

- [ ] **Step 1: Remove policy_ids from token queries**

Update token queries to remove `policy_ids` column references. The policies are now fetched via `get_policies_by_token`.

- [ ] **Step 2: Update token handler to use new relationship**

In `gateway/src/api/handlers/tokens.rs`, remove policy_ids handling from create_token:

```rust
// Remove this block - policies now reference tokens directly
// if let Some(ref policy_uuids) = payload.policy_ids { ... }
```

- [ ] **Step 3: Commit token changes**

```bash
git add gateway/src/store/postgres/tokens.rs gateway/src/api/handlers/tokens.rs
git commit -m "refactor(tokens): remove policy_ids from token model

- Policies now reference tokens via policies.token_id
- Use get_policies_by_token to fetch token's policies"
```

---

## Task 6: Update Guardrails Endpoint

**Files:**
- Modify: `gateway/src/api/guardrail_presets.rs`

- [ ] **Step 1: Update enable_guardrails to validate scope**

The guardrails endpoint creates policies and attaches them to tokens. Now it must validate scope before creating:

```rust
pub async fn enable_guardrails(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<EnableGuardrailsRequest>,
) -> Result<Json<GuardrailsResponse>, StatusCode> {
    // ... existing auth ...

    // Fetch token to get scope
    let token = state.db.get_token(&payload.token_id).await
        .map_err(|_| StatusCode::NOT_FOUND)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build policy rules
    let rules = build_guardrail_rules(&payload, &token);

    // Validate scope before creating policies
    if let Err(violations) = validate_guardrails_scope(&rules, &token) {
        // Return structured error
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create policy with token binding
    let policy_id = state.db.insert_policy_with_token(
        project_id,
        &policy_name,
        "enforce",
        "response",
        rules,
        None,
        Uuid::parse_str(&payload.token_id).map_err(|_| StatusCode::BAD_REQUEST)?,
    ).await.map_err(|e| {
        tracing::error!("enable_guardrails: failed to create policy: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // ... response ...
}
```

- [ ] **Step 2: Commit guardrails changes**

```bash
git add gateway/src/api/guardrail_presets.rs
git commit -m "feat(guardrails): validate scope and bind policies to tokens

- Guardrail policies now bound to tokens via token_id
- Validates routing targets against token scope"
```

---

## Task 7: Update Frontend Types

**Files:**
- Modify: `dashboard/src/lib/types/policy.ts`
- Modify: `dashboard/src/lib/types/token.ts`

- [ ] **Step 1: Update policy types**

```typescript
export interface PolicyRow {
  id: string
  project_id: string
  name: string
  mode: PolicyMode
  phase: PolicyPhase
  rules: Rule[]
  retry?: RetryConfig
  is_active: boolean
  created_at: string
  token_id: string  // NEW: Required binding to token
}

export interface CreatePolicyRequest {
  name: string
  mode?: PolicyMode
  phase?: PolicyPhase
  rules: Rule[]
  retry?: RetryConfig
  project_id?: string
  token_id: string  // NEW: Required
}
```

- [ ] **Step 2: Update token types**

```typescript
export interface TokenRow {
  id: string
  project_id: string
  name: string
  // ... other fields ...
  // policy_ids: string[]  // REMOVED - policies reference tokens now
  allowed_models: JsonValue
  allowed_providers: string[] | null
}

// Add helper type for fetching policies by token
export interface TokenWithPolicies extends TokenRow {
  policies: PolicyRow[]
}
```

- [ ] **Step 3: Commit type changes**

```bash
git add dashboard/src/lib/types/policy.ts dashboard/src/lib/types/token.ts
git commit -m "feat(frontend): update types for policy-token binding

- PolicyRow.token_id is required
- Remove policy_ids from TokenRow (derived relationship)"
```

---

## Task 8: Update Frontend Policy Form

**Files:**
- Modify: `dashboard/src/components/policies/policy-form.tsx`
- Create: `dashboard/src/components/policies/token-scope-warning.tsx`

- [ ] **Step 1: Add token selector to policy form**

```typescript
// In PolicyFormProps
interface PolicyFormProps {
  initialData?: PolicyRow | null
  onSubmit: (data: CreatePolicyRequest | UpdatePolicyRequest) => Promise<void>
  isSubmitting: boolean
  preselectedTokenId?: string  // Optional: token to bind to
}

// In form state
const [selectedTokenId, setSelectedTokenId] = useState<string>(initialData?.token_id || preselectedTokenId || "")
const [tokenScope, setTokenScope] = useState<TokenScope | null>(null)
const [scopeWarnings, setScopeWarnings] = useState<ScopeViolation[]>([])

// Fetch token scope when token selected
useEffect(() => {
  if (selectedTokenId) {
    fetchTokenScope(selectedTokenId).then(setTokenScope)
  }
}, [selectedTokenId])

// Validate routing actions against token scope
const validateRoutingAgainstScope = (routingAction: ActionDynamicRoute | null) => {
  if (!routingAction || !tokenScope) return []

  const violations: ScopeViolation[] = []

  // Check pool models
  for (const target of routingAction.pool || []) {
    if (tokenScope.allowed_providers && !isProviderAllowed(target.model, tokenScope.allowed_providers)) {
      violations.push({
        model: target.model,
        detected_provider: detectProvider(target.model),
        violation_type: 'provider_not_allowed'
      })
    }
  }

  // Check fallback
  if (routingAction.fallback) {
    // ... same check
  }

  return violations
}
```

- [ ] **Step 2: Create scope warning component**

```typescript
// dashboard/src/components/policies/token-scope-warning.tsx
"use client"

import { AlertCircle, AlertTriangle } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"

interface ScopeViolation {
  model: string
  detected_provider: string
  violation_type: 'provider_not_allowed' | 'model_not_allowed'
  allowed?: string[]
}

interface TokenScopeWarningProps {
  violations: ScopeViolation[]
}

export function TokenScopeWarning({ violations }: TokenScopeWarningProps) {
  if (violations.length === 0) return null

  return (
    <Alert variant="destructive">
      <AlertCircle className="h-4 w-4" />
      <AlertTitle>Policy Scope Violation</AlertTitle>
      <AlertDescription>
        <p className="mb-2">The following routing targets are outside the token's allowed scope:</p>
        <ul className="space-y-1">
          {violations.map((v, i) => (
            <li key={i} className="flex items-center gap-2">
              <code className="text-sm bg-muted px-1 rounded">{v.model}</code>
              <Badge variant="secondary">{v.detected_provider}</Badge>
              <span className="text-xs text-muted-foreground">
                {v.violation_type === 'provider_not_allowed'
                  ? `Provider not in allowed list: ${v.allowed?.join(', ')}`
                  : 'Model not in allowed patterns'}
              </span>
            </li>
          ))}
        </ul>
      </AlertDescription>
    </Alert>
  )
}
```

- [ ] **Step 3: Commit form changes**

```bash
git add dashboard/src/components/policies/policy-form.tsx dashboard/src/components/policies/token-scope-warning.tsx
git commit -m "feat(frontend): add token selector and scope validation to policy form

- Token selection required for policy creation
- Real-time scope validation for routing actions
- Show warnings when targets exceed token scope"
```

---

## Task 9: Update Policy Creation Page

**Files:**
- Modify: `dashboard/src/app/(dashboard)/policies/new/page.tsx`

- [ ] **Step 1: Add token selection flow**

```typescript
// Option 1: Token passed via query param
// /policies/new?token_id=tf_v1_xxx

// Option 2: Token selector in form
"use client"

import { useSearchParams } from "next/navigation"
import { PolicyForm } from "@/components/policies/policy-form"

export default function NewPolicyPage() {
  const searchParams = useSearchParams()
  const preselectedTokenId = searchParams.get('token_id')

  return (
    <div className="container py-6">
      <h1 className="text-2xl font-bold mb-6">Create Policy</h1>
      <PolicyForm
        preselectedTokenId={preselectedTokenId || undefined}
        onSubmit={handleCreatePolicy}
        isSubmitting={false}
      />
    </div>
  )
}
```

- [ ] **Step 2: Commit page changes**

```bash
git add dashboard/src/app/\(dashboard\)/policies/new/page.tsx
git commit -m "feat(frontend): add token selection to policy creation page

- Support preselected token via query param
- Token selector for manual selection"
```

---

## Task 10: Add Backend Error Response Helper

**Files:**
- Create: `gateway/src/api/error_responses.rs`

- [ ] **Step 1: Create structured error types**

```rust
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations: Option<Vec<serde_json::Value>>,
}

impl ApiError {
    pub fn policy_scope_violation(violations: Vec<serde_json::Value>) -> Self {
        Self {
            error: ErrorDetail {
                code: "policy_scope_violation".to_string(),
                message: "Policy routing targets exceed token's allowed scope".to_string(),
                violations: Some(violations),
            },
        }
    }

    pub fn token_not_found() -> Self {
        Self {
            error: ErrorDetail {
                code: "token_not_found".to_string(),
                message: "The specified token was not found".to_string(),
                violations: None,
            },
        }
    }
}
```

- [ ] **Step 2: Commit error responses**

```bash
git add gateway/src/api/error_responses.rs
git commit -m "feat(api): add structured error response types

- Policy scope violation with detailed list
- Token not found error
- Used for frontend error display"
```

---

## Task 11: Update Python SDK

**Files:**
- Modify: `sdk/python/src/trueflow/client.py`

- [ ] **Step 1: Update create_policy to require token_id**

```python
def create_policy(
    self,
    name: str,
    rules: List[Dict],
    token_id: str,  # Now required
    mode: str = "enforce",
    phase: str = "pre",
    retry: Optional[Dict] = None,
) -> Dict:
    """Create a policy bound to a token.

    Args:
        name: Policy name
        rules: Policy rules
        token_id: Token to bind policy to (required)
        mode: 'enforce' or 'shadow'
        phase: 'pre' or 'post'
        retry: Retry configuration

    Returns:
        Created policy with id

    Raises:
        PolicyScopeViolation: If routing targets exceed token's scope
    """
    response = self._post("/api/v1/policies", json={
        "name": name,
        "rules": rules,
        "token_id": token_id,
        "mode": mode,
        "phase": phase,
        "retry": retry,
    })

    if response.status_code == 400:
        error = response.json()
        if error.get("error", {}).get("code") == "policy_scope_violation":
            raise PolicyScopeViolation(
                violations=error["error"]["violations"]
            )

    response.raise_for_status()
    return response.json()


class PolicyScopeViolation(Exception):
    """Raised when policy routing targets exceed token's scope."""

    def __init__(self, violations: List[Dict]):
        self.violations = violations
        messages = [
            f"Model '{v['model']}' ({v['detected_provider']}): {v['violation_type']}"
            for v in violations
        ]
        super().__init__(f"Policy scope violations:\n" + "\n".join(messages))
```

- [ ] **Step 2: Add validation helper**

```python
def validate_policy_scope(
    self,
    token_id: str,
    routing_targets: List[str],  # List of models
) -> List[Dict]:
    """Pre-validate policy routing targets against token scope.

    Args:
        token_id: Token to validate against
        routing_targets: List of model names to check

    Returns:
        List of violations (empty if all valid)
    """
    token = self.get_token(token_id)

    violations = []
    for model in routing_targets:
        provider = detect_provider(model)

        # Check provider
        if token.get("allowed_providers"):
            if provider.lower() not in [p.lower() for p in token["allowed_providers"]]:
                violations.append({
                    "model": model,
                    "detected_provider": provider,
                    "violation_type": "provider_not_allowed",
                })

        # Check model patterns
        if token.get("allowed_models"):
            if not any(match_pattern(p, model) for p in token["allowed_models"]):
                violations.append({
                    "model": model,
                    "detected_provider": provider,
                    "violation_type": "model_not_allowed",
                })

    return violations
```

- [ ] **Step 3: Commit SDK changes**

```bash
git add sdk/python/src/trueflow/client.py
git commit -m "feat(sdk): require token_id for policy creation with scope validation

- create_policy now requires token_id
- Add PolicyScopeViolation exception
- Add validate_policy_scope helper for pre-validation"
```

---

## Self-Review Checklist

- [ ] **Spec coverage**: All routes for policy creation covered
- [ ] **No placeholders**: All code blocks contain actual implementation code
- [ ] **Type consistency**: Types match across Rust, TypeScript, Python
- [ ] **Database migration**: Handles existing data correctly
- [ ] **Error handling**: Structured errors for UI display
- [ ] **SDK support**: Python SDK updated with new flow

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-03-28-policy-token-binding-validation.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**