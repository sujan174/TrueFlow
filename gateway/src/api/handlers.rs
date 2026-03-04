use std::sync::Arc;
use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    Extension,
    Json,
};
use futures::stream::{self, Stream};
use serde_json::json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AuthContext, ApiKeyRole};
use crate::models::approval::ApprovalStatus;
use crate::store::postgres::{
    ApiKeyRow, AuditLogDetailRow, AuditLogRow, CredentialMeta, PolicyRow, TokenRow,
    TokenSummary, TokenVolumeStat, TokenStatusStat, TokenLatencyStat,
};
use crate::AppState;

// ── Request / Response DTOs ──────────────────────────────────

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
}

impl CreateTokenRequest {
    /// Resolve the numeric log level from either the string name or the numeric field.
    pub fn resolved_log_level(&self) -> Option<i16> {
        if let Some(ref name) = self.log_level_str {
            return match name.as_str() {
                "metadata" => Some(0),
                "redacted"  => Some(1),
                "full"      => Some(2),
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

#[derive(Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub project_id: Option<Uuid>,
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

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
}

// ── Default org ID for MVP ───────────────────────────────
#[allow(dead_code)]
fn default_org_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
}

// ── Default project ID for MVP ───────────────────────────────
#[allow(dead_code)]
fn default_project_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
}

/// Verify that `project_id` belongs to `org_id`.
/// Returns `Err(FORBIDDEN)` if the project doesn't belong to the org,
/// or `Err(INTERNAL_SERVER_ERROR)` on DB failure.
async fn verify_project_ownership(
    state: &crate::AppState,
    org_id: Uuid,
    project_id: Uuid,
) -> Result<(), StatusCode> {
    let belongs = state
        .db
        .project_belongs_to_org(project_id, org_id)
        .await
        .map_err(|e| {
            tracing::error!("project_belongs_to_org failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !belongs {
        tracing::warn!(
            org_id = %org_id,
            project_id = %project_id,
            "project isolation: project does not belong to org"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

// ── Handlers ─────────────────────────────────────────────────

/// GET /api/v1/projects — list projects for the org
pub async fn list_projects(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ProjectResponse>>, StatusCode> {
    auth.require_scope("projects:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let projects = state.db.list_projects(auth.org_id).await.map_err(|e| {
        tracing::error!("list_projects failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        projects
            .into_iter()
            .map(|p| ProjectResponse {
                id: p.id,
                name: p.name,
            })
            .collect(),
    ))
}

/// POST /api/v1/projects — create a new project
pub async fn create_project(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), StatusCode> {
    auth.require_scope("projects:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let id = state
        .db
        .create_project(auth.org_id, &payload.name)
        .await
        .map_err(|e| {
            tracing::error!("create_project failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse {
            id,
            name: payload.name,
        }),
    ))
}

/// PUT /api/v1/projects/:id — rename a project
pub async fn update_project(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(payload): Json<CreateProjectRequest>, // Reuse struct since it just needs name
) -> Result<Json<ProjectResponse>, StatusCode> {
    auth.require_scope("projects:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    let updated = state
        .db
        .update_project(id, auth.org_id, &payload.name)
        .await
        .map_err(|e| {
            tracing::error!("update_project failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(ProjectResponse {
        id,
        name: payload.name,
    }))
}

/// DELETE /api/v1/projects/:id — delete a project
pub async fn delete_project(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<StatusCode, StatusCode> {
    // Only Admin (or SuperAdmin) can delete projects
    if auth.role != ApiKeyRole::Admin && auth.role != ApiKeyRole::SuperAdmin {
        return Err(StatusCode::FORBIDDEN);
    }

    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    // PREVENT DELETING THE DEFAULT PROJECT
    if id == auth.default_project_id() {
        tracing::warn!("attempt to delete default project prevented");
        return Err(StatusCode::BAD_REQUEST);
    }

    let deleted = state
        .db
        .delete_project(id, auth.org_id)
        .await
        .map_err(|e| {
            tracing::error!("delete_project failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// POST /api/v1/projects/:id/purge — GDPR Article 17 (Right to Erasure)
///
/// Irreversibly purges all personal and operational data associated with a project:
/// - Audit logs / request traces
/// - Agent sessions
/// - Virtual key usage records
///
/// The project and its virtual keys are preserved so operators can still issue invoices.
/// To fully remove the project, call DELETE /api/v1/projects/:id after purging.
///
/// **This action is irreversible. Requires Admin or SuperAdmin role.**
pub async fn purge_project_data(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Only Admin (or SuperAdmin) can trigger a data purge
    if auth.role != ApiKeyRole::Admin && auth.role != ApiKeyRole::SuperAdmin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "admin or superadmin role required" })),
        ));
    }

    let project_id = Uuid::parse_str(&id_str).map_err(|_| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "invalid project id" })),
    ))?;

    tracing::warn!(
        project_id = %project_id,
        actor_role = ?auth.role,
        "GDPR data purge requested"
    );

    let rows_purged = state
        .db
        .purge_project_data(project_id, auth.org_id)
        .await
        .map_err(|e| {
            tracing::error!("purge_project_data failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "purge failed", "detail": e.to_string() })),
            )
        })?;

    Ok(Json(serde_json::json!({
        "status": "purged",
        "project_id": project_id,
        "rows_deleted": rows_purged,
        "gdpr_article": "17 — Right to Erasure"
    })))
}


/// GET /api/v1/tokens — list all tokens for a project
pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenRow>>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let tokens = state.db.list_tokens(project_id).await.map_err(|e| {
        tracing::error!("list_tokens failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(tokens))
}

/// POST /api/v1/tokens — create a new virtual token
pub async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreateTokenResponse>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = payload.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Validate upstream URL (SSRF protection — same as CLI)
    let url = reqwest::Url::parse(&payload.upstream_url).map_err(|_| {
        tracing::warn!(
            "create_token: invalid upstream URL: {}",
            payload.upstream_url
        );
        StatusCode::BAD_REQUEST
    })?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // P1.4: Validate upstreams list if provided (weight > 0, no duplicate URLs)
    if let Some(ref upstreams) = payload.upstreams {
        // Check for zero/negative weights
        for u in upstreams {
            if u.weight == 0 {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
        // Check for duplicate URLs
        let urls: Vec<_> = upstreams.iter().map(|u| u.url.as_str()).collect();
        let unique: std::collections::HashSet<_> = urls.iter().copied().collect();
        if unique.len() != urls.len() {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    // Generate token ID
    let proj_short = &project_id.to_string()[..8];
    let mut random_bytes = [0u8; 16];
    use aes_gcm::aead::OsRng;
    use rand::RngCore;
    OsRng.fill_bytes(&mut random_bytes);
    let token_id = format!("tf_v1_{}_tok_{}", proj_short, hex::encode(random_bytes));

    let resolved_log_level = payload.resolved_log_level();
    let new_token = crate::store::postgres::NewToken {
        id: token_id.clone(),
        project_id,
        name: payload.name.clone(),
        credential_id: payload.credential_id,
        upstream_url: payload.upstream_url,
        scopes: serde_json::json!([]),
        policy_ids: payload.policy_ids.unwrap_or_default(),
        log_level: resolved_log_level,
        circuit_breaker: payload.circuit_breaker,
        allowed_models: payload.allowed_models,
        team_id: payload.team_id,
        tags: payload.tags,
        mcp_allowed_tools: payload.mcp_allowed_tools,
        mcp_blocked_tools: payload.mcp_blocked_tools,
    };

    state.db.insert_token(&new_token).await.map_err(|e| {
        tracing::error!("create_token failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateTokenResponse {
            token_id: token_id.clone(),
            name: payload.name,
            message: format!("Use: Authorization: Bearer {}", token_id),
        }),
    ))
}

/// DELETE /api/v1/tokens/:id — revoke a token
pub async fn revoke_token(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;
    // Verify the token belongs to the org by looking it up first
    let token = state.db.get_token(&id).await.map_err(|e| {
        tracing::error!("revoke_token lookup failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if let Some(ref t) = token {
        verify_project_ownership(&state, auth.org_id, t.project_id).await?;
    }

    let revoked = state.db.revoke_token(&id).await.map_err(|e| {
        tracing::error!("revoke_token failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}



/// GET /api/v1/approvals — list pending HITL requests
pub async fn list_approvals(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::approval::ApprovalRequest>>, StatusCode> {
    auth.require_scope("approvals:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let approvals = state
        .db
        .list_approval_requests(project_id)
        .await
        .map_err(|e| {
            tracing::error!("list_approvals failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(approvals))
}

/// POST /api/v1/approvals/:id/decision — approve or reject a request
pub async fn decide_approval(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
    Json(payload): Json<DecisionRequest>,
) -> Result<Json<DecisionResponse>, StatusCode> {
    auth.require_scope("approvals:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let id = Uuid::parse_str(&id_str).map_err(|_| {
        tracing::warn!("decide_approval: invalid UUID: {}", id_str);
        StatusCode::BAD_REQUEST
    })?;
    // Map string to enum
    let status = match payload.decision.to_lowercase().as_str() {
        "approved" | "approve" => ApprovalStatus::Approved,
        "rejected" | "reject" => ApprovalStatus::Rejected,
        other => {
            tracing::warn!("decide_approval: invalid decision: {}", other);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Extract project_id from query or default
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    tracing::info!(
        "decide_approval: properties id={}, project_id={}, status={:?}",
        id,
        project_id,
        status
    );

    let updated = state
        .db
        .update_approval_status(id, project_id, status.clone())
        .await
        .map_err(|e| {
            tracing::error!("decide_approval failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let status_str = match status {
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Rejected => "rejected",
        _ => "unknown",
    };

    // ── M4: Notify waiting BLPOP in proxy handler via Redis ──────────────
    // Push the decision to `hitl:decision:{id}` so the gateway's BLPOP
    // unblocks instantly instead of waiting for the next poll interval.
    // Fire-and-forget — Redis failure doesn't affect the HTTP response.
    if updated {
        let mut redis_conn = state.cache.redis();
        let hitl_key = format!("hitl:decision:{}", id);
        let _: redis::RedisResult<i64> = redis::AsyncCommands::lpush(
            &mut redis_conn,
            &hitl_key,
            status_str,
        ).await;
        // Set a short TTL so the key doesn't linger if the gateway crashed
        let _: redis::RedisResult<bool> = redis::AsyncCommands::expire(
            &mut redis_conn,
            &hitl_key,
            60_i64,
        ).await;
    }

    Ok(Json(DecisionResponse {
        id,
        status: status_str.to_string(),
        updated,
    }))
}

/// GET /api/v1/audit — paginated audit logs
pub async fn list_audit_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<AuditLogRow>>, StatusCode> {
    // Audit logs require explicit scope or read-all
    auth.require_scope("audit:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200); // 1 <= limit <= 200
    let offset = params.offset.unwrap_or(0).max(0); // non-negative

    let logs = state
        .db
        .list_audit_logs(project_id, limit, offset)
        .await
        .map_err(|e| {
            tracing::error!("list_audit_logs failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(logs))
}

/// GET /api/v1/audit/:id — single audit log detail with bodies
pub async fn get_audit_log(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<AuditLogDetailRow>, StatusCode> {
    auth.require_scope("audit:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let log_id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    let log = state
        .db
        .get_audit_log_detail(log_id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_audit_log_detail failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(log))
}

/// GET /api/v1/sessions/:session_id — cost rollup for an entire agent session.
///
/// Returns aggregate totals (cost, tokens, latency, models) plus a per-request
/// breakdown ordered chronologically. Useful for calculating true agent run cost.
///
/// Example response:
/// ```json
/// {
///   "session_id": "agent-run-42",
///   "total_requests": 5,
///   "total_cost_usd": "0.0847",
///   "total_prompt_tokens": 12500,
///   ...
///   "requests": [{ "id": "...", "model": "gpt-4o", "cost_usd": "0.03", ... }]
/// }
/// ```
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::store::postgres::SessionSummaryRow>, StatusCode> {
    auth.require_scope("audit:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let summary = state
        .db
        .get_session_summary(&session_id, project_id)
        .await
        .map_err(|e| {
            tracing::error!(session_id = %session_id, "get_session_summary failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(summary))
}

/// GET /api/v1/sessions — list recent sessions ordered by latest activity.
///
/// Returns per-session aggregates (cost, tokens, latency, models) without
/// per-request breakdown. Use GET /api/v1/sessions/:id for the full detail.
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::store::postgres::SessionSummaryRow>>, StatusCode> {
    auth.require_scope("audit:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    let sessions = state
        .db
        .list_sessions(project_id, limit, offset)
        .await
        .map_err(|e| {
            tracing::error!("list_sessions failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(sessions))
}

// ── Session Lifecycle Management ────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct UpdateSessionStatusRequest {
    pub status: String, // "active", "paused", "completed"
}

/// PATCH /api/v1/sessions/:session_id/status — change session lifecycle status.
///
/// Valid transitions: active → paused, paused → active, * → completed.
pub async fn update_session_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
    Json(payload): Json<UpdateSessionStatusRequest>,
) -> Result<Json<crate::store::postgres::SessionEntity>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("sessions:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Validate status value
    match payload.status.as_str() {
        "active" | "paused" | "completed" => {}
        _ => return Err(StatusCode::UNPROCESSABLE_ENTITY),
    }

    let session = state
        .db
        .update_session_status(&session_id, project_id, &payload.status)
        .await
        .map_err(|e| {
            tracing::error!(session_id = %session_id, "update_session_status failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    tracing::info!(
        session_id = %session_id,
        new_status = %payload.status,
        "Session status updated"
    );

    Ok(Json(session))
}

#[derive(serde::Deserialize)]
pub struct SetSpendCapRequest {
    pub spend_cap_usd: rust_decimal::Decimal,
}

/// PUT /api/v1/sessions/:session_id/spend-cap — set session-level budget.
pub async fn set_session_spend_cap(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
    Json(payload): Json<SetSpendCapRequest>,
) -> Result<Json<crate::store::postgres::SessionEntity>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("sessions:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let session = state
        .db
        .set_session_spend_cap(&session_id, project_id, payload.spend_cap_usd)
        .await
        .map_err(|e| {
            tracing::error!(session_id = %session_id, "set_session_spend_cap failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    tracing::info!(
        session_id = %session_id,
        spend_cap_usd = %payload.spend_cap_usd,
        "Session spend cap set"
    );

    Ok(Json(session))
}

/// GET /api/v1/sessions/:session_id/entity — get the session lifecycle entity.
///
/// Returns the first-class session entity with status, spend caps, and totals.
/// Different from GET /sessions/:id which returns audit log aggregates.
pub async fn get_session_entity(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::store::postgres::SessionEntity>, StatusCode> {
    auth.require_scope("audit:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let session = state
        .db
        .get_session_entity(&session_id, project_id)
        .await
        .map_err(|e| {
            tracing::error!(session_id = %session_id, "get_session_entity failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(session))
}

// ── Policy DTOs ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub mode: Option<String>, // "enforce" | "shadow", defaults to "enforce"
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

// ── Policy Handlers ──────────────────────────────────────────

/// GET /api/v1/policies — list all policies for a project
pub async fn list_policies(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<PolicyRow>>, StatusCode> {
    auth.require_scope("policies:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let policies = state.db.list_policies(project_id).await.map_err(|e| {
        tracing::error!("list_policies failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(policies))
}

/// POST /api/v1/policies — create a new policy
pub async fn create_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreatePolicyRequest>,
) -> impl IntoResponse {
    if auth.require_role("admin").is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    if auth.require_scope("policies:write").is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let project_id = payload.project_id.unwrap_or_else(|| auth.default_project_id());
    // SEC: verify project isolation
    if let Err(status) = verify_project_ownership(&state, auth.org_id, project_id).await {
        return status.into_response();
    }
    let mode = payload.mode.unwrap_or_else(|| "enforce".to_string());
    let phase = payload.phase.unwrap_or_else(|| "pre".to_string());

    // Validate mode
    if mode != "enforce" && mode != "shadow" {
        tracing::warn!("create_policy: invalid mode: {}", mode);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("invalid mode: {}", mode) })),
        ).into_response();
    }

    // Validate phase
    if phase != "pre" && phase != "post" {
        tracing::warn!("create_policy: invalid phase: {}", phase);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("invalid phase: {}", phase) })),
        ).into_response();
    }

    // SEC: enforce max size on rules JSON to prevent oversized payloads clogging DB+memory
    const MAX_RULES_BYTES: usize = 64 * 1024; // 64KB
    let rules_str = payload.rules.to_string();
    if rules_str.len() > MAX_RULES_BYTES {
        tracing::warn!("create_policy: rules JSON too large: {} bytes", rules_str.len());
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": format!("rules JSON exceeds maximum size of {}KB", MAX_RULES_BYTES / 1024) })),
        ).into_response();
    }

    match state
        .db
        .insert_policy(project_id, &payload.name, &mode, &phase, payload.rules, payload.retry)
        .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(json!(PolicyResponse {
                id,
                name: payload.name,
                message: "Policy created".to_string(),
            })),
        ).into_response(),
        Err(e) => {
            tracing::error!("create_policy failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal server error" })),
            ).into_response()
        }
    }
}

/// PUT /api/v1/policies/:id — update a policy
pub async fn update_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(payload): Json<UpdatePolicyRequest>,
) -> Result<Json<PolicyResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("policies:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    // Validate mode if provided
    if let Some(ref mode) = payload.mode {
        if mode != "enforce" && mode != "shadow" {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Validate phase if provided
    if let Some(ref phase) = payload.phase {
        if phase != "pre" && phase != "post" {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let updated = state
        .db
        .update_policy(
            id,
            project_id,
            payload.mode.as_deref(),
            payload.phase.as_deref(),
            payload.rules,
            payload.retry,
            payload.name.as_deref(),
        )
        .await
        .map_err(|e| {
            tracing::error!("update_policy failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(PolicyResponse {
        id,
        name: payload.name.unwrap_or_default(),
        message: "Policy updated".to_string(),
    }))
}

/// DELETE /api/v1/policies/:id — soft-delete a policy
pub async fn delete_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<DeleteResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("policies:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    let deleted = state.db.delete_policy(id, project_id).await.map_err(|e| {
        tracing::error!("delete_policy failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(DeleteResponse { id, deleted }))
}

/// GET /api/v1/policies/:id/versions — list policy version history
pub async fn list_policy_versions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<Vec<crate::store::postgres::PolicyVersionRow>>, StatusCode> {
    auth.require_scope("policies:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    let versions = state.db.list_policy_versions(id).await.map_err(|e| {
        tracing::error!("list_policy_versions failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(versions))
}

// ── Credential Handlers ──────────────────────────────────────

/// GET /api/v1/credentials — list credential metadata (no secrets)
pub async fn list_credentials(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CredentialMeta>>, StatusCode> {
    auth.require_scope("credentials:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let creds = state.db.list_credentials(project_id).await.map_err(|e| {
        tracing::error!("list_credentials failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(creds))
}

/// POST /api/v1/credentials — create a new encrypted credential
pub async fn create_credential(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateCredentialRequest>,
) -> Result<(StatusCode, Json<CreateCredentialResponse>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = payload.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Encrypt the secret using the vault
    let (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce) =
        state.vault.encrypt_string(&payload.secret).map_err(|e| {
            tracing::error!("credential encryption failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let injection_mode = payload
        .injection_mode
        .unwrap_or_else(|| "bearer".to_string());
    let injection_header = payload
        .injection_header
        .unwrap_or_else(|| "Authorization".to_string());

    // Validate injection mode
    match injection_mode.as_str() {
        "bearer" | "basic" | "header" | "query" => {}
        _ => {
            tracing::warn!(
                "create_credential: invalid injection_mode: {}",
                injection_mode
            );
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Validate injection header name
    if reqwest::header::HeaderName::from_bytes(injection_header.as_bytes()).is_err() {
        tracing::warn!(
            "create_credential: invalid injection_header: {}",
            injection_header
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let new_cred = crate::store::postgres::NewCredential {
        project_id,
        name: payload.name.clone(),
        provider: payload.provider,
        encrypted_dek,
        dek_nonce,
        encrypted_secret,
        secret_nonce,
        injection_mode,
        injection_header,
    };

    let id = state.db.insert_credential(&new_cred).await.map_err(|e| {
        tracing::error!("create_credential failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateCredentialResponse {
            id,
            name: payload.name,
            message: "Credential encrypted and stored".to_string(),
        }),
    ))
}

/// DELETE /api/v1/credentials/:id — soft-delete a credential
pub async fn delete_credential(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<DeleteResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    let deleted = state.db.delete_credential(id, project_id).await.map_err(|e| {
        tracing::error!("delete_credential failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(DeleteResponse { id, deleted }))
}

// ── Token Revocation Handler ─────────────────────────────────



// ── Token Usage Handler ──────────────────────────────────────

/// GET /api/v1/tokens/:id/usage — per-token usage analytics (24h)
pub async fn get_token_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::models::analytics::TokenUsageStats>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_usage(&token_id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_usage failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

// ── Audit Stream (SSE) ───────────────────────────────────────

/// GET /api/v1/audit/stream — Server-Sent Events live audit tail
pub async fn stream_audit_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // SEC: scope check (SSE handler can't return Result, so we filter silently on auth failure)
    let has_scope = auth.require_scope("audit:read").is_ok();
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    let project_ok = verify_project_ownership(&state, auth.org_id, project_id).await.is_ok();
    let authorized = has_scope && project_ok;

    let stream = stream::unfold(
        (state, project_id, None::<chrono::DateTime<chrono::Utc>>, authorized),
        |(state, project_id, last_seen, authorized)| async move {
            // Poll every 2 seconds
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            // If not authorized, just send heartbeats (no data)
            if !authorized {
                return Some((Ok(Event::default().comment("heartbeat")), (state, project_id, last_seen, authorized)));
            }

            let rows = state
                .db
                .list_audit_logs(project_id, 20, 0)
                .await
                .unwrap_or_default();

            // Filter to only new entries since last_seen
            let new_rows: Vec<&AuditLogRow> = if let Some(last) = last_seen {
                rows.iter().filter(|r| r.created_at > last).collect()
            } else {
                // First poll: send nothing, just record the cursor
                vec![]
            };

            let next_cursor = rows.first().map(|r| r.created_at).or(last_seen);

            if new_rows.is_empty() {
                // Send a heartbeat comment to keep connection alive
                Some((Ok(Event::default().comment("heartbeat")), (state, project_id, next_cursor, authorized)))
            } else {
                let data = serde_json::to_string(&new_rows).unwrap_or_default();
                Some((
                    Ok(Event::default().data(data).event("audit")),
                    (state, project_id, next_cursor, authorized),
                ))
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ── Notification Handlers ────────────────────────────────────

/// GET /api/v1/notifications — list notifications
pub async fn list_notifications(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::notification::Notification>>, StatusCode> {
    auth.require_scope("notifications:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let limit = 20;

    let notifs = state
        .db
        .list_notifications(project_id, limit)
        .await
        .map_err(|e| {
            tracing::error!("list_notifications failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(notifs))
}

/// GET /api/v1/notifications/unread — count unread
pub async fn count_unread_notifications(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("notifications:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let count = state
        .db
        .count_unread_notifications(project_id)
        .await
        .map_err(|e| {
            tracing::error!("count_unread_notifications failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({ "count": count })))
}

/// POST /api/v1/notifications/:id/read — mark as read
pub async fn mark_notification_read(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // SEC: require scope (was missing)
    auth.require_scope("notifications:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    let success = state
        .db
        .mark_notification_read(id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("mark_notification_read failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({ "success": success })))
}

/// POST /api/v1/notifications/read-all — mark all as read
pub async fn mark_all_notifications_read(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // SEC: require scope and project isolation (both were missing)
    auth.require_scope("notifications:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let success = state
        .db
        .mark_all_notifications_read(project_id)
        .await
        .map_err(|e| {
            tracing::error!("mark_all_notifications_read failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({ "success": success })))
}

// ── Service Registry ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateServiceRequest {
    pub name: String,
    pub description: Option<String>,
    pub base_url: String,
    pub service_type: Option<String>,
    pub credential_id: Option<String>,
    pub project_id: Option<Uuid>,
}

/// GET /api/v1/services — list all registered services for a project
pub async fn list_services(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::service::Service>>, StatusCode> {
    auth.require_scope("services:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let services = state
        .db
        .list_services(project_id)
        .await
        .map_err(|e| {
            tracing::error!("list_services failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(services))
}

/// POST /api/v1/services — register a new external service
pub async fn create_service(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateServiceRequest>,
) -> Result<(StatusCode, Json<crate::models::service::Service>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("services:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = payload.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // SEC: Validate base_url (SSRF protection)
    let url = reqwest::Url::parse(&payload.base_url).map_err(|_| {
        tracing::warn!("create_service: invalid base_url: {}", payload.base_url);
        StatusCode::BAD_REQUEST
    })?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Block private/reserved IPs
    if let Some(host) = url.host_str() {
        let blocked_hosts = [
            "169.254.169.254", "metadata.google.internal", "metadata.internal", "0.0.0.0",
        ];
        if blocked_hosts.contains(&host) {
            tracing::warn!("create_service: base_url targets blocked host: {}", host);
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            let is_private = match ip {
                std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
                std::net::IpAddr::V6(v6) => v6.is_loopback(),
            };
            if is_private {
                tracing::warn!("create_service: base_url targets private IP: {}", host);
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
    }

    let credential_id = if let Some(ref cid) = payload.credential_id {
        Some(Uuid::parse_str(cid).map_err(|_| StatusCode::BAD_REQUEST)?)
    } else {
        None
    };

    let svc = crate::store::postgres::NewService {
        project_id,
        name: payload.name,
        description: payload.description.unwrap_or_default(),
        base_url: payload.base_url,
        service_type: payload.service_type.unwrap_or_else(|| "generic".to_string()),
        credential_id,
    };

    let created = state
        .db
        .create_service(&svc)
        .await
        .map_err(|e| {
            tracing::error!("create_service failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// DELETE /api/v1/services/:id — unregister a service
pub async fn delete_service(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("services:write").map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state
        .db
        .delete_service(id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("delete_service failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({ "deleted": deleted })))
}

// ── API Key DTOs ─────────────────────────────────────────────

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

/// GET /api/v1/auth/keys — list API keys for the org
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKeyRow>>, StatusCode> {
    auth.require_scope("keys:manage").map_err(|_| StatusCode::FORBIDDEN)?;

    let keys = state.db.list_api_keys(auth.org_id).await.map_err(|e| {
        tracing::error!("list_api_keys failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(keys))
}

/// POST /api/v1/auth/keys — create a new API key
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), (StatusCode, Json<serde_json::Value>)> {
    auth.require_role("admin").map_err(|s| {
        (s, Json(json!({ "error": { "code": "forbidden", "message": "Admin role required" } })))
    })?;
    auth.require_scope("keys:manage").map_err(|_| {
        (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden", "message": "Insufficient permissions: requires scope 'keys:manage'" } })))
    })?;

    // P1.11: Role escalation guard — a non-admin caller cannot create an admin key
    let caller_is_admin = matches!(auth.role, crate::api::ApiKeyRole::SuperAdmin | crate::api::ApiKeyRole::Admin);
    let target_is_admin = matches!(payload.role.as_str(), "admin" | "superadmin");
    if target_is_admin && !caller_is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": { "code": "role_escalation", "message": format!("Cannot create a key with role '{}' when your role is '{:?}'. Only admin keys can create other admin keys.", payload.role, auth.role) } })),
        ));
    }

    // Generate key: ak_live_<8-char-prefix>_<32-char-hex>
    let org_prefix = &auth.org_id.to_string()[..8];
    let mut random_bytes = [0u8; 16];
    use aes_gcm::aead::OsRng;
    use rand::RngCore;
    OsRng.fill_bytes(&mut random_bytes);
    let random_hex = hex::encode(random_bytes);
    let key = format!("ak_live_{}_{}", org_prefix, random_hex);

    // Hash it
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let key_hash = hex::encode(hasher.finalize());

    let scopes = payload.scopes.unwrap_or_default();
    let scopes_json = serde_json::to_value(&scopes).unwrap();

    let id = state
        .db
        .create_api_key(
            auth.org_id,
            auth.user_id,
            &payload.name,
            &key_hash,
            org_prefix,
            &payload.role,
            scopes_json,
        )
        .await
        .map_err(|e| {
            tracing::error!("create_api_key failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to create API key" } })))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id,
            key, // Return the raw key only once!
            message: "Save this key now. It will never be shown again.".into(),
        }),
    ))
}

/// DELETE /api/v1/auth/keys/:id — revoke an API key
pub async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    auth.require_role("admin").map_err(|s| {
        (s, Json(json!({ "error": { "code": "forbidden", "message": "Admin role required" } })))
    })?;
    auth.require_scope("keys:manage").map_err(|_| {
        (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden", "message": "Insufficient permissions: requires scope 'keys:manage'" } })))
    })?;

    // P1.11: Last admin key guard — prevent revoking the last admin key
    let all_keys = state.db.list_api_keys(auth.org_id).await.map_err(|e| {
        tracing::error!("revoke_api_key: list_api_keys failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to check admin key count" } })))
    })?;
    let admin_keys: Vec<_> = all_keys.iter().filter(|k| k.role == "admin" && k.is_active).collect();
    let is_revoking_admin = admin_keys.iter().any(|k| k.id == id);
    if is_revoking_admin && admin_keys.len() <= 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "last_admin_key", "message": "Cannot revoke the last admin key. Create another admin key first to avoid losing access." } })),
        ));
    }

    let found = state.db.revoke_api_key(id, auth.org_id).await.map_err(|e| {
        tracing::error!("revoke_api_key failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to revoke API key" } })))
    })?;

    if found {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, Json(json!({ "error": { "code": "not_found", "message": "API key not found" } }))))
    }
}

/// GET /api/v1/auth/whoami — current identity
pub async fn whoami(Extension(auth): Extension<AuthContext>) -> Json<WhoAmIResponse> {
    Json(WhoAmIResponse {
        org_id: auth.org_id,
        user_id: auth.user_id,
        role: format!("{:?}", auth.role),
        scopes: auth.scopes,
    })
}

// ── Billing / Metering ───────────────────────────────────────


/// GET /api/v1/billing/usage — get current usage meter for the org
pub async fn get_org_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("billing:read").map_err(|_| StatusCode::FORBIDDEN)?;
    use chrono::{Datelike, Utc};
    let now = Utc::now();
    let period = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();

    // Try the pre-aggregated usage_meters table first
    let existing = state.db.get_usage(auth.org_id, period).await.map_err(|e| {
        tracing::error!("get_org_usage (usage_meters) failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (total_requests, total_tokens, total_spend) = if let Some(row) = existing {
        (row.total_requests, row.total_tokens_used, row.total_spend_usd)
    } else {
        // Fall back: aggregate live from audit_logs
        state.db.get_usage_from_audit_logs(auth.org_id, period).await.map_err(|e| {
            tracing::error!("get_org_usage (audit_logs fallback) failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let period_str = format!("{}-{:02}", now.year(), now.month());
    let resp = serde_json::json!({
        "org_id": auth.org_id,
        "period": period_str,
        "total_requests": total_requests,
        "total_tokens_used": total_tokens,
        "total_spend_usd": total_spend,
        "updated_at": now.to_rfc3339(),
    });

    Ok(Json(resp))
}

// ── Per-Token Analytics ──────────────────────────────────────

/// GET /api/v1/analytics/tokens — summary of all tokens
pub async fn get_token_analytics(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenSummary>>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let summary = state.db.get_token_summary(project_id).await.map_err(|e| {
        tracing::error!("get_token_analytics failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/tokens/:id/volume — hourly volume for a token
pub async fn get_token_volume(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenVolumeStat>>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_volume_24h(project_id, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_volume failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/tokens/:id/status — status distribution for a token
pub async fn get_token_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenStatusStat>>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_status_distribution_24h(project_id, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_status failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/tokens/:id/latency — latency percentiles for a token
pub async fn get_token_latency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<TokenLatencyStat>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_latency_percentiles_24h(project_id, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_latency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/health/upstreams — current status of all tracked upstreams
pub async fn get_upstream_health(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<crate::proxy::loadbalancer::UpstreamStatus>>, StatusCode> {
    auth.require_scope("system:read").map_err(|_| StatusCode::FORBIDDEN)?;
    Ok(Json(state.lb.get_all_status()))
}

/// GET /api/v1/tokens/:id/circuit-breaker — get circuit breaker config for a token
pub async fn get_circuit_breaker(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let token = state.db.get_token(&token_id).await
        .map_err(|e| {
            tracing::error!("get_circuit_breaker: db error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Return the stored config, or the default if not set
    let config = token.circuit_breaker
        .unwrap_or_else(|| serde_json::json!({
            "enabled": true,
            "failure_threshold": 3,
            "recovery_cooldown_secs": 30,
            "half_open_max_requests": 1
        }));

    Ok(Json(config))
}

/// PATCH /api/v1/tokens/:id/circuit-breaker — update circuit breaker config for a token
pub async fn update_circuit_breaker(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    auth.require_scope("tokens:write").map_err(|_| {
        (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden", "message": "tokens:write scope required" } })))
    })?;
    // P1.6: Validate the payload before deserializing to catch missing fields
    let cb_config: crate::proxy::loadbalancer::CircuitBreakerConfig =
        serde_json::from_value(payload.clone())
            .map_err(|e| {
                tracing::warn!("update_circuit_breaker: invalid config: {}", e);
                (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": { "code": "invalid_config", "message": format!("Invalid circuit breaker config: {}", e) } })))
            })?;

    // P1.6: Range validation with actionable messages
    if cb_config.failure_threshold < 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "invalid_config", "message": "failure_threshold must be >= 1. Set to 1 to open the circuit after a single failure." } })),
        ));
    }
    if cb_config.recovery_cooldown_secs < 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "invalid_config", "message": "recovery_cooldown_secs must be >= 1 (minimum 1 second before retrying an open circuit)." } })),
        ));
    }
    if cb_config.half_open_max_requests < 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "invalid_config", "message": "half_open_max_requests must be >= 1 (number of probe requests allowed in half-open state)." } })),
        ));
    }

    // Verify the token exists
    let _token = state.db.get_token(&token_id).await
        .map_err(|e| {
            tracing::error!("update_circuit_breaker: db error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Database error" } })))
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({ "error": { "code": "not_found", "message": "Token not found" } }))))?;

    let updated = state.db.update_circuit_breaker(&token_id, payload.clone()).await
        .map_err(|e| {
            tracing::error!("update_circuit_breaker: update failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to update circuit breaker config" } })))
        })?;

    if !updated {
        return Err((StatusCode::NOT_FOUND, Json(json!({ "error": { "code": "not_found", "message": "Token not found" } }))));
    }

    tracing::info!(token_id = %token_id, "circuit breaker config updated");
    Ok(Json(payload))
}

// ── Spend Cap Handlers ───────────────────────────────────────────

/// Verify that a token exists and belongs to the caller's project.
/// Returns the token's project_id on success.
async fn verify_token_ownership(
    state: &Arc<AppState>,
    token_id: &str,
    auth: &AuthContext,
) -> Result<(), StatusCode> {
    let token = state.db.get_token(token_id).await.map_err(|e| {
        tracing::error!("verify_token_ownership DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    match token {
        Some(t) if t.project_id == auth.default_project_id() => Ok(()),
        Some(_) => {
            tracing::warn!(token_id, "spend cap access denied: token belongs to different project");
            Err(StatusCode::NOT_FOUND) // Don't reveal existence to other projects
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Validate a webhook URL: must be HTTPS (or HTTP in dev), no private/reserved IPs.
fn validate_webhook_url(url_str: &str) -> Result<(), StatusCode> {
    // Must be a valid URL
    let parsed = url::Url::parse(url_str).map_err(|_| {
        tracing::warn!(url = url_str, "invalid webhook URL");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    // Scheme check
    match parsed.scheme() {
        "https" => {},
        "http" => {
            // Allow HTTP only for localhost in development
            let host = parsed.host_str().unwrap_or("");
            if host != "localhost" && host != "127.0.0.1" && host != "[::1]" {
                tracing::warn!(url = url_str, "webhook URL must use HTTPS");
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
        _ => {
            tracing::warn!(url = url_str, "webhook URL has unsupported scheme");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    // Block private/reserved hosts
    let host = parsed.host_str().unwrap_or("");
    let blocked_hosts = [
        "169.254.169.254",   // Cloud metadata
        "metadata.google.internal",
        "metadata.internal",
        "0.0.0.0",
    ];
    if blocked_hosts.contains(&host) {
        tracing::warn!(url = url_str, "webhook URL targets blocked host");
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Block common private IP ranges
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        let is_private = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local()
                    || v4.octets()[0] == 169 && v4.octets()[1] == 254 // link-local
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        };
        if is_private {
            tracing::warn!(url = url_str, "webhook URL targets private IP");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    Ok(())
}

/// GET /api/v1/tokens/:id/spend — current spend status + caps for a token
pub async fn get_spend_caps(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
) -> Result<Json<crate::middleware::spend::SpendStatus>, StatusCode> {
    // SEC-04: scope check
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-05: ownership check
    verify_token_ownership(&state, &token_id, &auth).await?;

    crate::middleware::spend::get_spend_status(state.db.pool(), &state.cache, &token_id)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("get_spend_caps failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(Deserialize)]
pub struct UpsertSpendCapRequest {
    pub period: String,      // "daily" | "monthly"
    pub limit_usd: f64,
}

/// PUT /api/v1/tokens/:id/spend — set or update a spend cap
pub async fn upsert_spend_cap(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Json(payload): Json<UpsertSpendCapRequest>,
) -> Result<StatusCode, StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-05: ownership check
    verify_token_ownership(&state, &token_id, &auth).await?;

    if payload.period != "daily" && payload.period != "monthly" && payload.period != "lifetime" {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let limit = rust_decimal::Decimal::try_from(payload.limit_usd)
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    // BUG-02: reject zero or negative limits
    if limit <= rust_decimal::Decimal::ZERO {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let project_id = auth.default_project_id();

    crate::middleware::spend::upsert_spend_cap(state.db.pool(), &token_id, project_id, &payload.period, limit)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| {
            tracing::error!("upsert_spend_cap failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// DELETE /api/v1/tokens/:id/spend/:period — remove a spend cap
pub async fn delete_spend_cap(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((token_id, period)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-05: ownership check
    verify_token_ownership(&state, &token_id, &auth).await?;

    crate::middleware::spend::delete_spend_cap(state.db.pool(), &token_id, &period)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| {
            tracing::error!("delete_spend_cap failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

// ── Webhook Handlers ─────────────────────────────────────────────

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

/// GET /api/v1/webhooks — list all webhook configs for the project
pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<WebhookRow>>, StatusCode> {
    // SEC-04: scope check
    auth.require_scope("webhooks:read").map_err(|_| StatusCode::FORBIDDEN)?;

    let project_id = auth.default_project_id();
    let rows = sqlx::query_as::<_, WebhookRow>(
        // SEC: signing_secret intentionally omitted (shown only once on creation)
        "SELECT id, project_id, url, events, is_active, created_at, NULL::text AS signing_secret FROM webhooks WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!(project_id = %project_id, error = %e, "list_webhooks query failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

/// POST /api/v1/webhooks — create a new webhook
pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<WebhookRow>), StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("webhooks:write").map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-09: validate webhook URL
    validate_webhook_url(&payload.url)?;

    let project_id = auth.default_project_id();
    let events = payload.events.unwrap_or_default();

    // Generate a 32-byte (256-bit) random signing secret shown once on creation.
    let signing_secret: String = (0..32)
        .map(|_| rand::random::<u8>())
        .fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{:02x}", b);
            acc
        });

    tracing::info!(project_id = %project_id, url = %payload.url, "creating webhook with signing secret");

    // Fetch the auto-inserted row with signing_secret included
    let row = sqlx::query_as::<_, WebhookRow>(
        r#"
        INSERT INTO webhooks (project_id, url, events, signing_secret)
        VALUES ($1, $2, $3, $4)
        RETURNING id, project_id, url, events, is_active, created_at, signing_secret
        "#,
    )
    .bind(project_id)
    .bind(&payload.url)
    .bind(&events)
    .bind(&signing_secret)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!(project_id = %project_id, error = %e, "create_webhook DB insert failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(row)))
}

/// DELETE /api/v1/webhooks/:id — remove a webhook
pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<StatusCode, StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("webhooks:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let id = uuid::Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    sqlx::query("DELETE FROM webhooks WHERE id = $1 AND project_id = $2")
        .bind(id)
        .bind(project_id)
        .execute(state.db.pool())
        .await
        .map_err(|e| {
            tracing::error!("delete_webhook failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/webhooks/test — send a test event to a URL
pub async fn test_webhook(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<TestWebhookRequest>,
) -> Result<Json<TestWebhookResponse>, StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("webhooks:write").map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-02: validate URL before making outbound request
    validate_webhook_url(&payload.url)?;

    let test_event = crate::notification::webhook::WebhookEvent::policy_violation(
        "test-token-id",
        "Test Token",
        "test-project-id",
        "test-policy",
        "This is a test webhook delivery from TrueFlow Gateway",
    );

    match state.webhook.send(&payload.url, &test_event).await {
        Ok(_) => Ok(Json(TestWebhookResponse {
            success: true,
            message: format!("Test event delivered to {}", payload.url),
        })),
        Err(e) => Ok(Json(TestWebhookResponse {
            success: false,
            message: format!("Delivery failed: {}", e),
        })),
    }
}

// ── Model Pricing Handlers ────────────────────────────────────────────────────

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

/// GET /api/v1/pricing — list all active model pricing entries
pub async fn list_pricing(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<PricingEntryResponse>>, StatusCode> {
    auth.require_scope("pricing:read").map_err(|_| StatusCode::FORBIDDEN)?;

    let rows = state.db.list_model_pricing().await.map_err(|e| {
        tracing::error!("list_pricing failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let resp = rows.into_iter().map(|r| PricingEntryResponse {
        id: r.id,
        provider: r.provider,
        model_pattern: r.model_pattern,
        input_per_m: r.input_per_m,
        output_per_m: r.output_per_m,
        is_active: r.is_active,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }).collect();

    Ok(Json(resp))
}

// ── Analytics Handlers ───────────────────────────────────────

/// GET /api/v1/analytics/summary — aggregated stats for a time range
pub async fn get_analytics_summary(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<crate::models::analytics::AnalyticsSummary>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(24)
        .clamp(1, 8760); // 1 hour minimum, 1 year maximum

    let summary = state.db.get_analytics_summary(project_id, hours).await.map_err(|e| {
        tracing::error!("get_analytics_summary failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/timeseries — timeseries data for charts
pub async fn get_analytics_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::AnalyticsTimeseriesPoint>>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(24)
        .clamp(1, 8760); // 1 hour minimum, 1 year maximum

    let points = state.db.get_analytics_timeseries(project_id, hours).await.map_err(|e| {
        tracing::error!("get_analytics_timeseries failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/experiments — A/B testing experiment data
pub async fn get_analytics_experiments(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::analytics::ExperimentSummary>>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let experiments = state.db.get_analytics_experiments(project_id).await.map_err(|e| {
        tracing::error!("get_analytics_experiments failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(experiments))
}

/// GET /api/v1/analytics/spend/breakdown?group_by=model|token|tag:KEY&hours=720
///
/// Returns spend grouped by a chosen dimension over a time window.
/// - `group_by=model`   → spend per LLM model (gpt-4o, claude-3, etc.)
/// - `group_by=token`   → spend per virtual token (agent key)
/// - `group_by=tag:team` → spend per custom tag value (from X-Properties header)
///
/// Default: group_by=model, hours=720 (30 days)
pub async fn get_spend_breakdown(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<SpendBreakdownParams>,
) -> Result<Json<SpendBreakdownResponse>, StatusCode> {
    auth.require_scope("analytics:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = params.hours.unwrap_or(720); // default: 30 days
    if hours <= 0 || hours > 8760 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let group_by = params.group_by.as_deref().unwrap_or("model");

    let (dimension_label, rows) = if group_by == "model" {
        ("model", state.db.get_spend_by_model(project_id, hours).await)
    } else if group_by == "token" {
        ("token", state.db.get_spend_by_token(project_id, hours).await)
    } else if let Some(tag_key) = group_by.strip_prefix("tag:") {
        if tag_key.is_empty() || tag_key.len() > 64 {
            return Err(StatusCode::BAD_REQUEST);
        }
        (tag_key, state.db.get_spend_by_tag(project_id, hours, tag_key).await)
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let rows = rows.map_err(|e| {
        tracing::error!("get_spend_breakdown failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total_cost_usd: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
    let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

    Ok(Json(SpendBreakdownResponse {
        group_by: group_by.to_string(),
        dimension_label: dimension_label.to_string(),
        hours,
        total_cost_usd,
        total_requests,
        breakdown: rows,
    }))
}

/// PUT /api/v1/pricing — create or update a pricing entry
pub async fn upsert_pricing(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<UpsertPricingRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("pricing:write").map_err(|_| StatusCode::FORBIDDEN)?;

    if payload.provider.is_empty() || payload.model_pattern.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // SEC: Validate model_pattern as regex to prevent ReDoS when compiled during cost lookups
    if regex::RegexBuilder::new(&payload.model_pattern)
        .size_limit(1_000_000)
        .build()
        .is_err()
    {
        tracing::warn!("upsert_pricing: invalid or too complex model_pattern regex: {}", payload.model_pattern);
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let _id = state.db.upsert_model_pricing(
        &payload.provider,
        &payload.model_pattern,
        payload.input_per_m,
        payload.output_per_m,
    ).await.map_err(|e| {
        tracing::error!("upsert_pricing failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Reload cache so cost calculations pick up the change immediately
    match state.db.list_model_pricing().await {
        Ok(rows) => {
            let entries = rows.into_iter().map(|r| crate::models::pricing_cache::PricingEntry {
                provider: r.provider,
                model_pattern: r.model_pattern,
                input_per_m: r.input_per_m,
                output_per_m: r.output_per_m,
            }).collect();
            state.pricing.reload(entries).await;
        }
        Err(e) => tracing::warn!("Failed to reload pricing cache after upsert: {}", e),
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// DELETE /api/v1/pricing/:id — soft-delete a pricing entry
pub async fn delete_pricing(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("pricing:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let deleted = state.db.delete_model_pricing(id).await.map_err(|e| {
        tracing::error!("delete_pricing failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if deleted {
        // Reload cache so cost calculations pick up the change immediately
        match state.db.list_model_pricing().await {
            Ok(rows) => {
                let entries = rows.into_iter().map(|r| crate::models::pricing_cache::PricingEntry {
                    provider: r.provider,
                    model_pattern: r.model_pattern,
                    input_per_m: r.input_per_m,
                    output_per_m: r.output_per_m,
                }).collect();
                state.pricing.reload(entries).await;
            }
            Err(e) => tracing::warn!("Failed to reload pricing cache after delete: {}", e),
        }
    }

    Ok(Json(serde_json::json!({ "id": id, "deleted": deleted })))
}

// ── System Settings Handlers ─────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<std::collections::HashMap<String, serde_json::Value>>, StatusCode> {
    auth.require_role("admin")?;
    
    let settings = state.db.get_all_system_settings().await
        .map_err(|e| {
            tracing::error!("Failed to fetch settings: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
    Ok(Json(settings))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    // SEC: allowlist of permitted setting keys — prevents arbitrary key injection
    const ALLOWED_KEYS: &[&str] = &[
        "default_rate_limit",
        "default_rate_limit_window",
        "hitl_timeout_minutes",
        "max_request_body_bytes",
        "audit_retention_days",
        "enable_response_cache",
        "enable_guardrails",
        "slack_webhook_url",
    ];

    for key in payload.settings.keys() {
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            tracing::warn!(key = %key, "update_settings: rejected unknown setting key");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    for (key, value) in payload.settings {
        state.db.set_system_setting(&key, &value, None).await
            .map_err(|e| {
                tracing::error!("Failed to update setting {}: {}", key, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }
    
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn get_cache_stats(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    let mut conn = state.cache.redis();

    // Count llm_cache:* keys via SCAN (non-blocking)
    let mut cursor: u64 = 0;
    let mut key_count: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut sample_keys: Vec<serde_json::Value> = Vec::new();

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("llm_cache:*")
            .arg("COUNT")
            .arg(200u32)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("get_cache_stats SCAN failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        for key in &keys {
            key_count += 1;
            // Estimate size via STRLEN (works on string values stored as JSON)
            let size: u64 = redis::cmd("STRLEN")
                .arg(key)
                .query_async(&mut conn)
                .await
                .unwrap_or(0u64);
            total_bytes += size;

            // Collect up to 20 sample keys with TTL info for the UI
            if sample_keys.len() < 20 {
                let ttl_secs: i64 = redis::cmd("TTL")
                    .arg(key)
                    .query_async(&mut conn)
                    .await
                    .unwrap_or(-1i64);
                // Key suffix (last 12 chars) for display
                let display_key = if key.len() > 22 {
                    format!("{}…{}", &key[..10], &key[key.len()-8..])
                } else {
                    key.clone()
                };
                sample_keys.push(serde_json::json!({
                    "key": display_key,
                    "full_key": key,
                    "size_bytes": size,
                    "ttl_secs": ttl_secs,
                }));
            }
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    // Also count other namespaces for context (non-blocking estimates)
    let spend_count: u64 = {
        let (_, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg("spend:*")
            .arg("COUNT")
            .arg(100u32)
            .query_async(&mut conn)
            .await
            .unwrap_or((0u64, vec![]));
        keys.len() as u64
    };

    let rl_count: u64 = {
        let (_, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg("rl:*")
            .arg("COUNT")
            .arg(100u32)
            .query_async(&mut conn)
            .await
            .unwrap_or((0u64, vec![]));
        keys.len() as u64
    };

    Ok(Json(serde_json::json!({
        "cache_key_count": key_count,
        "estimated_size_bytes": total_bytes,
        "default_ttl_secs": crate::proxy::response_cache::DEFAULT_CACHE_TTL_SECS,
        "max_entry_bytes": 256 * 1024,
        "cached_fields": ["model", "messages", "temperature", "max_tokens", "tools", "tool_choice"],
        "skip_conditions": ["temperature > 0.1", "stream: true", "x-trueflow-no-cache: true", "Cache-Control: no-cache/no-store"],
        "namespace_counts": {
            "llm_cache": key_count,
            "spend_tracking": spend_count,
            "rate_limits": rl_count,
        },
        "sample_entries": sample_keys,
    })))
}

pub async fn flush_cache(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    // SEC: Use targeted SCAN+DEL on the `cache:*` namespace ONLY.
    // FLUSHDB was dangerous because it also wiped spend tracking (`spend:*`),
    // rate limit state (`rl:*`), and HITL decisions (`hitl:*`), which would
    // silently reset budget enforcement and bypass rate limits.
    let mut conn = state.cache.redis();

    let mut cursor: u64 = 0;
    let mut deleted: u64 = 0;
    loop {
        // SCAN with match pattern and count hint
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("llm_cache:*")
            .arg("COUNT")
            .arg(200u32)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("flush_cache SCAN failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if !keys.is_empty() {
            let n = keys.len() as u64;
            let _: () = redis::cmd("DEL")
                .arg(keys)
                .query_async(&mut conn)
                .await
                .unwrap_or(());
            deleted += n;
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    tracing::info!(
        user_id = %auth.user_id.unwrap_or_default(),
        keys_deleted = deleted,
        "Response cache (cache:*) flushed — spend/rate-limit/HITL keys preserved"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Response cache flushed successfully",
        "keys_deleted": deleted
    })))
}

// ── PII Tokenization Vault ──────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct RehydrateRequest {
    pub tokens: Vec<String>,
}

/// POST /api/v1/pii/rehydrate — reverse PII tokens back to original values.
///
/// Requires `pii:rehydrate` scope (PCI-DSS: only authorized callers can see raw PII).
/// Every rehydration request is logged for audit compliance.
pub async fn rehydrate_pii_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<RehydrateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("pii:rehydrate").map_err(|_| StatusCode::FORBIDDEN)?;

    if payload.tokens.is_empty() {
        return Ok(Json(serde_json::json!({ "values": {} })));
    }

    // Limit batch size to prevent abuse
    if payload.tokens.len() > 100 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Create a VaultCrypto instance for decryption
    let vault = crate::vault::builtin::VaultCrypto::new(&state.config.master_key)
        .map_err(|e| {
            tracing::error!("VaultCrypto init failed in rehydrate: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let project_id = auth.default_project_id();

    let values = crate::middleware::pii_vault::rehydrate_tokens(
        state.db.pool(),
        &vault,
        &payload.tokens,
        project_id,
    )
    .await
    .map_err(|e| {
        tracing::error!("PII rehydration failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Audit log: record who rehydrated what tokens
    tracing::info!(
        user_id = ?auth.user_id,
        org_id = %auth.org_id,
        token_count = values.len(),
        "PII tokens rehydrated"
    );

    Ok(Json(serde_json::json!({
        "values": values,
        "token_count": values.len(),
    })))
}

// ── Anomaly Detection Events ─────────────────────────────────

/// GET /api/v1/anomalies — list recent anomaly velocity data per token.
///
/// Scans Redis `anomaly:tok:*` sorted sets, computes current velocity
/// vs. baseline for each token, and returns results.
pub async fn get_anomaly_events(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    let mut conn = state.cache.redis();
    let config = crate::middleware::anomaly::AnomalyConfig::default();
    let now = chrono::Utc::now().timestamp() as f64;

    // SCAN for anomaly keys
    let mut cursor: u64 = 0;
    let mut events: Vec<serde_json::Value> = Vec::new();

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("anomaly:tok:*")
            .arg("COUNT")
            .arg(200u32)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("anomaly SCAN failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        for key in &keys {
            let token_id = key.strip_prefix("anomaly:tok:").unwrap_or(key);

            // Current window velocity
            let window_start = now - config.window_secs as f64;
            let current_velocity: u64 = redis::cmd("ZCOUNT")
                .arg(key)
                .arg(window_start)
                .arg(now)
                .query_async(&mut conn)
                .await
                .unwrap_or(0);

            // Total data points for baseline
            let cutoff = now - config.baseline_secs as f64;
            let total_points: u64 = redis::cmd("ZCOUNT")
                .arg(key)
                .arg(cutoff)
                .arg(now)
                .query_async(&mut conn)
                .await
                .unwrap_or(0);

            // Simple baseline estimate: total / number of windows
            let num_windows = (config.baseline_secs / config.window_secs) as f64;
            let baseline_mean = if num_windows > 0.0 {
                total_points as f64 / num_windows
            } else {
                0.0
            };

            let threshold = baseline_mean + config.sigma_threshold * baseline_mean.sqrt();
            let is_anomalous = current_velocity as f64 > threshold && total_points >= config.min_datapoints as u64;

            events.push(serde_json::json!({
                "token_id": token_id,
                "current_velocity": current_velocity,
                "baseline_mean": (baseline_mean * 100.0).round() / 100.0,
                "threshold": (threshold * 100.0).round() / 100.0,
                "is_anomalous": is_anomalous,
                "window_secs": config.window_secs,
                "total_data_points": total_points,
            }));
        }

        cursor = next_cursor;
        if cursor == 0 || events.len() >= 100 {
            break;
        }
    }

    // Sort: anomalous first, then by velocity desc
    events.sort_by(|a, b| {
        let a_anom = a["is_anomalous"].as_bool().unwrap_or(false);
        let b_anom = b["is_anomalous"].as_bool().unwrap_or(false);
        if a_anom != b_anom {
            return b_anom.cmp(&a_anom);
        }
        let a_vel = a["current_velocity"].as_u64().unwrap_or(0);
        let b_vel = b["current_velocity"].as_u64().unwrap_or(0);
        b_vel.cmp(&a_vel)
    });

    Ok(Json(serde_json::json!({
        "events": events,
        "total": events.len(),
        "window_secs": config.window_secs,
        "sigma_threshold": config.sigma_threshold,
    })))
}

// ── Model Access Groups (RBAC Depth) ──────────────────────────

/// GET /api/v1/model-access-groups — list all model access groups for the project
pub async fn list_model_access_groups(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<crate::middleware::model_access::ModelAccessGroup>>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let rows = sqlx::query_as::<_, crate::middleware::model_access::ModelAccessGroup>(
        "SELECT * FROM model_access_groups WHERE project_id = $1 ORDER BY name"
    )
    .bind(auth.default_project_id())
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to list model access groups: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rows))
}

/// POST /api/v1/model-access-groups — create a new model access group
pub async fn create_model_access_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::model_access::ModelAccessGroup>, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body.get("name").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let description = body.get("description").and_then(|v| v.as_str());
    let models = body.get("models").ok_or(StatusCode::BAD_REQUEST)?;

    // Validate models is an array of strings
    if let Some(arr) = models.as_array() {
        for v in arr {
            if v.as_str().is_none() {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query_as::<_, crate::middleware::model_access::ModelAccessGroup>(
        r#"INSERT INTO model_access_groups (project_id, name, description, models)
           VALUES ($1, $2, $3, $4)
           RETURNING *"#
    )
    .bind(auth.default_project_id())
    .bind(name)
    .bind(description)
    .bind(models)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to create model access group: {}", e);
        if e.to_string().contains("duplicate key") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    Ok(Json(row))
}

/// PUT /api/v1/model-access-groups/:id — update a model access group
pub async fn update_model_access_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(group_id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::model_access::ModelAccessGroup>, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body.get("name").and_then(|v| v.as_str());
    let description = body.get("description").and_then(|v| v.as_str());
    let models = body.get("models");

    let row = sqlx::query_as::<_, crate::middleware::model_access::ModelAccessGroup>(
        r#"UPDATE model_access_groups SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            models = COALESCE($5, models),
            updated_at = NOW()
           WHERE id = $1 AND project_id = $2
           RETURNING *"#
    )
    .bind(group_id)
    .bind(auth.default_project_id())
    .bind(name)
    .bind(description)
    .bind(models)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to update model access group: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        Some(r) => Ok(Json(r)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// DELETE /api/v1/model-access-groups/:id — delete a model access group
pub async fn delete_model_access_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(group_id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let result = sqlx::query(
        "DELETE FROM model_access_groups WHERE id = $1 AND project_id = $2"
    )
    .bind(group_id)
    .bind(auth.default_project_id())
    .execute(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete model access group: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

// ── Teams Management (Org Hierarchy) ──────────────────────────

/// GET /api/v1/teams — list all teams for the organization
pub async fn list_teams(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<crate::middleware::teams::Team>>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;
    let rows = sqlx::query_as::<_, crate::middleware::teams::Team>(
        "SELECT * FROM teams WHERE org_id = $1 ORDER BY name"
    )
    .bind(auth.org_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to list teams: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rows))
}

/// POST /api/v1/teams — create a new team
pub async fn create_team(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::teams::Team>, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body.get("name").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let description = body.get("description").and_then(|v| v.as_str());
    let max_budget = body.get("max_budget_usd").and_then(|v| v.as_f64())
        .map(|f| rust_decimal::Decimal::from_f64_retain(f).unwrap_or_default());
    let budget_duration = body.get("budget_duration").and_then(|v| v.as_str());
    let allowed_models = body.get("allowed_models");
    let default_tags = serde_json::json!({});
    let tags = body.get("tags").unwrap_or(&default_tags);

    let row = sqlx::query_as::<_, crate::middleware::teams::Team>(
        r#"INSERT INTO teams (org_id, name, description, max_budget_usd, budget_duration, allowed_models, tags)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#
    )
    .bind(auth.org_id)
    .bind(name)
    .bind(description)
    .bind(max_budget)
    .bind(budget_duration)
    .bind(allowed_models)
    .bind(tags)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to create team: {}", e);
        if e.to_string().contains("duplicate key") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    Ok(Json(row))
}

/// PUT /api/v1/teams/:id — update a team
pub async fn update_team(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::teams::Team>, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body.get("name").and_then(|v| v.as_str());
    let description = body.get("description").and_then(|v| v.as_str());
    let max_budget = body.get("max_budget_usd").and_then(|v| v.as_f64())
        .map(|f| rust_decimal::Decimal::from_f64_retain(f).unwrap_or_default());
    let budget_duration = body.get("budget_duration").and_then(|v| v.as_str());
    let allowed_models = body.get("allowed_models");
    let tags = body.get("tags");

    let row = sqlx::query_as::<_, crate::middleware::teams::Team>(
        r#"UPDATE teams SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            max_budget_usd = COALESCE($5, max_budget_usd),
            budget_duration = COALESCE($6, budget_duration),
            allowed_models = COALESCE($7, allowed_models),
            tags = COALESCE($8, tags),
            updated_at = NOW()
           WHERE id = $1 AND org_id = $2
           RETURNING *"#
    )
    .bind(team_id)
    .bind(auth.org_id)
    .bind(name)
    .bind(description)
    .bind(max_budget)
    .bind(budget_duration)
    .bind(allowed_models)
    .bind(tags)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to update team: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        Some(r) => Ok(Json(r)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// DELETE /api/v1/teams/:id — delete a team
pub async fn delete_team(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let result = sqlx::query(
        "DELETE FROM teams WHERE id = $1 AND org_id = $2"
    )
    .bind(team_id)
    .bind(auth.org_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete team: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// GET /api/v1/teams/:id/members — list members of a team
pub async fn list_team_members(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;

    let rows = sqlx::query_as::<_, crate::middleware::teams::TeamMember>(
        "SELECT tm.* FROM team_members tm JOIN teams t ON tm.team_id = t.id WHERE tm.team_id = $1 AND t.org_id = $2"
    )
    .bind(team_id)
    .bind(auth.org_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to list team members: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let members: Vec<serde_json::Value> = rows.iter().map(|m| {
        serde_json::json!({
            "id": m.id,
            "team_id": m.team_id,
            "user_id": m.user_id,
            "role": m.role,
            "created_at": m.created_at,
        })
    }).collect();

    Ok(Json(members))
}

/// POST /api/v1/teams/:id/members — add a member to a team
pub async fn add_team_member(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::teams::TeamMember>, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let user_id: uuid::Uuid = body.get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let role = body.get("role").and_then(|v| v.as_str()).unwrap_or("member");

    // Verify team belongs to org
    let team_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM teams WHERE id = $1 AND org_id = $2)"
    )
    .bind(team_id)
    .bind(auth.org_id)
    .fetch_one(state.db.pool())
    .await
    .unwrap_or(false);

    if !team_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    let row = sqlx::query_as::<_, crate::middleware::teams::TeamMember>(
        r#"INSERT INTO team_members (team_id, user_id, role)
           VALUES ($1, $2, $3)
           RETURNING *"#
    )
    .bind(team_id)
    .bind(user_id)
    .bind(role)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("duplicate key") {
            tracing::warn!("Duplicate team member: team={}, user={}", team_id, user_id);
            StatusCode::CONFLICT
        } else if msg.contains("foreign key") || msg.contains("violates foreign key") {
            tracing::warn!("Team member FK violation (user_id not found): team={}, user={}: {}", team_id, user_id, msg);
            StatusCode::UNPROCESSABLE_ENTITY
        } else {
            tracing::error!("Failed to add team member: {}", msg);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    Ok(Json(row))
}

/// DELETE /api/v1/teams/:id/members/:user_id — remove a member from a team
pub async fn remove_team_member(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((team_id, user_id)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> Result<StatusCode, StatusCode> {
    auth.require_scope("tokens:write").map_err(|_| StatusCode::FORBIDDEN)?;

    let result = sqlx::query(
        r#"DELETE FROM team_members
           WHERE team_id = $1 AND user_id = $2
           AND team_id IN (SELECT id FROM teams WHERE org_id = $3)"#
    )
    .bind(team_id)
    .bind(user_id)
    .bind(auth.org_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to remove team member: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// GET /api/v1/teams/:id/spend — get team spend summary
pub async fn get_team_spend(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<crate::middleware::teams::TeamSpend>>, StatusCode> {
    auth.require_scope("tokens:read").map_err(|_| StatusCode::FORBIDDEN)?;

    let rows = sqlx::query_as::<_, crate::middleware::teams::TeamSpend>(
        r#"SELECT ts.* FROM team_spend ts
           JOIN teams t ON ts.team_id = t.id
           WHERE ts.team_id = $1 AND t.org_id = $2
           ORDER BY ts.period DESC LIMIT 30"#
    )
    .bind(team_id)
    .bind(auth.org_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to get team spend: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}
