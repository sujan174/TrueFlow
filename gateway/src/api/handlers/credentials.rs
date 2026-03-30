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
use crate::errors::AppError;
use crate::store::postgres::CredentialMeta;
use crate::vault::VaultBackend;
use crate::AppState;

pub async fn list_credentials(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CredentialMeta>>, AppError> {
    auth.require_scope("credentials:read")
        .map_err(|_| AppError::Forbidden("credentials:read scope required".to_string()))?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let creds = state.db.list_credentials(project_id).await?;

    Ok(Json(creds))
}

/// POST /api/v1/credentials — create a new encrypted credential
///
/// Supports two modes:
/// 1. Builtin vault (default): Provide `secret`, TrueFlow encrypts and stores it
/// 2. External vault: Provide `vault_backend` and `encrypted_secret_ref`
pub async fn create_credential(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateCredentialRequest>,
) -> Result<(StatusCode, Json<CreateCredentialResponse>), AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("credentials:write")
        .map_err(|_| AppError::Forbidden("credentials:write scope required".to_string()))?;
    let project_id = payload
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Parse vault backend
    let vault_backend = match &payload.vault_backend {
        Some(backend_str) => backend_str.parse::<VaultBackend>().map_err(|e| {
            tracing::warn!("create_credential: invalid vault_backend: {}", e);
            AppError::ValidationError {
                message: format!("Invalid vault_backend: {}", e),
            }
        })?,
        None => VaultBackend::Builtin,
    };

    let injection_mode = payload
        .injection_mode
        .clone()
        .unwrap_or_else(|| "bearer".to_string());
    let injection_header = payload
        .injection_header
        .clone()
        .unwrap_or_else(|| "Authorization".to_string());

    // Validate injection mode
    match injection_mode.as_str() {
        "bearer" | "basic" | "header" | "query" | "sigv4" => {}
        _ => {
            tracing::warn!(
                "create_credential: invalid injection_mode: {}",
                injection_mode
            );
            return Err(AppError::ValidationError {
                message: format!("Invalid injection_mode: {}. Must be one of: bearer, basic, header, query, sigv4", injection_mode),
            });
        }
    }

    // Validate injection header name
    if reqwest::header::HeaderName::from_bytes(injection_header.as_bytes()).is_err() {
        tracing::warn!(
            "create_credential: invalid injection_header: {}",
            injection_header
        );
        return Err(AppError::ValidationError {
            message: format!("Invalid injection_header: {}", injection_header),
        });
    }

    // Handle credential creation based on vault backend
    let (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce, external_vault_ref) =
        match vault_backend {
            VaultBackend::Builtin => {
                // Builtin vault: encrypt the plaintext secret
                let secret = payload.secret.as_ref().ok_or_else(|| {
                    tracing::warn!("create_credential: secret required for builtin vault");
                    AppError::ValidationError {
                        message: "secret is required for builtin vault".to_string(),
                    }
                })?;

                let (enc_dek, dek_nonce, enc_secret, secret_nonce) =
                    state.vault.encrypt_string(secret)?;

                (
                    Some(enc_dek),
                    Some(dek_nonce),
                    Some(enc_secret),
                    Some(secret_nonce),
                    None,
                )
            }
            VaultBackend::AwsKms | VaultBackend::HashicorpVault => {
                // External vault: store the pre-encrypted reference
                let encrypted_ref = payload.encrypted_secret_ref.as_ref().ok_or_else(|| {
                    tracing::warn!(
                        "create_credential: encrypted_secret_ref required for external vault"
                    );
                    AppError::ValidationError {
                        message: "encrypted_secret_ref is required for external vault".to_string(),
                    }
                })?;

                // Warn if secret is also provided (shouldn't be for external vault)
                if payload.secret.is_some() {
                    tracing::warn!(
                        "create_credential: secret provided for external vault, it will be ignored"
                    );
                }

                (None, None, None, None, Some(encrypted_ref.clone()))
            }
            VaultBackend::AwsSecretsManager => {
                // AWS Secrets Manager: store the secret ARN
                let secret_arn = payload.encrypted_secret_ref.as_ref().ok_or_else(|| {
                    tracing::warn!(
                        "create_credential: secret_arn required for AWS Secrets Manager"
                    );
                    AppError::ValidationError {
                        message: "secret_arn is required for AWS Secrets Manager".to_string(),
                    }
                })?;

                // Validate ARN format
                if !secret_arn.starts_with("arn:aws:secretsmanager:") {
                    tracing::warn!("create_credential: invalid Secrets Manager ARN format");
                    return Err(AppError::ValidationError {
                        message: "Invalid Secrets Manager ARN format".to_string(),
                    });
                }

                (None, None, None, None, Some(secret_arn.clone()))
            }
            VaultBackend::HashicorpVaultKv | VaultBackend::AzureKeyVault | VaultBackend::GcpSecretManager => {
                // Future backends: not yet implemented
                tracing::warn!(
                    "create_credential: vault backend {:?} not yet implemented",
                    vault_backend
                );
                return Err(AppError::ValidationError {
                    message: format!("Vault backend {:?} not yet implemented", vault_backend),
                });
            }
        };

    let new_cred = crate::store::postgres::NewCredential {
        project_id,
        name: payload.name.clone(),
        provider: payload.provider.clone(),
        encrypted_dek,
        dek_nonce,
        encrypted_secret,
        secret_nonce,
        external_vault_ref,
        vault_backend,
        injection_mode,
        injection_header,
    };

    let id = state.db.insert_credential(&new_cred).await?;

    let message = match vault_backend {
        VaultBackend::Builtin => "Credential encrypted and stored".to_string(),
        VaultBackend::AwsKms => "Credential stored with AWS KMS reference".to_string(),
        VaultBackend::AwsSecretsManager => {
            "Credential stored with AWS Secrets Manager reference".to_string()
        }
        VaultBackend::HashicorpVault => {
            "Credential stored with HashiCorp Vault reference".to_string()
        }
        VaultBackend::HashicorpVaultKv | VaultBackend::AzureKeyVault | VaultBackend::GcpSecretManager => {
            "Credential stored with external vault reference".to_string()
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateCredentialResponse {
            id,
            name: payload.name,
            message,
        }),
    ))
}

/// DELETE /api/v1/credentials/:id — soft-delete a credential
pub async fn delete_credential(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<DeleteResponse>, AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("credentials:write")
        .map_err(|_| AppError::Forbidden("credentials:write scope required".to_string()))?;
    let id = Uuid::parse_str(&id_str).map_err(|_| AppError::ValidationError {
        message: format!("Invalid credential ID: {}", id_str),
    })?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    // HIGH-2: Verify project ownership for explicit isolation
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state.db.delete_credential(id, project_id).await?;

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