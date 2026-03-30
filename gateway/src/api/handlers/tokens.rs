use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::{
    BulkCreateTokenRequest, BulkCreateTokenResponse, BulkRevokeRequest, BulkRevokeResponse,
    BulkTokenFailure,
};
use super::dtos::{CreateTokenRequest, CreateTokenResponse, PaginationParams};
use super::helpers::{verify_project_ownership, verify_token_ownership};
use crate::api::AuthContext;
use crate::errors::AppError;
use crate::store::postgres::TokenRow;
use crate::AppState;

/// Detect provider from upstream URL hostname.
fn detect_provider_from_url(url: &str) -> Option<String> {
    let url_lower = url.to_lowercase();

    if url_lower.contains("api.openai.com") || url_lower.contains("openai.com") {
        return Some("openai".to_string());
    }
    if url_lower.contains("api.anthropic.com") || url_lower.contains("anthropic.com") {
        return Some("anthropic".to_string());
    }
    if url_lower.contains("generativelanguage.googleapis.com") || url_lower.contains("googleapis.com") {
        return Some("google".to_string());
    }
    if url_lower.contains("api.groq.com") {
        return Some("groq".to_string());
    }
    if url_lower.contains("api.mistral.ai") {
        return Some("mistral".to_string());
    }
    if url_lower.contains("api.cohere.ai") || url_lower.contains("cohere.ai") {
        return Some("cohere".to_string());
    }
    if url_lower.contains("api.together.xyz") {
        return Some("together".to_string());
    }
    if url_lower.contains("openrouter.ai") {
        return Some("openrouter".to_string());
    }
    if url_lower.contains("localhost") || url_lower.contains("127.0.0.1") {
        return Some("ollama".to_string());
    }
    // Azure and Bedrock have varied URLs
    if url_lower.contains("azure.com") || url_lower.contains("azure.net") {
        return Some("azure".to_string());
    }
    if url_lower.contains("amazonaws.com") || url_lower.contains("bedrock") {
        return Some("bedrock".to_string());
    }

    // Unknown URL - could be custom
    None
}

pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenRow>>, AppError> {
    auth.require_scope("tokens:read")
        .map_err(|_| AppError::Forbidden("tokens:read scope required".to_string()))?;
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
            .await?
    } else {
        state.db.list_tokens(project_id, limit, offset).await?
    };

    Ok(Json(tokens))
}

