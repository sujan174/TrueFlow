use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::{PaginationParams, SetSpendCapRequest, UpdateSessionStatusRequest};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::AppState;

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::store::postgres::SessionSummaryRow>, StatusCode> {
    auth.require_scope("audit:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("audit:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("sessions:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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

/// PUT /api/v1/sessions/:session_id/spend-cap — set session-level budget.
pub async fn set_session_spend_cap(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
    Json(payload): Json<SetSpendCapRequest>,
) -> Result<Json<crate::store::postgres::SessionEntity>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("sessions:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("audit:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
