use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde_json::json;
use uuid::Uuid;

use super::dtos::{
    CreatePolicyRequest, DeleteResponse, PaginationParams, PolicyResponse, UpdatePolicyRequest,
};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::store::postgres::PolicyRow;
use crate::AppState;

pub async fn list_policies(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<PolicyRow>>, StatusCode> {
    auth.require_scope("policies:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    let policies = state
        .db
        .list_policies(project_id, limit, offset)
        .await
        .map_err(|e| {
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
    let project_id = payload
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
        )
            .into_response();
    }

    // Validate phase
    if phase != "pre" && phase != "post" {
        tracing::warn!("create_policy: invalid phase: {}", phase);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("invalid phase: {}", phase) })),
        )
            .into_response();
    }

    // SEC: enforce max size on rules JSON to prevent oversized payloads clogging DB+memory
    const MAX_RULES_BYTES: usize = 64 * 1024; // 64KB
    let rules_str = payload.rules.to_string();
    if rules_str.len() > MAX_RULES_BYTES {
        tracing::warn!(
            "create_policy: rules JSON too large: {} bytes",
            rules_str.len()
        );
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": format!("rules JSON exceeds maximum size of {}KB", MAX_RULES_BYTES / 1024) })),
        ).into_response();
    }

    // Validate model patterns in routing actions
    if let Err(e) = validate_routing_actions(&payload.rules) {
        tracing::warn!("create_policy: invalid routing action: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e })),
        ).into_response();
    }

    match state
        .db
        .insert_policy(
            project_id,
            &payload.name,
            &mode,
            &phase,
            payload.rules,
            payload.retry,
        )
        .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(json!(PolicyResponse {
                id,
                name: payload.name,
                message: "Policy created".to_string(),
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("create_policy failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal server error" })),
            )
                .into_response()
        }
    }
}

/// PUT /api/v1/policies/:id — update a policy
pub async fn update_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
    Json(payload): Json<UpdatePolicyRequest>,
) -> Result<Json<PolicyResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("policies:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    // HIGH-3: Accept explicit project_id from query params
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

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

    // Validate model patterns in routing actions if rules are being updated
    if let Some(ref rules) = payload.rules {
        if let Err(e) = validate_routing_actions(rules) {
            tracing::warn!("update_policy: invalid routing action: {}", e);
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
            None, // No optimistic locking for this API endpoint
        )
        .await
        .map_err(|e| {
            tracing::error!("update_policy failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match updated {
        Ok(true) => {}
        Ok(false) => return Err(StatusCode::NOT_FOUND),
        Err(()) => return Err(StatusCode::CONFLICT), // Version mismatch
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
    Query(params): Query<PaginationParams>,
) -> Result<Json<DeleteResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("policies:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    // HIGH-3: Accept explicit project_id from query params
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state.db.delete_policy(id, project_id).await.map_err(|e| {
        tracing::error!("delete_policy failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !deleted {
        tracing::warn!(
            policy_id = %id,
            project_id = %project_id,
            "HIGH-3: Policy deletion failed - not found or cross-project access attempt"
        );
    }

    Ok(Json(DeleteResponse { id, deleted }))
}

/// GET /api/v1/policies/:id/versions — list policy version history
pub async fn list_policy_versions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<Vec<crate::store::postgres::PolicyVersionRow>>, StatusCode> {
    auth.require_scope("policies:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    // SEC-03: Enforce project isolation
    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let versions = state.db.list_policy_versions(id, project_id).await.map_err(|e| {
        tracing::error!("list_policy_versions failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(versions))
}

/// Validate model patterns in routing actions within policy rules.
/// Checks dynamic_route and conditional_route actions for valid model patterns.
fn validate_routing_actions(rules: &serde_json::Value) -> Result<(), String> {
    if let Some(arr) = rules.as_array() {
        for rule in arr {
            if let Some(actions) = rule.get("actions").and_then(|a| a.as_array()) {
                for action in actions {
                    // Check dynamic_route action
                    if let Some(pool) = action.get("pool").and_then(|p| p.as_array()) {
                        for entry in pool {
                            // Validate model field if present
                            if let Some(model) = entry.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    crate::proxy::loadbalancer::validate_model_pattern(model)?;
                                }
                            }
                        }
                    }
                    // Check conditional_route action
                    if let Some(routes) = action.get("routes").and_then(|r| r.as_array()) {
                        for route in routes {
                            // Validate model field if present
                            if let Some(model) = route.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    crate::proxy::loadbalancer::validate_model_pattern(model)?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
