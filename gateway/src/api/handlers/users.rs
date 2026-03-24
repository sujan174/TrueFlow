use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    Extension, Json,
};
use serde_json::json;

use crate::store::postgres::types::{SyncUserRequest, SyncUserResponse, UserRow};
use crate::AppState;

/// POST /api/v1/auth/sync-user
///
/// Syncs a user from Supabase Auth to the gateway database.
/// Called by the dashboard after successful Supabase login.
///
/// This endpoint requires SuperAdmin authentication (X-Admin-Key)
/// because the dashboard validates the Supabase session separately.
pub async fn sync_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SyncUserRequest>,
) -> Result<(StatusCode, Json<SyncUserResponse>), (StatusCode, Json<serde_json::Value>)> {
    // Validate email is not empty
    if payload.email.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_email",
                    "message": "Email address is required"
                }
            })),
        ));
    }

    // Sync user from Supabase
    let response = state
        .db
        .sync_user_from_supabase(payload)
        .await
        .map_err(|e| {
            tracing::error!("sync_user_from_supabase failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "internal_error",
                        "message": "Failed to sync user"
                    }
                })),
            )
        })?;

    let status = if response.is_new_user {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((status, Json(response)))
}

/// GET /api/v1/users
///
/// List all users in the current organization.
/// Requires admin role.
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<crate::api::AuthContext>,
) -> Result<Json<Vec<UserRow>>, StatusCode> {
    auth.require_role("admin").map_err(|_| StatusCode::FORBIDDEN)?;

    let users = state.db.list_users_by_org(auth.org_id).await.map_err(|e| {
        tracing::error!("list_users failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(users))
}

/// GET /api/v1/users/:id
///
/// Get a specific user by ID.
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<crate::api::AuthContext>,
    axum::extract::Path(user_id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<UserRow>, (StatusCode, Json<serde_json::Value>)> {
    let user = state
        .db
        .get_user_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!("get_user failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": { "code": "internal_error", "message": "Failed to get user" } })),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "User not found" } })),
        ))?;

    // Verify user belongs to caller's org
    if user.org_id != auth.org_id {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "User not found" } })),
        ));
    }

    Ok(Json(user))
}

/// PATCH /api/v1/users/:id/role
///
/// Update a user's role.
/// Requires admin role.
pub async fn update_user_role(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<crate::api::AuthContext>,
    axum::extract::Path(user_id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    auth.require_role("admin").map_err(|s| {
        (
            s,
            Json(json!({ "error": { "code": "forbidden", "message": "Admin role required" } })),
        )
    })?;

    // Validate role
    if !["admin", "member", "viewer"].contains(&payload.role.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_role",
                    "message": "Role must be 'admin', 'member', or 'viewer'"
                }
            })),
        ));
    }

    // Verify user exists and belongs to caller's org
    let user = state
        .db
        .get_user_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!("get_user failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": { "code": "internal_error", "message": "Failed to get user" } })),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "User not found" } })),
        ))?;

    if user.org_id != auth.org_id {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "User not found" } })),
        ));
    }

    // Update role
    let updated = state
        .db
        .update_user_role(user_id, &payload.role)
        .await
        .map_err(|e| {
            tracing::error!("update_user_role failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": { "code": "internal_error", "message": "Failed to update user role" } })),
            )
        })?;

    if !updated {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "User not found" } })),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role: String,
}

/// PUT /api/v1/users/me/last-project
///
/// Update the current user's last used project.
/// This is used for cross-device project persistence.
pub async fn update_last_project(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<crate::api::AuthContext>,
    Json(payload): Json<UpdateLastProjectRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    // Verify the project exists and belongs to user's org
    let belongs = state
        .db
        .project_belongs_to_org(payload.project_id, auth.org_id)
        .await
        .map_err(|e| {
            tracing::error!("project_belongs_to_org failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": { "code": "internal_error", "message": "Failed to verify project" } })),
            )
        })?;

    if !belongs {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "Project not found" } })),
        ));
    }

    // Get user_id - for SuperAdmin, we need to look up by org_id
    let user_id = match auth.user_id {
        Some(id) => id,
        None => {
            // For SuperAdmin, try to find a user in this org
            // This is a fallback for admin key usage
            let users = state.db.list_users_by_org(auth.org_id).await.map_err(|e| {
                tracing::error!("list_users_by_org failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": { "code": "internal_error", "message": "Failed to find user" } })),
                )
            })?;
            match users.first() {
                Some(u) => u.id,
                None => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(json!({ "error": { "code": "not_found", "message": "No user found for this organization" } })),
                    ));
                }
            }
        }
    };

    // Update user's last project
    state
        .db
        .update_user_last_project(user_id, payload.project_id)
        .await
        .map_err(|e| {
            tracing::error!("update_user_last_project failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": { "code": "internal_error", "message": "Failed to update last project" } })),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateLastProjectRequest {
    pub project_id: uuid::Uuid,
}

/// GET /api/v1/users/me
///
/// Get the current authenticated user's data.
/// Returns user info including last_project_id for project persistence.
pub async fn get_current_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<crate::api::AuthContext>,
) -> Result<Json<UserRow>, (StatusCode, Json<serde_json::Value>)> {
    // Get user_id from auth context
    let user_id = match auth.user_id {
        Some(id) => id,
        None => {
            // For SuperAdmin (no user_id), try to find a user in this org
            let users = state.db.list_users_by_org(auth.org_id).await.map_err(|e| {
                tracing::error!("list_users_by_org failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": { "code": "internal_error", "message": "Failed to find user" } })),
                )
            })?;
            match users.first() {
                Some(u) => u.id,
                None => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(json!({ "error": { "code": "not_found", "message": "No user found for this organization" } })),
                    ));
                }
            }
        }
    };

    let user = state
        .db
        .get_user_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!("get_user failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": { "code": "internal_error", "message": "Failed to get user" } })),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "User not found" } })),
        ))?;

    Ok(Json(user))
}