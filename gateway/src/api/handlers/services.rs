use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde_json::json;
use uuid::Uuid;

use super::dtos::{CreateServiceRequest, PaginationParams};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::AppState;

pub async fn list_services(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::service::Service>>, StatusCode> {
    auth.require_scope("services:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    let services = state
        .db
        .list_services(project_id, limit, offset)
        .await
        .map_err(|e| {
            tracing::error!("list_services failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(services))
}

/// POST /api/v1/services — register a new external service
pub async fn create_service(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateServiceRequest>,
) -> Result<(StatusCode, Json<crate::models::service::Service>), StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("services:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = payload
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // SEC: Validate base_url (SSRF protection)
    let url = reqwest::Url::parse(&payload.base_url).map_err(|_| {
        tracing::warn!("create_service: invalid base_url: {}", payload.base_url);
        StatusCode::BAD_REQUEST
    })?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Block private/reserved IPs
    if let Some(host) = url.host_str() {
        let blocked_hosts = [
            "169.254.169.254",
            "metadata.google.internal",
            "metadata.internal",
            "0.0.0.0",
        ];
        if blocked_hosts.contains(&host) {
            tracing::warn!("create_service: base_url targets blocked host: {}", host);
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            let is_private = match ip {
                std::net::IpAddr::V4(v4) => {
                    v4.is_loopback() || v4.is_private() || v4.is_link_local()
                }
                std::net::IpAddr::V6(v6) => v6.is_loopback(),
            };
            if is_private {
                tracing::warn!("create_service: base_url targets private IP: {}", host);
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
    }

    let credential_id = if let Some(ref cid) = payload.credential_id {
        Some(Uuid::parse_str(cid).map_err(|_| StatusCode::BAD_REQUEST)?)
    } else {
        None
    };

    let svc = crate::store::postgres::NewService {
        project_id,
        name: payload.name,
        description: payload.description.unwrap_or_default(),
        base_url: payload.base_url,
        service_type: payload
            .service_type
            .unwrap_or_else(|| "generic".to_string()),
        credential_id,
    };

    let created = state.db.create_service(&svc).await.map_err(|e| {
        tracing::error!("create_service failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// DELETE /api/v1/services/:id — unregister a service
pub async fn delete_service(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("services:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state.db.delete_service(id, project_id).await.map_err(|e| {
        tracing::error!("delete_service failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "deleted": deleted })))
}
