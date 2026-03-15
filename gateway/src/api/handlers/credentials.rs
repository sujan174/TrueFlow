use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use super::dtos::{
    CreateCredentialRequest, CreateCredentialResponse, DeleteResponse, PaginationParams,
};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::store::postgres::CredentialMeta;
use crate::AppState;

pub async fn list_credentials(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CredentialMeta>>, StatusCode> {
    auth.require_scope("credentials:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let creds = state.db.list_credentials(project_id).await.map_err(|e| {
        tracing::error!("list_credentials failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(creds))
}

/// POST /api/v1/credentials — create a new encrypted credential
pub async fn create_credential(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateCredentialRequest>,
) -> Result<(StatusCode, Json<CreateCredentialResponse>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = payload
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Encrypt the secret using the vault
    let (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce) =
        state.vault.encrypt_string(&payload.secret).map_err(|e| {
            tracing::error!("credential encryption failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let injection_mode = payload
        .injection_mode
        .unwrap_or_else(|| "bearer".to_string());
    let injection_header = payload
        .injection_header
        .unwrap_or_else(|| "Authorization".to_string());

    // Validate injection mode
    match injection_mode.as_str() {
        "bearer" | "basic" | "header" | "query" => {}
        _ => {
            tracing::warn!(
                "create_credential: invalid injection_mode: {}",
                injection_mode
            );
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Validate injection header name
    if reqwest::header::HeaderName::from_bytes(injection_header.as_bytes()).is_err() {
        tracing::warn!(
            "create_credential: invalid injection_header: {}",
            injection_header
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let new_cred = crate::store::postgres::NewCredential {
        project_id,
        name: payload.name.clone(),
        provider: payload.provider,
        encrypted_dek,
        dek_nonce,
        encrypted_secret,
        secret_nonce,
        injection_mode,
        injection_header,
    };

    let id = state.db.insert_credential(&new_cred).await.map_err(|e| {
        tracing::error!("create_credential failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateCredentialResponse {
            id,
            name: payload.name,
            message: "Credential encrypted and stored".to_string(),
        }),
    ))
}

/// DELETE /api/v1/credentials/:id — soft-delete a credential
pub async fn delete_credential(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<DeleteResponse>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("credentials:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    // HIGH-2: Verify project ownership for explicit isolation
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state
        .db
        .delete_credential(id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("delete_credential failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // HIGH-11: Invalidate credential cache on delete
    if deleted {
        let cache_key = format!("credential:{}", id);
        if let Err(e) = state.cache.invalidate(&cache_key).await {
            tracing::warn!(
                credential_id = %id,
                error = %e,
                "HIGH-11: Failed to invalidate credential cache on delete - cache will expire naturally"
            );
        }
    }

    if !deleted {
        tracing::warn!(
            credential_id = %id,
            project_id = %project_id,
            "HIGH-2: Credential deletion failed - not found or cross-project access attempt"
        );
    }

    Ok(Json(DeleteResponse { id, deleted }))
}
