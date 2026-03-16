# TrueFlow — Security Model

> This document describes TrueFlow's security architecture, threat model, and encryption design.

---

## Core Security Principle

**Agents do not access real API keys.**

All agent credentials are virtual tokens (`tf_v1_...`) that only work through the TrueFlow gateway. Real API keys are encrypted at rest, decrypted in memory only during request forwarding, and zeroed immediately after.

---

## Threat Model

### Threats & Mitigations

| # | Threat | Severity | Mitigation |
|---|---|---|---|
| T1 | **Prompt Injection → Key Exfiltration** | Critical | Agent only has virtual token. Real key never in agent's environment. `print(os.environ)` yields `tf_v1_...`, which is useless without the gateway |
| T2 | **Stolen Virtual Token** | High | Tokens are scoped (methods, paths, rate limits). Instantly revocable. IP allowlisting available. Short TTLs optional |
| T3 | **Replay Attack** | Medium | Idempotency keys, request timestamping, rate limiting |
| T4 | **Man-in-the-Middle** | High | TLS 1.3 enforced on all connections. mTLS available for enterprise |
| T5 | **Runaway Agent Costs** | High | Per-token spend caps (atomic checks via Redis Lua). Per-window rate limits. HITL for high-value operations |
| T5.1 | **HITL Resource Exhaustion** | Medium | `HITL_MAX_PENDING_PER_TOKEN` boundary limits pending approvals, preventing memory/queue exhaustion |
| T6 | **Accidental Destructive Operations** | High | Method + path whitelists (e.g., GET only). HITL for write operations. Shadow mode for safe rollout |
| T7 | **Gateway Infrastructure Compromise** | Critical | Secrets encrypted at rest (AES-256-GCM). DEKs held in memory only during request. Master key in environment variable or external KMS, never in database |
| T8 | **Insider Threat (TrueFlow Operator)** | High | Envelope encryption — operators can access encrypted blobs but not plaintext. Master keys in HSM/KMS for enterprise |
| T9 | **Stale Compromised Credentials** | Medium | Configurable key rotation via background jobs minimizes the blast radius of a compromised credential |
| T10 | **Supply Chain Attack (SDK)** | Medium | SDKs published with SLSA provenance. Dependencies pinned and audited |
| T11 | **Database Breach** | High | All credentials encrypted at rest. Audit logs contain request hashes, not request bodies. PII redacted before storage |
| T12 | **SSRF (Server-Side Request Forgery)** | High | Webhook URLs validated: HTTPS-only, no private/reserved IPs (RFC 1918), no cloud metadata access |
| T13 | **Timing Attacks** | Medium | Constant-time string comparison for all API key and token validations |

---

## Role-Based Access Control (RBAC)

TrueFlow enforces a layered authorization model: **Role → Scope → Resource**.

### Roles

| Role | How Assigned | Auto-Passes All Scopes? | Description |
|------|-------------|-------------------------|-------------|
| **SuperAdmin** | `TRUEFLOW_ADMIN_KEY` env var | ✅ Yes | Full system access. Used for initial setup and break-glass operations |
| **Admin** | API key with `role: "admin"` | ✅ Yes | Full access within the organization. Can create/delete tokens, policies, credentials |
| **Member** | API key with `role: "member"` | ❌ No | Read/write access gated by individual scopes. Cannot perform admin-only operations |
| **ReadOnly** | API key with `role: "read_only"` | ❌ No | Read-only access gated by individual scopes |

> **Key behavior**: Admin and SuperAdmin roles automatically satisfy any scope check. Member and ReadOnly roles must have the specific scope explicitly granted on the API key.

### Scopes

Scopes follow the `resource:action` convention. 16 scope namespaces are available:

| Scope | Description |
|-------|-------------|
| `tokens:read` | List tokens, view usage, view circuit breaker config |
| `tokens:write` | Create and revoke tokens (requires **admin** role) |
| `policies:read` | List policies and policy versions |
| `policies:write` | Create, update, delete policies (requires **admin** role) |
| `credentials:read` | List credential metadata (secrets never returned) |
| `credentials:write` | Store and delete credentials (requires **admin** role) |
| `projects:read` | List projects |
| `projects:write` | Create and update projects |
| `approvals:read` | List pending HITL approval requests |
| `approvals:write` | Approve or reject HITL requests |
| `audit:read` | Query audit logs, stream logs, view sessions |
| `sessions:write` | Update session status, set session spend caps (requires **admin** role) |
| `services:read` | List registered services |
| `services:write` | Register and delete services (requires **admin** role) |
| `webhooks:read` | List webhooks |
| `webhooks:write` | Create, delete, and test webhooks (requires **admin** role) |
| `notifications:read` | List notifications, count unread |
| `notifications:write` | Mark notifications as read |
| `pricing:read` | List model pricing entries |
| `pricing:write` | Create and delete pricing overrides (requires **admin** role) |
| `billing:read` | View organization-level usage and spend |
| `analytics:read` | View analytics dashboards, per-token metrics, spend breakdowns |
| `keys:manage` | List API keys; create and revoke keys (create/revoke require **admin** role) |
| `mcp:read` | List MCP servers, view cached tools, discover endpoints |
| `mcp:write` | Register, delete, and refresh MCP servers (requires **admin** role) |
| `pii:rehydrate` | Decrypt tokenized PII references (requires **admin** role) |

