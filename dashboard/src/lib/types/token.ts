// Token types - from gateway/src/store/postgres/types.rs

// JSON value type - matches backend's serde_json::Value
// Can be null, boolean, number, string, array, or object
type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

export interface TokenRow {
  id: string                    // tf_v1_xxx_tok_xxx format
  project_id: string
  name: string
  credential_id: string | null
  upstream_url: string
  scopes: JsonValue             // JSON value
  policy_ids: string[]          // UUID array
  is_active: boolean            // true = active, false = revoked
  expires_at: string | null
  created_at: string
  log_level: number             // 0=metadata, 1=redacted, 2=full
  upstreams: UpstreamTarget[] | null     // Multi-upstream config
  circuit_breaker: CircuitBreakerConfig | null
  allowed_models: JsonValue     // JSON value (typically string[] or null)
  allowed_model_group_ids: string[] | null
  allowed_providers: string[] | null  // Provider access control
  team_id: string | null
  tags: JsonValue               // JSON value (typically string[] or null)
  mcp_allowed_tools: JsonValue  // JSON value (typically string[] or null)
  mcp_blocked_tools: JsonValue  // JSON value (typically string[] or null)
  allowed_ips: JsonValue        // JSON value (typically string[] or null) - CIDR notation
  blocked_ips: JsonValue        // JSON value (typically string[] or null)
  guardrail_header_mode: string | null
  external_user_id: string | null
  metadata: Record<string, unknown> | null
  purpose: 'llm' | 'tool' | 'both'
  spend_cap_usd: number | null
  spend_used_usd: number | null
}

export interface UpstreamTarget {
  url: string
  weight: number
  priority: number
  credential_id?: string | null
  /** Model override - if set, replaces the request model when this upstream is selected */
  model?: string | null
  /** Glob patterns for model filtering - only route matching models to this upstream */
  allowed_models?: string[] | null
}

export interface CircuitBreakerConfig {
  enabled: boolean
  failure_threshold: number     // >= 1
  recovery_cooldown_secs: number // >= 1
  half_open_max_requests: number // >= 1
}

// CreateTokenRequest - from gateway/src/api/handlers/dtos.rs
// Note: Backend uses serde_json::Value for flexibility, but we typically use arrays
export interface CreateTokenRequest {
  name: string
  credential_id?: string | null  // null = BYOK passthrough mode
  upstream_url: string
  project_id?: string
  policy_ids?: string[]
  log_level?: number            // 0/1/2
  log_level_name?: 'metadata' | 'redacted' | 'full'
  circuit_breaker?: CircuitBreakerConfig
  fallback_url?: string
  upstreams?: UpstreamTarget[]
  allowed_models?: string[]     // Sent as JSON array to backend
  allowed_providers?: string[]  // Provider access control
  team_id?: string
  tags?: string[]               // Sent as JSON array to backend
  mcp_allowed_tools?: string[]  // Sent as JSON array to backend
  mcp_blocked_tools?: string[]  // Sent as JSON array to backend
  allowed_ips?: string[]        // CIDR notation: ["192.168.0.0/16", "10.0.0.1"]
  blocked_ips?: string[]        // Block specific IPs
  external_user_id?: string
  metadata?: Record<string, unknown>
  purpose?: 'llm' | 'tool' | 'both'
}

// TokenUsageStats - from gateway/src/models/analytics.rs
export interface TokenUsageStats {
  total_requests: number
  success_count: number
  error_count: number
  avg_latency_ms: number
  total_cost_usd: number
  hourly: TokenUsageBucket[]
}

export interface TokenUsageBucket {
  bucket: string
  count: number
}

// CredentialMeta - from gateway/src/store/postgres/types.rs
export interface CredentialMeta {
  id: string
  name: string
  provider: string
  version: number
  is_active: boolean
  created_at: string
}

// CreateCredentialRequest - from gateway/src/api/handlers/dtos.rs
// Supports two modes:
// 1. Builtin vault (default): Provide `secret`, AILink encrypts and stores it
// 2. External vault: Provide `vault_backend` and `encrypted_secret_ref`
export interface CreateCredentialRequest {
  name: string
  provider: string
  // Plaintext API key for builtin vault (will be encrypted by AILink)
  // Required when vault_backend is "builtin" or not specified
  secret?: string
  // Vault backend to use: "builtin" (default), "aws_kms", "hashicorp_vault"
  vault_backend?: 'builtin' | 'aws_kms' | 'hashicorp_vault'
  // Pre-encrypted secret reference for external vaults
  // For AWS KMS: base64-encoded ciphertext blob from `aws kms encrypt`
  encrypted_secret_ref?: string
  project_id?: string
  injection_mode?: 'bearer' | 'basic' | 'header' | 'query' | 'sigv4'
  injection_header?: string    // e.g. "Authorization"
}

// Bulk operations
export interface BulkCreateTokenRequest {
  tokens: CreateTokenRequest[]
}

export interface BulkCreateTokenResponse {
  created: CreateTokenResponse[]
  failed: BulkTokenFailure[]
  total_requested: number
  total_created: number
}

export interface BulkTokenFailure {
  name: string
  error: string
}

export interface BulkRevokeRequest {
  external_user_id?: string
  team_id?: string
  token_ids?: string[]
}

export interface BulkRevokeResponse {
  revoked_count: number
  token_ids: string[]
}

// API response types
export interface CreateTokenResponse {
  token_id: string
  name: string
  message: string
}

export interface CreateCredentialResponse {
  id: string
  name: string
  message: string
}

export interface DeleteResponse {
  id: string
  deleted: boolean
}