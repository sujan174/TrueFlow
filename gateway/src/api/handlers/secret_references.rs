//! Secret Reference API handlers.
//!
//! These endpoints manage workspace-scoped references to external secrets
//! stored in AWS Secrets Manager, HashiCorp Vault KV, or Azure Key Vault.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use super::dtos::{
    CreateSecretReferenceDto, DeleteResponse, SecretFetchResponse, SecretReferenceFilterParams,
    SecretReferenceResponse, UpdateSecretReferenceDto,
};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::models::secret_reference::{
    CreateSecretReferenceRequest, SecretReferenceFilter, UpdateSecretReferenceRequest,
};
use crate::AppState;

/// GET /api/v1/secret-references — list secret references
pub async fn list_secret_references(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<SecretReferenceFilterParams>,
) -> Result<Json<Vec<SecretReferenceResponse>>, StatusCode> {
    auth.require_scope("credentials:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let filter = SecretReferenceFilter {
        vault_backend: params.vault_backend,
        provider: params.provider,
        is_active: params.is_active,
    };

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    let refs = state
        .db
        .list_secret_references(project_id, filter, limit, offset)
        .await
        .map_err(|e| {
            tracing::error!("list_secret_references failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(refs.into_iter().map(Into::into).collect()))
}

/// POST /api/v1/secret-references — create a new secret reference
pub async fn create_secret_reference(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateSecretReferenceDto>,
) -> Result<(StatusCode, Json<SecretReferenceResponse>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Validate injection mode if provided
    if let Some(ref mode) = payload.injection_mode {
        match mode.as_str() {
            "bearer" | "header" | "query" | "none" => {}
            _ => {
                tracing::warn!("create_secret_reference: invalid injection_mode: {}", mode);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    // Validate injection header name if provided
    if let Some(ref header) = payload.injection_header {
        if reqwest::header::HeaderName::from_bytes(header.as_bytes()).is_err() {
            tracing::warn!("create_secret_reference: invalid injection_header: {}", header);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Validate vault_backend for external vaults only
    match payload.vault_backend.as_str() {
        "aws_secrets_manager" | "hashicorp_vault" | "hashicorp_vault_kv" | "azure_key_vault" => {}
        _ => {
            tracing::warn!("create_secret_reference: invalid vault_backend: {}", payload.vault_backend);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let req = CreateSecretReferenceRequest {
        name: payload.name,
        description: payload.description,
        vault_backend: payload.vault_backend,
        external_ref: payload.external_ref,
        vault_config_id: payload.vault_config_id,
        provider: payload.provider,
        injection_mode: payload.injection_mode,
        injection_header: payload.injection_header,
        allowed_team_ids: payload.allowed_team_ids,
        allowed_user_ids: payload.allowed_user_ids,
    };

    let sr = state
        .db
        .create_secret_reference(project_id, req, auth.user_id)
        .await
        .map_err(|e| {
            tracing::error!("create_secret_reference failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::CREATED, Json(sr.into())))
}

/// GET /api/v1/secret-references/:id — get a secret reference
pub async fn get_secret_reference(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<SecretReferenceResponse>, StatusCode> {
    auth.require_scope("credentials:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let sr = state
        .db
        .get_secret_reference(id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_secret_reference failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(sr.into()))
}

/// PUT /api/v1/secret-references/:id — update a secret reference
pub async fn update_secret_reference(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(payload): Json<UpdateSecretReferenceDto>,
) -> Result<Json<SecretReferenceResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Validate injection mode if provided
    if let Some(ref mode) = payload.injection_mode {
        match mode.as_str() {
            "bearer" | "header" | "query" | "none" => {}
            _ => {
                tracing::warn!("update_secret_reference: invalid injection_mode: {}", mode);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    // Validate injection header name if provided
    if let Some(ref header) = payload.injection_header {
        if reqwest::header::HeaderName::from_bytes(header.as_bytes()).is_err() {
            tracing::warn!("update_secret_reference: invalid injection_header: {}", header);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let req = UpdateSecretReferenceRequest {
        name: payload.name,
        description: payload.description,
        external_ref: payload.external_ref,
        vault_config_id: payload.vault_config_id,
        provider: payload.provider,
        injection_mode: payload.injection_mode,
        injection_header: payload.injection_header,
        allowed_team_ids: payload.allowed_team_ids,
        allowed_user_ids: payload.allowed_user_ids,
        version: payload.version,
        is_active: payload.is_active,
    };

    let sr = state
        .db
        .update_secret_reference(id, project_id, req)
        .await
        .map_err(|e| {
            tracing::error!("update_secret_reference failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(sr.into()))
}

/// DELETE /api/v1/secret-references/:id — delete a secret reference
pub async fn delete_secret_reference(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<DeleteResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state
        .db
        .hard_delete_secret_reference(id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("delete_secret_reference failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(DeleteResponse { id, deleted }))
}

/// POST /api/v1/secret-references/:id/fetch — fetch and cache the secret value
///
/// This endpoint triggers an immediate fetch of the secret from the external vault.
/// The secret is cached in Redis for subsequent lookups.
pub async fn fetch_secret(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<SecretFetchResponse>, StatusCode> {
    auth.require_scope("credentials:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Get the secret reference
    let sr = state
        .db
        .get_secret_reference(id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("fetch_secret: get_secret_reference failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if the secret reference is active
    if !sr.is_active {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check access if allowlists are configured
    // User must be authenticated for access control
    let user_id = auth.user_id.ok_or_else(|| {
        tracing::warn!("fetch_secret: unauthenticated access attempt");
        StatusCode::UNAUTHORIZED
    })?;

    let access_result = state
        .db
        .check_secret_access(id, user_id, None)
        .await
        .map_err(|e| {
            tracing::error!("fetch_secret: check_secret_access failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match access_result {
        crate::models::secret_reference::SecretAccessResult::Granted => {}
        crate::models::secret_reference::SecretAccessResult::Denied => {
            tracing::warn!(
                secret_reference_id = %id,
                user_id = ?auth.user_id,
                "Secret access denied"
            );
            return Err(StatusCode::FORBIDDEN);
        }
        crate::models::secret_reference::SecretAccessResult::NotFound => {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // Fetch the secret from the vault using the cached retrieval
    // Note: The actual secret value is intentionally not returned in the response
    let (_secret, provider, injection_mode, injection_header) = state
        .vault
        .retrieve_credential_cached(state.db.pool(), &sr.external_ref)
        .await
        .map_err(|e| {
            tracing::error!("fetch_secret: vault fetch failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update last_accessed_at
    if let Err(e) = state.db.touch_secret_access(id).await {
        tracing::warn!("fetch_secret: failed to update last_accessed_at: {}", e);
    }

    // Cache the secret for future lookups (optional, for performance)
    let cache_key = format!("secret_ref:{}", id);
    let cached_data = serde_json::json!({
        "provider": provider,
        "injection_mode": injection_mode,
        "injection_header": injection_header,
    });
    if let Err(e) = state.cache.set(&cache_key, &cached_data, 3600).await {
        tracing::warn!("fetch_secret: failed to cache secret metadata: {}", e);
    }

    Ok(Json(SecretFetchResponse {
        reference_id: id,
        fetched_at: chrono::Utc::now(),
        message: "Secret fetched and cached successfully".to_string(),
    }))
}