/// POST /api/v1/tokens — create a new virtual token
pub async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreateTokenResponse>), AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;
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
        AppError::ValidationError {
            message: format!("Invalid upstream URL: {}", payload.upstream_url),
        }
    })?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(AppError::ValidationError {
            message: "URL scheme must be http or https".to_string(),
        });
    }

    // SSRF protection: Block URLs that resolve to private/internal IPs
    // unless explicitly allowed via environment variable (for dev/testing)
    let allow_private_upstreams = std::env::var("TRUEFLOW_ALLOW_PRIVATE_UPSTREAMS").is_ok();
    if !allow_private_upstreams {
        if !crate::utils::is_safe_webhook_url(&payload.upstream_url).await {
            tracing::warn!(
                "create_token: upstream URL blocked by SSRF protection: {}",
                payload.upstream_url
            );
            return Err(AppError::ValidationError {
                message: format!(
                    "Upstream URL blocked by SSRF protection: {}. Set TRUEFLOW_ALLOW_PRIVATE_UPSTREAMS=1 to allow.",
                    payload.upstream_url
                ),
            });
        }
    }

    // P1.4: Validate upstreams list if provided (weight > 0, no duplicate URLs)
    if let Some(ref upstreams) = payload.upstreams {
        // Check for zero/negative weights
        for u in upstreams {
            if u.weight == 0 {
                return Err(AppError::ValidationError {
                    message: "Upstream weight must be greater than 0".to_string(),
                });
            }
        }
        // Check for duplicate URLs
        let urls: Vec<_> = upstreams.iter().map(|u| u.url.as_str()).collect();
        let unique: std::collections::HashSet<_> = urls.iter().copied().collect();
        if unique.len() != urls.len() {
            return Err(AppError::ValidationError {
                message: "Duplicate upstream URLs detected".to_string(),
            });
        }
        // Validate allowed_models patterns in upstreams
        for upstream in upstreams {
            if let Some(ref patterns) = upstream.allowed_models {
                if let Err(e) = crate::proxy::loadbalancer::validate_model_patterns(patterns) {
                    tracing::warn!("create_token: invalid model pattern: {}", e);
                    return Err(AppError::ValidationError {
                        message: format!("Invalid model pattern: {}", e),
                    });
                }
            }
        }
    }

    // Validate that upstreams match allowed_providers
    if let Some(ref upstreams) = payload.upstreams {
        if let Some(ref allowed) = payload.allowed_providers {
            for upstream in upstreams {
                if let Some(upstream_provider) = detect_provider_from_url(&upstream.url) {
                    let allowed_lower: Vec<String> =
                        allowed.iter().map(|p| p.to_lowercase()).collect();
                    if !allowed_lower.contains(&upstream_provider.to_lowercase()) {
                        return Err(AppError::UpstreamProviderMismatch {
                            upstream_provider,
                            allowed_providers: allowed.clone(),
                        });
                    }
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
                        return Err(AppError::ValidationError {
                            message: format!("Invalid allowed_models pattern: {}", e),
                        });
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
                return Err(AppError::ValidationError {
                    message: format!(
                        "BYOK token cannot have multiple upstreams (got {})",
                        upstreams.len()
                    ),
                });
            }
        }

        // Check allowed_providers - should be None or have exactly one entry
        if let Some(ref providers) = payload.allowed_providers {
            if providers.len() > 1 {
                tracing::warn!(
                    "create_token: BYOK token cannot have multiple allowed_providers (got {})",
                    providers.len()
                );
                return Err(AppError::ValidationError {
                    message: format!(
                        "BYOK token cannot have multiple allowed_providers (got {})",
                        providers.len()
                    ),
                });
            }
        }
    }

    // Note: Policy binding validation now happens at policy creation time
    // Policies are created separately and bound to tokens via token_id

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

    state.db.insert_token(&new_token).await?;

    // Record token creation metric (Task 36)
    crate::middleware::metrics::record_token_created(payload.credential_id.is_some());

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
) -> Result<StatusCode, AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;
    // Verify the token belongs to the org by looking it up first
    let token = state.db.get_token(&id).await?;
    if let Some(ref t) = token {
        verify_project_ownership(&state, auth.org_id, t.project_id).await?;
    }

    let revoked = state
        .db
        .revoke_token(
            &id,
            token.as_ref().map(|t| t.project_id).unwrap_or_default(),
        )
        .await?;

    if revoked {
        // Clean up load balancer state to prevent memory leaks
        state.lb.cleanup_token(&id);
        // Clean up round-robin counter to prevent memory leak
        crate::proxy::smart_router::cleanup_round_robin_counter(&id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("Token {}", id)))
    }
}

pub async fn get_token_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::models::analytics::TokenUsageStats>, AppError> {
    auth.require_scope("tokens:read")
        .map_err(|_| AppError::Forbidden("tokens:read scope required".to_string()))?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state.db.get_token_usage(&token_id, project_id).await?;

    Ok(Json(stats))
}

