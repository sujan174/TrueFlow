use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Token DTOs ──────────────────────────────────────────────
#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
    pub credential_id: Option<Uuid>,
    pub upstream_url: String,
    pub project_id: Option<Uuid>,
    pub policy_ids: Option<Vec<Uuid>>,
    /// Numeric log level (0/1/2) — deprecated in favour of log_level_name.
    #[serde(rename = "log_level")]
    pub log_level_num: Option<i16>,
    /// String log level: "metadata" | "redacted" | "full"
    /// Takes precedence over log_level (numeric) if both are supplied.
    #[serde(rename = "log_level_name")]
    pub log_level_str: Option<String>,
    /// Optional circuit breaker config override. Omit to use gateway defaults.
    /// Example: `{"enabled": false}` to disable CB for dev/test tokens.
    pub circuit_breaker: Option<serde_json::Value>,
    /// Convenience shorthand: set a single fallback URL (priority 2).
    /// Equivalent to specifying two entries in `upstreams` with priority 1 and 2.
    #[allow(dead_code)]
    pub fallback_url: Option<String>,
    /// Optional full upstream list with weights and priorities.
    /// If provided, `upstream_url` is used as a fallback only if this list is empty.
    /// Each entry: {"url": "...", "weight": 100, "priority": 1, "credential_id": null}
    pub upstreams: Option<Vec<crate::proxy::loadbalancer::UpstreamTarget>>,
    /// Model access control: list of allowed model patterns (globs).
    /// NULL = all models allowed (no restriction).
    pub allowed_models: Option<serde_json::Value>,
    /// Team this token belongs to (for attribution and budget tracking).
    pub team_id: Option<Uuid>,
    /// Tags for cost attribution and tracking.
    pub tags: Option<serde_json::Value>,
    /// MCP tool allowlist. NULL=all allowed, []=none, ["mcp__server__*"]=glob.
    pub mcp_allowed_tools: Option<serde_json::Value>,
    /// MCP tool blocklist. Takes priority over allowlist.
    pub mcp_blocked_tools: Option<serde_json::Value>,
    /// External user/customer identifier for SaaS builders.
    /// Links this token to a specific customer in your billing system.
    pub external_user_id: Option<String>,
    /// Flexible metadata for SaaS-specific data (plan tier, region, custom attributes).
    pub metadata: Option<serde_json::Value>,
    /// Token purpose: "llm" (LLM calls only), "tool" (tool/MCP calls only), "both" (either).
    /// Defaults to "llm".
    #[serde(default = "default_purpose")]
    pub purpose: String,
}

fn default_purpose() -> String {
    "llm".to_string()
}

impl CreateTokenRequest {
    /// Resolve the numeric log level from either the string name or the numeric field.
    pub fn resolved_log_level(&self) -> Option<i16> {
        if let Some(ref name) = self.log_level_str {
            return match name.as_str() {
                "metadata" => Some(0),
                "redacted" => Some(1),
                "full" => Some(2),
                _ => self.log_level_num,
            };
        }
        self.log_level_num
    }
}

#[derive(Serialize)]
pub struct CreateTokenResponse {
    pub token_id: String,
    pub name: String,
    pub message: String,
}

// ── Bulk Token DTOs ─────────────────────────────────────────────

/// Request for bulk token creation (SaaS builder onboarding).
/// Maximum 500 tokens per request.
#[derive(Deserialize)]
pub struct BulkCreateTokenRequest {
    pub tokens: Vec<CreateTokenRequest>,
}

#[derive(Serialize)]
pub struct BulkCreateTokenResponse {
    pub created: Vec<CreateTokenResponse>,
    pub failed: Vec<BulkTokenFailure>,
    pub total_requested: usize,
    pub total_created: usize,
}

#[derive(Serialize)]
pub struct BulkTokenFailure {
    pub name: String,
    pub error: String,
}

