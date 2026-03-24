use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use super::dtos::{CreateProjectRequest, ProjectResponse};
use crate::api::{ApiKeyRole, AuthContext};
use crate::AppState;

pub async fn list_projects(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ProjectResponse>>, StatusCode> {
    auth.require_scope("projects:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
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
    auth.require_scope("projects:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

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
    auth.require_scope("projects:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
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

    // Check how many projects exist in the org
    let projects = state.db.list_projects(auth.org_id).await.map_err(|e| {
        tracing::error!("list_projects failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Prevent deleting the last project - users must always have at least one
    if projects.len() <= 1 {
        tracing::warn!("attempt to delete last project prevented");
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

    let project_id = Uuid::parse_str(&id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid project id" })),
        )
    })?;

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
