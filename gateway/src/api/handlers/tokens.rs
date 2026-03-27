use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde_json::json;

use super::dtos::{CreateTokenRequest, CreateTokenResponse, PaginationParams};
use super::dtos::{BulkCreateTokenRequest, BulkCreateTokenResponse, BulkRevokeRequest, BulkRevokeResponse, BulkTokenFailure};
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

    // Use filtered query if filters are provided
    let tokens = if params.external_user_id.is_some() || params.team_id.is_some() {
        state
            .db
            .list_tokens_by_filter(
                project_id,
                params.external_user_id.as_deref(),
                params.team_id,
                limit,
                offset,
            )
            .await
            .map_err(|e| {
                tracing::error!("list_tokens_by_filter failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    } else {
        state
            .db
            .list_tokens(project_id, limit, offset)
            .await
            .map_err(|e| {
                tracing::error!("list_tokens failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    };

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

    // SSRF protection: Block URLs that resolve to private/internal IPs
    // unless explicitly allowed via environment variable (for dev/testing)
    let allow_private_upstreams =
        std::env::var("TRUEFLOW_ALLOW_PRIVATE_UPSTREAMS").is_ok();
    if !allow_private_upstreams {
        if !crate::utils::is_safe_webhook_url(&payload.upstream_url).await {
            tracing::warn!(
                "create_token: upstream URL blocked by SSRF protection: {}",
                payload.upstream_url
            );
            return Err(StatusCode::BAD_REQUEST);
        }
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
        // Validate allowed_models patterns in upstreams
        for upstream in upstreams {
            if let Some(ref patterns) = upstream.allowed_models {
                if let Err(e) = crate::proxy::loadbalancer::validate_model_patterns(patterns) {
                    tracing::warn!("create_token: invalid model pattern: {}", e);
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
        }
    }

    // Validate allowed_models patterns on token
    if let Some(ref patterns_json) = payload.allowed_models {
        if let Some(arr) = patterns_json.as_array() {
            for v in arr {
                if let Some(pattern) = v.as_str() {
                    if let Err(e) = crate::proxy::loadbalancer::validate_model_pattern(pattern) {
                        tracing::warn!("create_token: invalid allowed_models pattern: {}", e);
                        return Err(StatusCode::BAD_REQUEST);
                    }
                }
            }
        }
    }

    // BYOK (Passthrough) mode validation: single provider only
    // When credential_id is None, the token operates in passthrough mode
    // which only supports a single provider (set via upstream_url)
    if payload.credential_id.is_none() {
        // Check upstreams - should be None or have exactly one entry
        if let Some(ref upstreams) = payload.upstreams {
            if upstreams.len() > 1 {
                tracing::warn!(
                    "create_token: BYOK token cannot have multiple upstreams (got {})",
                    upstreams.len()
                );
                return Err(StatusCode::BAD_REQUEST);
            }
        }

        // Check allowed_providers - should be None or have exactly one entry
        if let Some(ref providers) = payload.allowed_providers {
            if providers.len() > 1 {
                tracing::warn!(
                    "create_token: BYOK token cannot have multiple allowed_providers (got {})",
                    providers.len()
                );
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    // Validate that policies' routing targets are within token's allowed scope
    if let Some(ref policy_uuids) = payload.policy_ids {
        if !policy_uuids.is_empty() {
            let policies = state
                .db
                .get_policies_for_token(project_id, policy_uuids)
                .await
                .map_err(|e| {
                    tracing::error!("create_token: failed to load policies: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            if let Err(e) = crate::middleware::policy_scope::validate_policies_against_token_scope(
                &policies,
                payload.allowed_providers.as_deref(),
                payload.allowed_models.as_ref(),
            ) {
                tracing::warn!("create_token: policy-token scope validation failed: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }
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
        allowed_providers: payload.allowed_providers,
        team_id: payload.team_id,
        tags: payload.tags,
        mcp_allowed_tools: payload.mcp_allowed_tools,
        mcp_blocked_tools: payload.mcp_blocked_tools,
        external_user_id: payload.external_user_id,
        metadata: payload.metadata,
        purpose: payload.purpose.clone(),
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
        // Clean up load balancer state to prevent memory leaks
        state.lb.cleanup_token(&id);
        // Clean up round-robin counter to prevent memory leak
        crate::proxy::smart_router::cleanup_round_robin_counter(&id);
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

// ── Bulk Token Operations (SaaS Builder Support) ─────────────────────────────

/// POST /api/v1/tokens/bulk — create multiple tokens at once.
/// Maximum 500 tokens per request to prevent abuse.
pub async fn bulk_create_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<BulkCreateTokenRequest>,
) -> Result<(StatusCode, Json<BulkCreateTokenResponse>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    // Limit bulk size to prevent abuse
    if payload.tokens.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if payload.tokens.len() > 500 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let mut created = Vec::new();
    let mut failed = Vec::new();
    let total_requested = payload.tokens.len();

    for token_req in payload.tokens {
        let proj = token_req.project_id.unwrap_or(project_id);
        let proj_short = &proj.to_string()[..8];
        let mut random_bytes = [0u8; 16];
        use aes_gcm::aead::OsRng;
        use rand::RngCore;
        OsRng.fill_bytes(&mut random_bytes);
        let token_id = format!("tf_v1_{}_tok_{}", proj_short, hex::encode(random_bytes));

        let name = token_req.name.clone();

        // Validate allowed_models patterns
        let mut validation_error: Option<String> = None;
        if let Some(ref patterns_json) = token_req.allowed_models {
            if let Some(arr) = patterns_json.as_array() {
                for v in arr {
                    if let Some(pattern) = v.as_str() {
                        if let Err(e) = crate::proxy::loadbalancer::validate_model_pattern(pattern) {
                            validation_error = Some(format!("Invalid allowed_models pattern: {}", e));
                            break;
                        }
                    }
                }
            }
        }
        // Validate upstreams allowed_models
        if validation_error.is_none() {
            if let Some(ref upstreams) = token_req.upstreams {
                for upstream in upstreams {
                    if let Some(ref patterns) = upstream.allowed_models {
                        if let Err(e) = crate::proxy::loadbalancer::validate_model_patterns(patterns) {
                            validation_error = Some(format!("Invalid upstream allowed_models: {}", e));
                            break;
                        }
                    }
                }
            }
        }

        if let Some(err) = validation_error {
            failed.push(BulkTokenFailure {
                name,
                error: err,
            });
            continue;
        }

        // Validate that policies' routing targets are within token's allowed scope
        if let Some(ref policy_uuids) = token_req.policy_ids {
            if !policy_uuids.is_empty() {
                match state.db.get_policies_for_token(proj, policy_uuids).await {
                    Ok(policies) => {
                        if let Err(e) = crate::middleware::policy_scope::validate_policies_against_token_scope(
                            &policies,
                            token_req.allowed_providers.as_deref(),
                            token_req.allowed_models.as_ref(),
                        ) {
                            validation_error = Some(format!("Policy-token scope validation failed: {}", e));
                        }
                    }
                    Err(e) => {
                        validation_error = Some(format!("Failed to load policies: {}", e));
                    }
                }
            }
        }

        if let Some(err) = validation_error {
            failed.push(BulkTokenFailure {
                name,
                error: err,
            });
            continue;
        }

        let new_token = crate::store::postgres::NewToken {
            id: token_id.clone(),
            project_id: proj,
            name: token_req.name.clone(),
            credential_id: token_req.credential_id,
            upstream_url: token_req.upstream_url.clone(),
            scopes: serde_json::json!([]),
            policy_ids: token_req.policy_ids.clone().unwrap_or_default(),
            log_level: token_req.resolved_log_level(),
            circuit_breaker: token_req.circuit_breaker.clone(),
            allowed_models: token_req.allowed_models.clone(),
            allowed_providers: token_req.allowed_providers.clone(),
            team_id: token_req.team_id,
            tags: token_req.tags.clone(),
            mcp_allowed_tools: token_req.mcp_allowed_tools.clone(),
            mcp_blocked_tools: token_req.mcp_blocked_tools.clone(),
            external_user_id: token_req.external_user_id.clone(),
            metadata: token_req.metadata.clone(),
            purpose: token_req.purpose.clone(),
        };

        match state.db.insert_token(&new_token).await {
            Ok(()) => {
                created.push(CreateTokenResponse {
                    token_id: token_id.clone(),
                    name,
                    message: format!("Use: Authorization: Bearer {}", token_id),
                });
            }
            Err(e) => {
                tracing::warn!("bulk_create_tokens: failed to create token: {}", e);
                failed.push(BulkTokenFailure {
                    name,
                    error: e.to_string(),
                });
            }
        }
    }

    let total_created = created.len();

    Ok((
        if created.is_empty() {
            StatusCode::BAD_REQUEST
        } else if failed.is_empty() {
            StatusCode::CREATED
        } else {
            StatusCode::MULTI_STATUS
        },
        Json(BulkCreateTokenResponse {
            created,
            failed,
            total_requested,
            total_created,
        }),
    ))
}

/// POST /api/v1/tokens/bulk-revoke — revoke tokens by filter criteria.
/// At least one filter must be provided (external_user_id, team_id, or token_ids).
pub async fn bulk_revoke_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<BulkRevokeRequest>,
) -> Result<Json<BulkRevokeResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    // Require at least one filter to prevent accidental mass revocation
    if payload.external_user_id.is_none()
        && payload.team_id.is_none()
        && payload.token_ids.is_none()
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let revoked_ids = state
        .db
        .bulk_revoke_tokens(
            project_id,
            payload.external_user_id.as_deref(),
            payload.team_id,
            payload.token_ids.as_deref(),
        )
        .await
        .map_err(|e| {
            tracing::error!("bulk_revoke_tokens failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Clean up load balancer state for revoked tokens
    for id in &revoked_ids {
        state.lb.cleanup_token(id);
        crate::proxy::smart_router::cleanup_round_robin_counter(id);
    }

    tracing::info!(
        count = revoked_ids.len(),
        "bulk token revocation completed"
    );

    Ok(Json(BulkRevokeResponse {
        revoked_count: revoked_ids.len(),
        token_ids: revoked_ids,
    }))
}
