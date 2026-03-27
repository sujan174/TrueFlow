use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};

use crate::api::AuthContext;
use crate::AppState;

pub async fn list_model_access_groups(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<crate::middleware::model_access::ModelAccessGroup>>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let rows = sqlx::query_as::<_, crate::middleware::model_access::ModelAccessGroup>(
        "SELECT * FROM model_access_groups WHERE project_id = $1 ORDER BY name",
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
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
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

    // Validate model patterns are valid
    if let Some(arr) = models.as_array() {
        for v in arr {
            if let Some(pattern) = v.as_str() {
                if let Err(e) = crate::proxy::loadbalancer::validate_model_pattern(pattern) {
                    tracing::warn!("create_model_access_group: invalid model pattern: {}", e);
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
        }
    }

    let row = sqlx::query_as::<_, crate::middleware::model_access::ModelAccessGroup>(
        r#"INSERT INTO model_access_groups (project_id, name, description, models)
           VALUES ($1, $2, $3, $4)
           RETURNING *"#,
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
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body.get("name").and_then(|v| v.as_str());
    let description = body.get("description").and_then(|v| v.as_str());
    let models = body.get("models");

    // Validate model patterns if models is being updated
    if let Some(models_json) = models {
        if let Some(arr) = models_json.as_array() {
            for v in arr {
                if let Some(pattern) = v.as_str() {
                    if let Err(e) = crate::proxy::loadbalancer::validate_model_pattern(pattern) {
                        tracing::warn!("update_model_access_group: invalid model pattern: {}", e);
                        return Err(StatusCode::BAD_REQUEST);
                    }
                }
            }
        }
    }

    let row = sqlx::query_as::<_, crate::middleware::model_access::ModelAccessGroup>(
        r#"UPDATE model_access_groups SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            models = COALESCE($5, models),
            updated_at = NOW()
           WHERE id = $1 AND project_id = $2
           RETURNING *"#,
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
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let result = sqlx::query("DELETE FROM model_access_groups WHERE id = $1 AND project_id = $2")
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