### Default Scopes by Role

When creating an API key, you can specify custom scopes. If omitted, these defaults apply:

| Role | Default Scopes |
|------|---------------|
| **Admin** | All scopes (auto-pass) |
| **Member** | All `*:read` scopes + `approvals:write` + `notifications:write` |
| **ReadOnly** | All `*:read` scopes only |

### Admin-Required Operations

These operations require `admin` role **in addition to** the relevant scope:

- **Token management**: Create, revoke tokens
- **Policy management**: Create, update, delete policies
- **Credential vault**: Store, delete credentials
- **Project deletion & purge**: Delete projects, GDPR data purge
- **Session lifecycle**: Update status, set spend caps
- **Services**: Register, delete services
- **Webhooks**: Create, delete, test webhooks
- **Pricing**: Create, delete pricing overrides
- **API keys**: Create, revoke keys
- **MCP servers**: Register, delete, refresh, discover
- **System**: Flush cache, view cache stats, anomaly detection, settings
- **PII**: Rehydrate tokenized PII

### Unscoped Endpoints

These endpoints require only a valid authenticated API key (any role):

| Endpoints | Rationale |
|-----------|-----------|
| `/prompts/*` | Prompts are operational resources used by all team members |
| `/experiments/*` | Experiments are read/managed as part of normal workflow |
| `/config/export`, `/config/import` | Config-as-code operations (export is read-only; import is additive) |
| `/guardrails/presets`, `/guardrails/status` | Listing available presets is informational |
| `/analytics/volume`, `/analytics/status`, `/analytics/latency` | Basic dashboard metrics |
| `/auth/whoami` | Self-identification |

### Safety Guards

- **Last admin key protection**: Revoking the last active admin key is blocked to prevent lockout
- **Constant-time comparison**: All API key and token validations use constant-time string comparison to prevent timing attacks
- **Scope escalation prevention**: A Member cannot create a key with Admin role or add scopes they don't have

---

## Encryption Design

### Envelope Encryption

TrueFlow implements envelope encryption, following the pattern used by AWS KMS and HashiCorp Vault.

