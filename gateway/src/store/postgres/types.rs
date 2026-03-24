use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct SpendByDimension {
    pub dimension: String,
    pub total_cost_usd: f64,
    pub request_count: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
}

/// User-level spend summary for SaaS builder analytics.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct UserSpendSummary {
    pub external_user_id: String,
    pub total_cost_usd: f64,
    pub request_count: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub token_count: i64,
}

// -- Input structs --

pub struct NewCredential {
    pub project_id: Uuid,
    pub name: String,
    pub provider: String,
    pub encrypted_dek: Vec<u8>,
    pub dek_nonce: Vec<u8>,
    pub encrypted_secret: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub injection_mode: String,
    pub injection_header: String,
}

pub struct NewToken {
    pub id: String,
    pub project_id: Uuid,
    pub name: String,
    pub credential_id: Option<Uuid>,
    pub upstream_url: String,
    pub scopes: serde_json::Value,
    pub policy_ids: Vec<Uuid>,
    pub log_level: Option<i16>,
    /// Optional circuit breaker config. `None` uses gateway defaults.
    pub circuit_breaker: Option<serde_json::Value>,
    /// Model access control: list of allowed model patterns (globs).
    pub allowed_models: Option<serde_json::Value>,
    /// Team assignment for attribution and budget tracking.
    pub team_id: Option<Uuid>,
    /// Tags for cost attribution and tracking.
    pub tags: Option<serde_json::Value>,
    /// MCP tool allowlist. NULL=all allowed, []=none allowed, ["mcp__server__*"]=glob match.
    pub mcp_allowed_tools: Option<serde_json::Value>,
    /// MCP tool blocklist. Takes priority over allowlist. Supports glob patterns.
    pub mcp_blocked_tools: Option<serde_json::Value>,
    /// External user/customer identifier for SaaS builders (e.g., customer ID from billing system).
    pub external_user_id: Option<String>,
    /// Flexible JSONB for SaaS-specific data (plan tier, region, custom attributes).
    pub metadata: Option<serde_json::Value>,
    /// Token purpose: "llm" (LLM calls only), "tool" (tool/MCP calls only), "both" (either).
    pub purpose: String,
}

// -- Output structs --

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct CredentialMeta {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub version: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct TokenRow {
    pub id: String,
    pub project_id: Uuid,
    pub name: String,
    pub credential_id: Option<Uuid>,
    pub upstream_url: String,
    pub scopes: serde_json::Value,
    pub policy_ids: Vec<Uuid>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// Privacy level: 0=metadata, 1=redacted(default), 2=full-debug
    pub log_level: i16,
    /// Optional multi-upstream configuration for loadbalancing
    pub upstreams: Option<serde_json::Value>,
    /// Optional per-token circuit breaker configuration
    pub circuit_breaker: Option<serde_json::Value>,
    /// Model access control: list of allowed model patterns (globs).
    /// NULL = all models allowed (backwards compatible).
    pub allowed_models: Option<serde_json::Value>,
    /// References to named model_access_groups for reusable model restrictions.
    pub allowed_model_group_ids: Option<Vec<Uuid>>,
    /// Team this token belongs to (for attribution and budget tracking)
    pub team_id: Option<Uuid>,
    /// Tags for cost attribution and tracking
    pub tags: Option<serde_json::Value>,
    /// MCP tool allowlist. NULL=all allowed, []=none allowed, ["mcp__server__*"]=glob match.
    pub mcp_allowed_tools: Option<serde_json::Value>,
    /// MCP tool blocklist. Takes priority over allowlist. Supports glob patterns.
    pub mcp_blocked_tools: Option<serde_json::Value>,
    /// SECURITY: Controls how X-TrueFlow-Guardrails header is processed.
    /// Options: "disabled" (ignore, default for security), "append" (add to policies), "override" (replace policies)
    pub guardrail_header_mode: Option<String>,
    /// External user/customer identifier for SaaS builders (e.g., customer ID from billing system).
    pub external_user_id: Option<String>,
    /// Flexible JSONB for SaaS-specific data (plan tier, region, custom attributes).
    pub metadata: Option<serde_json::Value>,
    /// Token purpose: "llm" (LLM calls only), "tool" (tool/MCP calls only), "both" (either).
    pub purpose: String,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct PolicyRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub mode: String,
    pub phase: String,
    pub rules: serde_json::Value,
    pub retry: Option<serde_json::Value>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

// ── Session Summary Types ─────────────────────────────────────

/// Per-request item inside a session summary.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct SessionRequestRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub model: Option<String>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub response_latency_ms: Option<i64>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub tool_call_count: Option<i16>,
    pub cache_hit: Option<bool>,
    pub custom_properties: Option<serde_json::Value>,
    pub payload_url: Option<String>,
}

/// Full session summary (aggregate + per-request breakdown).
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSummaryRow {
    pub session_id: Option<String>,
    pub total_requests: i64,
    pub total_cost_usd: Option<rust_decimal::Decimal>,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_latency_ms: i64,
    pub models_used: Option<Vec<String>>,
    pub first_request_at: DateTime<Utc>,
    pub last_request_at: DateTime<Utc>,
    pub requests: Vec<SessionRequestRow>,
}

// ── Session Entity (Lifecycle) ────────────────────────────────

/// A first-class session entity with lifecycle, spend caps, and metadata.
/// Created automatically on first request via upsert.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct SessionEntity {
    pub id: Uuid,
    pub session_id: String,
    pub project_id: Uuid,
    pub token_id: Option<Uuid>,
    pub status: String,
    pub spend_cap_usd: Option<rust_decimal::Decimal>,
    pub total_cost_usd: rust_decimal::Decimal,
    pub total_tokens: i64,
    pub total_requests: i64,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct OidcProviderRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub jwks_uri: Option<String>,
    pub audience: Option<String>,
    pub claim_mapping: serde_json::Value,
    pub default_role: String,
    pub default_scopes: String,
    pub enabled: bool,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub token_id: Option<String>,
    pub method: String,
    pub path: String,
    pub upstream_status: Option<i16>,
    pub response_latency_ms: i32,
    pub agent_name: Option<String>,
    pub policy_result: String,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub shadow_violations: Option<Vec<String>>,
    pub fields_redacted: Option<Vec<String>>,
    // Phase 4 columns
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub model: Option<String>,
    pub tokens_per_second: Option<f32>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub external_request_id: Option<String>,
    pub log_level: Option<i16>,
    // Phase 5: LLM Observability
    pub tool_call_count: Option<i16>,
    pub finish_reason: Option<String>,
    pub error_type: Option<String>,
    pub is_streaming: Option<bool>,
    // Phase 6: Response Cache
    pub cache_hit: Option<bool>,
}

