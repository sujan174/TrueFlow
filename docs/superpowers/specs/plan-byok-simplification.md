# Implementation Plan: BYOK Passthrough Simplification

**Spec**: `docs/superpowers/specs/2026-03-28-byok-simplification-design.md`

## Overview

Simplify BYOK mode with a single standard header (`X-TF-Real-Auth`) for real API keys and enforce single-provider constraint for passthrough tokens.

## Phase 1: Gateway Changes (Rust)

### Step 1.1: Update Token Extraction Logic
**File**: `gateway/src/proxy/handler/core.rs`

- Modify `extract_virtual_token()` function to:
  1. Check `X-TrueFlow-Auth` header first
  2. Fallback to `Authorization` header for backward compatibility
  3. Return the extracted token and a flag indicating the source header

### Step 1.2: Add X-TF-Real-Auth Header Extraction
**File**: `gateway/src/proxy/handler/core.rs`

- Add function `extract_real_auth_key()` that:
  1. Checks `X-TF-Real-Auth` header
  2. Validates format (Bearer prefix)
  3. Returns the real API key

### Step 1.3: Validate Passthrough Requirements
**File**: `gateway/src/proxy/handler/core.rs`

- In the main request handler:
  1. If `credential_id == NULL`, require `X-TF-Real-Auth` header
  2. Return error: "Passthrough token requires X-TF-Real-Auth header"
  3. Use the extracted real key for upstream requests

### Step 1.4: Update Header Redaction
**File**: `gateway/src/proxy/handler/headers.rs`

- Add `x-tf-real-auth` to the list of redacted headers for audit logs

### Step 1.5: Single Provider Validation (Token Creation)
**File**: `gateway/src/api/handlers/tokens.rs`

- In `create_token`:
  1. If `credential_id == NULL`, validate that `upstreams` has only one entry
  2. Return error if multiple upstreams provided for passthrough mode

### Step 1.6: Single Provider Validation (Policy Binding)
**File**: `gateway/src/middleware/policy_scope.rs`

- Extend `validate_policies_against_token_scope()` to:
  1. Check if token is passthrough mode (`credential_id == NULL`)
  2. Validate that all policy routes use the same provider as `upstream_url`
  3. Return error if policy routes to different provider

## Phase 2: Dashboard Changes (TypeScript)

### Step 2.1: Update Token Mode Selector
**File**: `dashboard/src/components/tokens/token-mode-selector.tsx`

- Already created, update messaging for new header name

### Step 2.2: Update Token Creation Form
**File**: `dashboard/src/app/(dashboard)/tokens/page.tsx`

- Already updated with mode selector
- Ensure single-provider dropdown shows for passthrough mode
- Update info box to mention `X-TF-Real-Auth` header

### Step 2.3: Update Token Detail Page
**File**: `dashboard/src/app/(dashboard)/tokens/[id]/page.tsx`

- Show upstream URL prominently for BYOK tokens
- Add usage instructions for X-TF-Real-Auth header

## Phase 3: Python SDK Changes

### Step 3.1: Update byok() Method
**File**: `sdk/python/trueflow/client.py`

- Update `TrueFlowClient.byok()` to send:
  - `X-TrueFlow-Auth: Bearer {virtual_token}`
  - `X-TF-Real-Auth: Bearer {real_api_key}`

### Step 3.2: Deprecate Old Methods
**File**: `sdk/python/trueflow/client.py`

- Add deprecation warning to `with_upstream_key()` method
- Update docstrings to recommend new approach

## Phase 4: Tests

### Step 4.1: Gateway Integration Tests
**File**: `gateway/tests/integration.rs`

- Test: Passthrough token with X-TF-Real-Auth header succeeds
- Test: Passthrough token without X-TF-Real-Auth header fails
- Test: Managed token with Authorization header succeeds
- Test: X-TF-Real-Auth header is redacted in audit logs

### Step 4.2: SDK Tests
**File**: `sdk/python/tests/test_client.py`

- Test: `byok()` sends correct headers
- Test: Real key is not logged

## Phase 5: Documentation

### Step 5.1: Update Authentication Docs
**File**: `docs/authentication-modes.md`

- Update BYOK section with new header
- Add migration guide for existing users
- Update code examples

## File Changes Summary

| File | Action | Description |
|------|--------|-------------|
| `gateway/src/proxy/handler/core.rs` | Modify | Update token extraction, add X-TF-Real-Auth handling |
| `gateway/src/proxy/handler/headers.rs` | Modify | Add x-tf-real-auth to redacted headers |
| `gateway/src/api/handlers/tokens.rs` | Modify | Validate single provider for passthrough |
| `gateway/src/middleware/policy_scope.rs` | Modify | Validate policy routes for passthrough tokens |
| `dashboard/src/components/tokens/token-mode-selector.tsx` | Modify | Update messaging |
| `dashboard/src/app/(dashboard)/tokens/page.tsx` | Modify | Already updated |
| `dashboard/src/app/(dashboard)/tokens/[id]/page.tsx` | Modify | Add passthrough instructions |
| `sdk/python/trueflow/client.py` | Modify | Update byok() headers |
| `docs/authentication-modes.md` | Modify | Update documentation |
| `gateway/tests/integration.rs` | Modify | Add BYOK tests |

## Execution Order

1. Gateway changes (Phase 1) - Backend first
2. Dashboard changes (Phase 2) - UI updates
3. SDK changes (Phase 3) - Client library
4. Tests (Phase 4) - Verify everything works
5. Documentation (Phase 5) - Update docs

## Risk Mitigation

- **Backward Compatibility**: Keep old headers working during migration period
- **Clear Errors**: Return helpful error messages for missing headers
- **Deprecation Warnings**: Log warnings when old headers are used

## Acceptance Criteria

- [ ] BYOK token creation enforces single provider
- [ ] X-TF-Real-Auth header is required for passthrough tokens
- [ ] Clear error message when header is missing
- [ ] SDK sends correct headers
- [ ] All tests pass
- [ ] Documentation updated