```
┌─────────────────────────────────────────────────────────┐
│ Master Key (KEK)                                        │
│ Source: Environment variable or external KMS             │
│ Never stored in database                                │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │ Data Encryption Key (DEK)                          │  │
│  │ Unique per credential                              │  │
│  │ Stored encrypted (by KEK) in PostgreSQL            │  │
│  │                                                    │  │
│  │  ┌─────────────────────────────────────────────┐   │  │
│  │  │ Credential (e.g., sk_live_...)               │   │  │
│  │  │ Encrypted by DEK using AES-256-GCM           │   │  │
│  │  │ Stored as: nonce (12B) + ciphertext + tag     │   │  │
│  │  └─────────────────────────────────────────────┘   │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Key Properties

| Property | Value |
|---|---|
| Algorithm | AES-256-GCM (authenticated encryption) |
| Nonce | 96-bit, unique per encryption operation, **never reused** |
| DEK | 256-bit, unique per credential |
| KEK (Master Key) | Derived from env var (`TRUEFLOW_MASTER_KEY`) or external KMS |

### Key Rotation

- **Master Key Rotation**: Decrypt all DEKs with old master, re-encrypt with new master. Credentials themselves are untouched.
- **DEK Rotation**: Generate new DEK, decrypt credential with old DEK, re-encrypt with new DEK.
- **Credential Rotation**: TrueFlow's auto-rotation feature creates a new key on the provider API (e.g., Stripe), encrypts it with a new DEK, and revokes the old key after a grace period.

---

## Data Security

### What TrueFlow Stores

| Data | Storage | Encryption |
|---|---|---|
| Real API keys | PostgreSQL | AES-256-GCM (envelope encrypted) |
| Virtual tokens | PostgreSQL | Plaintext (they are not secrets — useless without the gateway) |
| Policies | PostgreSQL | Plaintext (not sensitive) |
| Audit logs | PostgreSQL (partitioned) | Plaintext metadata. Bodies stored only at Level 1+ (PII-scrubbed) or Level 2 (full debug, auto-expires after 24h) |

### What TrueFlow Does NOT Store (at Level 0)

- Request/response bodies (only metadata: method, path, status, latency, cost)
- Real API keys in plaintext anywhere

### Privacy-Gated Body Capture (Phase 4)

| Log Level | Bodies | Headers | PII | Auto-Expiry |
|---|---|---|---|---|
| **0** (default) | ❌ Not stored | ❌ Not stored | N/A | — |
| **1** (scrubbed) | ✅ PII-redacted | ❌ Not stored | SSN, email, CC, phone, API keys scrubbed | — |
| **2** (full debug) | ✅ Raw bodies | ✅ Full headers | No redaction | **24 hours** (auto-downgraded to Level 0) |

### Data Retention

- Audit logs: 90-day retention (configurable). Old monthly partitions are dropped automatically.
- Level 2 debug bodies: Auto-expired by background cleanup job (runs hourly).
- Credentials: Retained until explicitly deleted. Old rotated versions deleted after grace period.

---

## Network Security

| Layer | Protection |
|---|---|
| Agent → Gateway | TLS 1.3 (enforced). mTLS optional for enterprise |
| Gateway → Upstream | TLS (uses upstream API's certificate) |
| Gateway → PostgreSQL | TLS or Unix socket |
| Gateway → Redis | TLS or private network |
| Dashboard → Gateway | Shared secret auth (`DASHBOARD_SECRET`) + Strict CORS (`DASHBOARD_ORIGIN`) |

### IP Allowlisting

Tokens can optionally specify allowed source IPs:

```json
{
  "when": { "field": "source_ip", "op": "not_in", "value": ["10.0.0.0/8", "192.168.1.100/32"] },
  "then": { "action": "deny" }
}
```

---

## Runtime Security

### Secret Lifecycle in Memory

1. Agent request arrives
2. Gateway decrypts DEK with master key (in memory)
3. Gateway decrypts credential with DEK (in memory)
4. Credential injected into upstream request header
5. Upstream request sent
6. **Credential zeroed from memory immediately after injection**

The real credential exists in memory for the shortest possible time — typically microseconds.

### Process Isolation

- The gateway runs as a non-root user in Docker
- No shell access from the gateway process
- Secrets are never logged (structured logging excludes credential values)
- Environment variables with secrets are not exposed to child processes

---

## Security Fixes Registry

The following security issues have been identified and addressed in the TrueFlow codebase. This registry documents each fix for compliance and audit purposes.

### Critical Severity

| ID | Description | Fix | Location |
|----|-------------|-----|----------|
| CRIT-1 | Default project ID was global shared UUID, allowing cross-tenant data access | Generate org-specific deterministic UUID v5 from org_id | `api/mod.rs:default_project_id()` |
| CRIT-4 | KEK held in memory after VaultCrypto drop | Zeroize KEK on drop using `zeroize` crate | `vault/builtin.rs:Drop` |

### High Severity

| ID | Description | Fix | Location |
|----|-------------|-----|----------|
| HIGH-4 | Policy engine continued processing after deny action | Short-circuit evaluation stops immediately on deny | `middleware/engine/mod.rs` |
| HIGH-9 | Circuit breaker half-open limit is instance-local | Documented limitation; mitigation via conservative settings | `proxy/loadbalancer.rs` |

### Medium Severity

| ID | Description | Fix | Location |
|----|-------------|-----|----------|
| MED-1 | Input validation gaps | Validation added | Various |
| MED-2 | Input validation gaps | Validation added | Various |
| MED-3 | Input validation gaps | Validation added | Various |
| MED-4 | Admin key weak entropy warning | Log warning if key < 32 chars | `api/mod.rs:admin_auth()` |
| MED-5 | Empty policy condition should be safe default | Treat empty conditions as denial | `middleware/engine/evaluate.rs` |
| MED-6 | Action ordering not documented | Documented MED-6 in policy engine | `middleware/engine/mod.rs` |
| MED-7 | Spend cap race conditions | Redis atomic operations | `middleware/spend.rs` |
| MED-8 | Cache miss thundering herd on spend checks | Single-flight pattern | `middleware/spend.rs` |
| MED-9 | Spend cap enforcement gaps | Hard cap enforcement | `middleware/spend.rs` |
| MED-10 | Input validation gaps | Validation added | Various |
| MED-11 | Input validation gaps | Validation added | Various |
| MED-12 | Input validation gaps | Validation added | Various |
| MED-13 | External guardrail threshold not validated | Threshold bounds checking | `middleware/external_guardrail.rs` |
| MED-14 | SSN regex matched dashed-only patterns incorrectly | Fixed SSN pattern | `middleware/redact.rs` |
| MED-15 | Credit card validation missing Luhn check | Added Luhn algorithm validation | `middleware/redact.rs` |

### Low Severity (H-series)

| ID | Description | Fix | Location |
|----|-------------|-----|----------|
| H-1 | Dangerous header injection blocked | Block host, content-length, etc. | `main.rs:handle_credential_command` |
| H-6 | UTF-8 duration parsing edge case | Fixed UTF-8 handling | `middleware/policy.rs` |

---

## OIDC JWT Authentication Flow

TrueFlow supports OpenID Connect (OIDC) authentication for the Management API. The flow uses full cryptographic verification via JWKS.

### Provider Registration

OIDC providers are registered directly in the `oidc_providers` database table:

```sql
INSERT INTO oidc_providers (org_id, name, issuer_url, client_id, jwks_uri, audience, claim_mapping, default_role, default_scopes, enabled)
VALUES (
  'org-uuid',
  'Okta',
  'https://your-org.okta.com',
  'client-id',
  'https://your-org.okta.com/.well-known/jwks.json',
  'trueflow-api',
  '{"role": "custom:role", "scopes": "custom:scopes"}',
  'member',
  '["tokens:read", "policies:read"]',
  true
);
```

### JWT Validation Flow

```
1. Request arrives with Authorization: Bearer {jwt}
   |