/// Detailed audit log row with joined body data (for single-entry view).
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct AuditLogDetailRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub token_id: Option<String>,
    pub method: String,
    pub path: String,
    pub upstream_url: String,
    pub upstream_status: Option<i16>,
    pub response_latency_ms: i32,
    pub agent_name: Option<String>,
    pub policy_result: String,
    pub policy_mode: Option<String>,
    pub deny_reason: Option<String>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub shadow_violations: Option<Vec<String>>,
    pub fields_redacted: Option<Vec<String>>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub model: Option<String>,
    pub tokens_per_second: Option<f32>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub external_request_id: Option<String>,
    pub log_level: Option<i16>,
    // Phase 5: LLM Observability
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_count: Option<i16>,
    pub finish_reason: Option<String>,
    pub session_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub error_type: Option<String>,
    pub is_streaming: Option<bool>,
    pub ttft_ms: Option<i64>,
    // From audit_log_bodies JOIN
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub request_headers: Option<serde_json::Value>,
    pub response_headers: Option<serde_json::Value>,
    // Phase 6: Router Debugger
    pub cache_hit: Option<bool>,
    pub router_info: Option<serde_json::Value>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct ProjectRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct PolicyVersionRow {
    pub id: Uuid,
    pub policy_id: Uuid,
    pub version: i32,
    pub name: Option<String>,
    pub mode: Option<String>,
    pub phase: Option<String>,
    pub rules: serde_json::Value,
    pub retry: Option<serde_json::Value>,
    pub changed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Service Registry ─────────────────────────────────────────

pub struct NewService {
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub base_url: String,
    pub service_type: String,
    pub credential_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub role: String,
    pub scopes: serde_json::Value,
    pub is_active: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub team_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct UsageMeterRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub period: NaiveDate,
    pub total_requests: i64,
    pub total_tokens_used: i64,
    pub total_spend_usd: rust_decimal::Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Analytics structs
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenSummary {
    pub token_id: Option<String>,
    pub total_requests: i64,
    pub errors: i64,
    pub avg_latency_ms: f64,
    pub last_active: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenVolumeStat {
    pub hour: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStatusStat {
    pub status: i16,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenLatencyStat {
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ModelPricingRow {
    pub id: Uuid,
    pub provider: String,
    pub model_pattern: String,
    pub input_per_m: rust_decimal::Decimal,
    pub output_per_m: rust_decimal::Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// -- Prompt Management --

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PromptRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub folder: String,
    pub tags: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PromptVersionRow {
    pub id: Uuid,
    pub prompt_id: Uuid,
    pub version: i32,
    pub model: String,
    pub messages: serde_json::Value,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub tools: Option<serde_json::Value>,
    pub commit_message: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub labels: serde_json::Value,
}

pub struct NewPrompt {
    pub project_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub folder: String,
    pub tags: serde_json::Value,
    pub created_by: String,
}

pub struct NewPromptVersion {
    pub prompt_id: Uuid,
    pub model: String,
    pub messages: serde_json::Value,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub tools: Option<serde_json::Value>,
    pub commit_message: String,
    pub created_by: String,
}

// ── Supabase User Sync ─────────────────────────────────────────

/// Request to sync a user from Supabase Auth to the gateway database.
/// Called by the dashboard after successful Supabase login.
#[derive(Debug, Deserialize)]
pub struct SyncUserRequest {
    /// Supabase Auth user ID (from JWT `sub` claim)
    pub supabase_id: Uuid,
    /// User email from Supabase Auth
    pub email: String,
    /// Display name (optional, from OAuth providers)
    pub name: Option<String>,
    /// Avatar URL (optional, from OAuth providers)
    pub picture: Option<String>,
}

/// Response after syncing a user from Supabase.
#[derive(Debug, Serialize)]
pub struct SyncUserResponse {
    /// Gateway user ID
    pub user_id: Uuid,
    /// Organization ID the user belongs to
    pub org_id: Uuid,
    /// User role in the organization
    pub role: String,
    /// Whether this was a new user creation
    pub is_new_user: bool,
    /// The user's last selected project (for cross-device persistence)
    pub last_project_id: Option<Uuid>,
}

/// User row with Supabase fields
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct UserRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub role: String,
    pub supabase_id: Option<Uuid>,
    pub name: Option<String>,
    pub picture_url: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_project_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// ── Users & Tokens Analytics Types ─────────────────────────────────────

/// User growth timeseries point for the Users & Tokens tab.
/// Tracks new users and cumulative user count over time.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserGrowthPoint {
    pub bucket: DateTime<Utc>,
    pub new_users: i64,
    pub cumulative_users: i64,
}

/// Engagement tier breakdown for the Users & Tokens tab.
/// Classifies users by request volume: Power (>100), Regular (10-100), Light (1-9).
#[derive(Debug, Serialize, Deserialize)]
pub struct EngagementTiersResponse {
    pub power_users: i64,
    pub regular_users: i64,
    pub light_users: i64,
    pub total_users: i64,
}

/// Token alerts for the Users & Tokens tab.
/// Shows active tokens and those hitting rate limits.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenAlertsResponse {
    pub active_tokens: i64,
    pub token_limit: Option<i64>,
    pub tokens_at_rate_limit: i64,
    pub rate_limited_tokens: Vec<RateLimitedToken>,
}

/// A token that is approaching or at rate limit.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RateLimitedToken {
    pub token_name: String,
    pub percent: f64,
}

/// Requests per user timeseries point for the Users & Tokens tab.
/// Shows request volume per user over time.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RequestsPerUserPoint {
    pub bucket: DateTime<Utc>,
    pub user_count: i64,
    pub request_count: i64,
    pub avg_per_user: f64,
}

// ── MCP Server Persistence Types ─────────────────────────────────────

/// Persisted MCP server configuration loaded from the database.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct McpServerRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub auth_type: String,
    pub api_key_encrypted: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret_enc: Option<String>,
    pub oauth_token_endpoint: Option<String>,
    pub oauth_scopes: Option<Vec<String>>,
    pub oauth_access_token_enc: Option<String>,
    pub oauth_refresh_token_enc: Option<String>,
    pub oauth_token_expires_at: Option<DateTime<Utc>>,
    pub status: String,
    pub tool_count: i32,
    pub last_error: Option<String>,
    pub discovered_server_info: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Persisted MCP tool schema from the database.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct McpServerToolRow {
    pub id: Uuid,
    pub server_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
}

/// Input for creating/updating an MCP server.
pub struct NewMcpServer {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub auth_type: String,
    pub api_key_encrypted: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret_enc: Option<String>,
    pub oauth_token_endpoint: Option<String>,
    pub oauth_scopes: Option<Vec<String>>,
    pub oauth_access_token_enc: Option<String>,
    pub oauth_refresh_token_enc: Option<String>,
    pub oauth_token_expires_at: Option<DateTime<Utc>>,
    pub status: String,
    pub tool_count: i32,
    pub discovered_server_info: Option<serde_json::Value>,
}
