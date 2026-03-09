use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde_json::json;
use uuid::Uuid;

use super::dtos::PaginationParams;
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::AppState;

pub async fn list_notifications(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::notification::Notification>>, StatusCode> {
    auth.require_scope("notifications:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("notifications:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("notifications:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
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
    auth.require_scope("notifications:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