2. Peek at JWT payload (unverified) to extract `iss` claim
   |
3. Look up OIDC provider by issuer URL in database
   |
4. Fetch JWKS from provider's jwks_uri (cached)
   |
5. Verify JWT signature using matching JWK
   |
6. Validate claims:
   - iss matches provider issuer_url
   - aud contains configured audience
   - exp is in the future
   - iat is valid
   |
7. Extract role and scopes from claim_mapping
   |
8. Enforce SEC-01: OIDC cannot grant SuperAdmin
   - "superadmin" claim capped to Admin role
   |
9. Create AuthContext with mapped role/scopes
   |
10. Proceed to authorization check
```

### Security Properties

- **SEC-01**: OIDC can never grant SuperAdmin role. Any `superadmin` claim is capped to Admin.
- **Signature Verification**: Full JWKS-based signature verification using RS256/RS384/RS512/ES256/ES384/ES512.
- **Audience Validation**: Token must include the configured audience.
- **Issuer Validation**: Token issuer must match a registered provider.
- **Claim Mapping**: Flexible mapping from JWT claims to TrueFlow roles and scopes.

### Example Claim Mapping

```json
{
  "role": "custom:trueflow_role",
  "scopes": "custom:trueflow_scopes",
  "org_id": "custom:organization_id"
}
```

---

## Guardrail Header Mode Security

The `X-TrueFlow-Guardrails` header allows per-request guardrail configuration. This is controlled by the token's `guardrail_header_mode` field.

### Modes

| Mode | Behavior | Security Implication |
|------|----------|---------------------|
| `disabled` (default) | Header is ignored | Prevents unauthorized guardrail injection |
| `append` | Header actions added to configured policies | Trusted callers can add guardrails |
| `override` | Header actions replace all policies | High trust; bypasses existing policies |

### Security Considerations

1. **Default is Secure**: By default, `guardrail_header_mode` is `disabled`, preventing unauthorized agents from injecting guardrails.

2. **Trust Required**: Only enable `append` or `override` modes for tokens where the agent is trusted to specify guardrails.

3. **Audit Trail**: All header-based guardrail activations are logged with the token ID for traceability.

### Warning Log

When a header is present but ignored:

```
WARN: X-TrueFlow-Guardrails header present but ignored (guardrail_header_mode=disabled)
```

### Recommended Configuration

- Production tokens: `guardrail_header_mode: "disabled"`
- Development tokens: `guardrail_header_mode: "append"`
- Never use `override` in production without explicit security review

---

## Compliance Roadmap

*Note: The following are planned roadmap items for TrueFlow enterprise deployments. Timelines are targets, not factual commitments.*

| Certification | Description |
|---|---|
| SOC 2 Type II | Ongoing continuous monitoring and auditing for cloud environments |
| GDPR | Comprehensive data residency configurations and native DSR support |
| HIPAA | Dedicated BAA process and expanded PHI-redaction presets |
