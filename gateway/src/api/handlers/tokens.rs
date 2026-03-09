use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde_json::json;

use super::dtos::{CreateTokenRequest, CreateTokenResponse, PaginationParams};
use super::helpers::{verify_project_ownership, verify_token_ownership};
use crate::api::AuthContext;
use crate::store::postgres::TokenRow;
use crate::AppState;

pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenRow>>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    let tokens = state
        .db
        .list_tokens(project_id, limit, offset)
        .await
        .map_err(|e| {
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
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = payload
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    // Verify the token belongs to the org by looking it up first
    let token = state.db.get_token(&id).await.map_err(|e| {
        tracing::error!("revoke_token lookup failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if let Some(ref t) = token {
        verify_project_ownership(&state, auth.org_id, t.project_id).await?;
    }

    let revoked = state
        .db
        .revoke_token(
            &id,
            token.as_ref().map(|t| t.project_id).unwrap_or_default(),
        )
        .await
        .map_err(|e| {
            tracing::error!("revoke_token failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn get_token_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::models::analytics::TokenUsageStats>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
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

pub async fn get_circuit_breaker(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    verify_token_ownership(&state, &token_id, &auth).await?;
    let token = state
        .db
        .get_token(&token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_circuit_breaker: db error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Return the stored config, or the default if not set
    let config = token.circuit_breaker.unwrap_or_else(|| {
        serde_json::json!({
            "enabled": true,
            "failure_threshold": 3,
            "recovery_cooldown_secs": 30,
            "half_open_max_requests": 1
        })
    });

    Ok(Json(config))
}

pub async fn update_circuit_breaker(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    auth.require_scope("tokens:write").map_err(|_| {
        (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden", "message": "tokens:write scope required" } })))
    })?;
    verify_token_ownership(&state, &token_id, &auth)
        .await
        .map_err(|status| {
            (
                status,
                Json(json!({ "error": { "code": "not_found", "message": "Token not found" } })),
            )
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
            Json(
                json!({ "error": { "code": "invalid_config", "message": "failure_threshold must be >= 1. Set to 1 to open the circuit after a single failure." } }),
            ),
        ));
    }
    if cb_config.recovery_cooldown_secs < 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({ "error": { "code": "invalid_config", "message": "recovery_cooldown_secs must be >= 1 (minimum 1 second before retrying an open circuit)." } }),
            ),
        ));
    }
    if cb_config.half_open_max_requests < 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({ "error": { "code": "invalid_config", "message": "half_open_max_requests must be >= 1 (number of probe requests allowed in half-open state)." } }),
            ),
        ));
    }

    // Verify the token exists
    let _token = state.db.get_token(&token_id).await
        .map_err(|e| {
            tracing::error!("update_circuit_breaker: db error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Database error" } })))
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({ "error": { "code": "not_found", "message": "Token not found" } }))))?;

    let updated = state.db.update_circuit_breaker(&token_id, _token.project_id, payload.clone()).await
        .map_err(|e| {
            tracing::error!("update_circuit_breaker: update failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to update circuit breaker config" } })))
        })?;

    if !updated {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "Token not found" } })),
        ));
    }

    tracing::info!(token_id = %token_id, "circuit breaker config updated");
    Ok(Json(payload))
}