/// Request for bulk token revocation by filter criteria.
/// At least one filter must be provided.
#[derive(Deserialize)]
pub struct BulkRevokeRequest {
    /// Revoke all tokens for this external user/customer.
    pub external_user_id: Option<String>,
    /// Revoke all tokens for this team.
    pub team_id: Option<Uuid>,
    /// Revoke specific token IDs (alternative to filter-based revocation).
    pub token_ids: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct BulkRevokeResponse {
    pub revoked_count: usize,
    pub token_ids: Vec<String>,
}

// ── Approval DTOs ───────────────────────────────────────────
#[derive(Deserialize)]
pub struct DecisionRequest {
    pub decision: String, // "approved" | "rejected"
}

#[derive(Serialize)]
pub struct DecisionResponse {
    pub id: Uuid,
    pub status: String,
    pub updated: bool,
}

// ── Pagination / Analytics DTOs ─────────────────────────────
#[derive(Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub project_id: Option<Uuid>,
    /// Filter tokens by external user/customer ID.
    pub external_user_id: Option<String>,
    /// Filter tokens by team ID.
    pub team_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct SpendBreakdownParams {
    pub project_id: Option<Uuid>,
    /// Grouping dimension: "model", "token", or "tag:<key>" (e.g. "tag:team")
    pub group_by: Option<String>,
    /// Time window in hours (default: 720 = 30 days, max: 8760 = 1 year)
    pub hours: Option<i32>,
}

#[derive(Serialize)]
pub struct SpendBreakdownResponse {
    pub group_by: String,
    pub dimension_label: String,
    pub hours: i32,
    pub total_cost_usd: f64,
    pub total_requests: i64,
    pub breakdown: Vec<crate::store::postgres::SpendByDimension>,
}

/// Response for user-level spend analytics (GET /api/v1/analytics/users).
#[derive(Serialize)]
pub struct UserSpendResponse {
    pub hours: i32,
    pub total_users: i64,
    pub total_cost_usd: f64,
    pub users: Vec<crate::store::postgres::UserSpendSummary>,
}

// ── Project DTOs ────────────────────────────────────────────
#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
}

// ── Session DTOs ────────────────────────────────────────────
#[derive(serde::Deserialize)]
pub struct UpdateSessionStatusRequest {
    pub status: String, // "active", "paused", "completed"
}

/// PATCH /api/v1/sessions/:session_id/status — change session lifecycle status.
///

#[derive(serde::Deserialize)]
pub struct SetSpendCapRequest {
    pub spend_cap_usd: rust_decimal::Decimal,
}

// ── Policy / Credential DTOs ────────────────────────────────
#[derive(Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub mode: Option<String>,  // "enforce" | "shadow", defaults to "enforce"
    pub phase: Option<String>, // "pre" | "post", defaults to "pre"
    pub rules: serde_json::Value,
    pub retry: Option<serde_json::Value>,
    pub project_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct UpdatePolicyRequest {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub phase: Option<String>,
    pub rules: Option<serde_json::Value>,
    pub retry: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct PolicyResponse {
    pub id: Uuid,
    pub name: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct DeleteResponse {
    pub id: Uuid,
    pub deleted: bool,
}

// ── Credential DTOs ──────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateCredentialRequest {
    pub name: String,
    pub provider: String,
    pub secret: String, // plaintext API key — will be encrypted
    pub project_id: Option<Uuid>,
    pub injection_mode: Option<String>, // "header" (default) | "bearer"
    pub injection_header: Option<String>, // e.g. "Authorization"
}

#[derive(Serialize)]
pub struct CreateCredentialResponse {
    pub id: Uuid,
    pub name: String,
    pub message: String,
}

// ── Revoke DTO ───────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Serialize)]
pub struct RevokeResponse {
    pub token_id: String,
    pub revoked: bool,
}

// ── Service DTOs ────────────────────────────────────────────
#[derive(Deserialize)]
pub struct CreateServiceRequest {
    pub name: String,
    pub description: Option<String>,
    pub base_url: String,
    pub service_type: Option<String>,
    pub credential_id: Option<String>,
    pub project_id: Option<Uuid>,
}

// ── API Key DTOs ────────────────────────────────────────────
#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub role: String, // admin | member | readonly
    pub scopes: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub id: Uuid,
    pub key: String, // Only returned once
    pub message: String,
}

#[derive(Serialize)]
pub struct WhoAmIResponse {
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub role: String,
    pub scopes: Vec<String>,
}

// ── API Key Handlers ─────────────────────────────────────────

// ── Spend Cap DTOs ──────────────────────────────────────────
#[derive(Deserialize)]
pub struct UpsertSpendCapRequest {
    pub period: String, // "daily" | "monthly"
    pub limit_usd: f64,
}

// ── Webhook DTOs ────────────────────────────────────────────
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WebhookRow {
    pub id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Signing secret returned once on creation; `None` on list responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_secret: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub events: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct TestWebhookResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct TestWebhookRequest {
    pub url: String,
}

// ── Pricing DTOs ────────────────────────────────────────────
#[derive(Deserialize)]
pub struct UpsertPricingRequest {
    pub provider: String,
    pub model_pattern: String,
    pub input_per_m: rust_decimal::Decimal,
    pub output_per_m: rust_decimal::Decimal,
}

#[derive(Serialize)]
pub struct PricingEntryResponse {
    pub id: uuid::Uuid,
    pub provider: String,
    pub model_pattern: String,
    pub input_per_m: rust_decimal::Decimal,
    pub output_per_m: rust_decimal::Decimal,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Settings DTOs ───────────────────────────────────────────
#[derive(serde::Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct RehydrateRequest {
    pub tokens: Vec<String>,
}