pub async fn get_circuit_breaker(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_scope("tokens:read")
        .map_err(|_| AppError::Forbidden("tokens:read scope required".to_string()))?;
    verify_token_ownership(&state, &token_id, &auth).await?;
    let token = state
        .db
        .get_token(&token_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Token {}", token_id)))?;

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
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;
    verify_token_ownership(&state, &token_id, &auth).await?;
    // P1.6: Validate the payload before deserializing to catch missing fields
    let cb_config: crate::proxy::loadbalancer::CircuitBreakerConfig =
        serde_json::from_value(payload.clone()).map_err(|e| {
            tracing::warn!("update_circuit_breaker: invalid config: {}", e);
            AppError::ValidationError {
                message: format!("Invalid circuit breaker config: {}", e),
            }
        })?;

    // P1.6: Range validation with actionable messages
    if cb_config.failure_threshold < 1 {
        return Err(AppError::ValidationError {
            message: "failure_threshold must be >= 1. Set to 1 to open the circuit after a single failure.".to_string(),
        });
    }
    if cb_config.recovery_cooldown_secs < 1 {
        return Err(AppError::ValidationError {
            message: "recovery_cooldown_secs must be >= 1 (minimum 1 second before retrying an open circuit).".to_string(),
        });
    }
    if cb_config.half_open_max_requests < 1 {
        return Err(AppError::ValidationError {
            message: "half_open_max_requests must be >= 1 (number of probe requests allowed in half-open state).".to_string(),
        });
    }

    // Verify the token exists
    let _token = state
        .db
        .get_token(&token_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Token {}", token_id)))?;

    let updated = state
        .db
        .update_circuit_breaker(&token_id, _token.project_id, payload.clone())
        .await?;

    if !updated {
        return Err(AppError::NotFound(format!("Token {}", token_id)));
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
) -> Result<(StatusCode, Json<BulkCreateTokenResponse>), AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;

    // Limit bulk size to prevent abuse
    if payload.tokens.is_empty() {
        return Err(AppError::ValidationError {
            message: "No tokens provided in bulk request".to_string(),
        });
    }
    if payload.tokens.len() > 500 {
        return Err(AppError::PayloadTooLarge);
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
                        if let Err(e) = crate::proxy::loadbalancer::validate_model_pattern(pattern)
                        {
                            validation_error =
                                Some(format!("Invalid allowed_models pattern: {}", e));
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
                        if let Err(e) =
                            crate::proxy::loadbalancer::validate_model_patterns(patterns)
                        {
                            validation_error =
                                Some(format!("Invalid upstream allowed_models: {}", e));
                            break;
                        }
                    }
                }
            }
        }

        if let Some(err) = validation_error {
            failed.push(BulkTokenFailure { name, error: err });
            continue;
        }

        // Note: Policy binding validation now happens at policy creation time
        // Policies are created separately and bound to tokens via token_id

        let new_token = crate::store::postgres::NewToken {
            id: token_id.clone(),
            project_id: proj,
            name: token_req.name.clone(),
            credential_id: token_req.credential_id,
            upstream_url: token_req.upstream_url.clone(),
            scopes: serde_json::json!([]),
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
) -> Result<Json<BulkRevokeResponse>, AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;

    // Require at least one filter to prevent accidental mass revocation
    if payload.external_user_id.is_none()
        && payload.team_id.is_none()
        && payload.token_ids.is_none()
    {
        return Err(AppError::ValidationError {
            message: "At least one filter (external_user_id, team_id, or token_ids) is required"
                .to_string(),
        });
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
        .await?;

    // Clean up load balancer state for revoked tokens
    for id in &revoked_ids {
        state.lb.cleanup_token(id);
        crate::proxy::smart_router::cleanup_round_robin_counter(id);
    }

    tracing::info!(count = revoked_ids.len(), "bulk token revocation completed");

    Ok(Json(BulkRevokeResponse {
        revoked_count: revoked_ids.len(),
        token_ids: revoked_ids,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_provider_from_url() {
        // OpenAI
        assert_eq!(
            detect_provider_from_url("https://api.openai.com/v1"),
            Some("openai".to_string())
        );
        assert_eq!(
            detect_provider_from_url("https://openai.com/v1/chat/completions"),
            Some("openai".to_string())
        );

        // Anthropic
        assert_eq!(
            detect_provider_from_url("https://api.anthropic.com/v1"),
            Some("anthropic".to_string())
        );
        assert_eq!(
            detect_provider_from_url("https://anthropic.com/v1/messages"),
            Some("anthropic".to_string())
        );

        // Google
        assert_eq!(
            detect_provider_from_url("https://generativelanguage.googleapis.com/v1"),
            Some("google".to_string())
        );

        // Groq
        assert_eq!(
            detect_provider_from_url("https://api.groq.com/openai/v1"),
            Some("groq".to_string())
        );

        // Mistral
        assert_eq!(
            detect_provider_from_url("https://api.mistral.ai/v1"),
            Some("mistral".to_string())
        );

        // Cohere
        assert_eq!(
            detect_provider_from_url("https://api.cohere.ai/v1"),
            Some("cohere".to_string())
        );

        // Together
        assert_eq!(
            detect_provider_from_url("https://api.together.xyz/v1"),
            Some("together".to_string())
        );

        // OpenRouter
        assert_eq!(
            detect_provider_from_url("https://openrouter.ai/api/v1"),
            Some("openrouter".to_string())
        );

        // Ollama (localhost)
        assert_eq!(
            detect_provider_from_url("http://localhost:11434/v1"),
            Some("ollama".to_string())
        );
        assert_eq!(
            detect_provider_from_url("http://127.0.0.1:11434/v1"),
            Some("ollama".to_string())
        );

        // Azure
        assert_eq!(
            detect_provider_from_url("https://my-resource.openai.azure.com/"),
            Some("azure".to_string())
        );

        // Bedrock
        assert_eq!(
            detect_provider_from_url("https://bedrock-runtime.us-east-1.amazonaws.com"),
            Some("bedrock".to_string())
        );

        // Unknown/custom URL
        assert_eq!(detect_provider_from_url("https://custom.api.com/v1"), None);
        assert_eq!(detect_provider_from_url("https://unknown.provider.io/v1"), None);
    }

    #[test]
    fn test_detect_provider_from_url_case_insensitive() {
        assert_eq!(
            detect_provider_from_url("HTTPS://API.OPENAI.COM/V1"),
            Some("openai".to_string())
        );
        assert_eq!(
            detect_provider_from_url("https://Api.Anthropic.COM/v1"),
            Some("anthropic".to_string())
        );
    }
}
