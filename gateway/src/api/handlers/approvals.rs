use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use super::dtos::{DecisionRequest, DecisionResponse, PaginationParams};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::models::approval::ApprovalStatus;
use crate::AppState;

pub async fn list_approvals(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::approval::ApprovalRequest>>, StatusCode> {
    auth.require_scope("approvals:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("approvals:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

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
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
        let _: redis::RedisResult<i64> =
            redis::AsyncCommands::lpush(&mut redis_conn, &hitl_key, status_str).await;
        // Set a short TTL so the key doesn't linger if the gateway crashed
        let _: redis::RedisResult<bool> =
            redis::AsyncCommands::expire(&mut redis_conn, &hitl_key, 60_i64).await;
    }

    Ok(Json(DecisionResponse {
        id,
        status: status_str.to_string(),
        updated,
    }))
}
