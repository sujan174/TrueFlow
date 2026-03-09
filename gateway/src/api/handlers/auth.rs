use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde_json::json;
use uuid::Uuid;

use super::dtos::{CreateApiKeyRequest, CreateApiKeyResponse, WhoAmIResponse};
use crate::api::AuthContext;
use crate::store::postgres::ApiKeyRow;
use crate::AppState;

pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKeyRow>>, StatusCode> {
    auth.require_scope("keys:manage")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let keys = state.db.list_api_keys(auth.org_id).await.map_err(|e| {
        tracing::error!("list_api_keys failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(keys))
}

/// POST /api/v1/auth/keys — create a new API key
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), (StatusCode, Json<serde_json::Value>)> {
    auth.require_role("admin").map_err(|s| {
        (
            s,
            Json(json!({ "error": { "code": "forbidden", "message": "Admin role required" } })),
        )
    })?;
    auth.require_scope("keys:manage").map_err(|_| {
        (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden", "message": "Insufficient permissions: requires scope 'keys:manage'" } })))
    })?;

    // P1.11: Role escalation guard — a non-admin caller cannot create an admin key
    let caller_is_admin = matches!(
        auth.role,
        crate::api::ApiKeyRole::SuperAdmin | crate::api::ApiKeyRole::Admin
    );
    let target_is_admin = matches!(payload.role.as_str(), "admin" | "superadmin");
    if target_is_admin && !caller_is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(
                json!({ "error": { "code": "role_escalation", "message": format!("Cannot create a key with role '{}' when your role is '{:?}'. Only admin keys can create other admin keys.", payload.role, auth.role) } }),
            ),
        ));
    }

    // Generate key: ak_live_<8-char-prefix>_<32-char-hex>
    let org_prefix = &auth.org_id.to_string()[..8];
    let mut random_bytes = [0u8; 16];
    use aes_gcm::aead::OsRng;
    use rand::RngCore;
    OsRng.fill_bytes(&mut random_bytes);
    let random_hex = hex::encode(random_bytes);
    let key = format!("ak_live_{}_{}", org_prefix, random_hex);

    // Hash it
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let key_hash = hex::encode(hasher.finalize());

    let scopes = payload.scopes.unwrap_or_default();
    let scopes_json = serde_json::to_value(&scopes).unwrap();

    let id = state
        .db
        .create_api_key(
            auth.org_id,
            auth.user_id,
            &payload.name,
            &key_hash,
            org_prefix,
            &payload.role,
            scopes_json,
        )
        .await
        .map_err(|e| {
            tracing::error!("create_api_key failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to create API key" } })))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id,
            key, // Return the raw key only once!
            message: "Save this key now. It will never be shown again.".into(),
        }),
    ))
}

/// DELETE /api/v1/auth/keys/:id — revoke an API key
pub async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    auth.require_role("admin").map_err(|s| {
        (
            s,
            Json(json!({ "error": { "code": "forbidden", "message": "Admin role required" } })),
        )
    })?;
    auth.require_scope("keys:manage").map_err(|_| {
        (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden", "message": "Insufficient permissions: requires scope 'keys:manage'" } })))
    })?;

    // P1.11: Last admin key guard — prevent revoking the last admin key
    let all_keys = state.db.list_api_keys(auth.org_id).await.map_err(|e| {
        tracing::error!("revoke_api_key: list_api_keys failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to check admin key count" } })))
    })?;
    let admin_keys: Vec<_> = all_keys
        .iter()
        .filter(|k| k.role == "admin" && k.is_active)
        .collect();
    let is_revoking_admin = admin_keys.iter().any(|k| k.id == id);
    if is_revoking_admin && admin_keys.len() <= 1 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({ "error": { "code": "last_admin_key", "message": "Cannot revoke the last admin key. Create another admin key first to avoid losing access." } }),
            ),
        ));
    }

    let found = state.db.revoke_api_key(id, auth.org_id).await.map_err(|e| {
        tracing::error!("revoke_api_key failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": { "code": "internal_server_error", "message": "Failed to revoke API key" } })))
    })?;

    if found {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "API key not found" } })),
        ))
    }
}

/// GET /api/v1/auth/whoami — current identity
pub async fn whoami(Extension(auth): Extension<AuthContext>) -> Json<WhoAmIResponse> {
    Json(WhoAmIResponse {
        org_id: auth.org_id,
        user_id: auth.user_id,
        role: format!("{:?}", auth.role),
        scopes: auth.scopes,
    })
}
