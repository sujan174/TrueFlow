use std::sync::Arc;

use axum::http::StatusCode;
use uuid::Uuid;

use crate::api::AuthContext;
use crate::AppState;

/// Verify that `project_id` belongs to `org_id`.
/// SEC-05: Returns `Err(NOT_FOUND)` instead of FORBIDDEN to prevent ID enumeration attacks.
/// Attackers should not be able to distinguish between "project doesn't exist" and
/// "project belongs to another org" - both cases should return NOT_FOUND.
pub async fn verify_project_ownership(
    state: &crate::AppState,
    org_id: Uuid,
    project_id: Uuid,
) -> Result<(), StatusCode> {
    let belongs = state
        .db
        .project_belongs_to_org(project_id, org_id)
        .await
        .map_err(|e| {
            tracing::error!("project_belongs_to_org failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !belongs {
        tracing::warn!(
            org_id = %org_id,
            project_id = %project_id,
            "SEC-05: project isolation - project not found or does not belong to org"
        );
        // Return NOT_FOUND to prevent ID enumeration attacks
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(())
}

pub(super) async fn verify_token_ownership(
    state: &Arc<AppState>,
    token_id: &str,
    auth: &AuthContext,
) -> Result<(), StatusCode> {
    let token = state.db.get_token(token_id).await.map_err(|e| {
        tracing::error!("verify_token_ownership DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    match token {
        Some(t) if t.project_id == auth.default_project_id() => Ok(()),
        Some(_) => {
            tracing::warn!(
                token_id,
                "spend cap access denied: token belongs to different project"
            );
            Err(StatusCode::NOT_FOUND) // Don't reveal existence to other projects
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Validate a webhook URL: must be HTTPS (or HTTP in dev), no private/reserved IPs.
pub(super) fn validate_webhook_url(url_str: &str) -> Result<(), StatusCode> {
    // Must be a valid URL
    let parsed = url::Url::parse(url_str).map_err(|_| {
        tracing::warn!(url = url_str, "invalid webhook URL");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    // Scheme check
    match parsed.scheme() {
        "https" => {}
        "http" => {
            // Allow HTTP only for localhost in development
            let host = parsed.host_str().unwrap_or("");
            if host != "localhost" && host != "127.0.0.1" && host != "[::1]" {
                tracing::warn!(url = url_str, "webhook URL must use HTTPS");
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
        _ => {
            tracing::warn!(url = url_str, "webhook URL has unsupported scheme");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    // Block private/reserved hosts
    let host = parsed.host_str().unwrap_or("");
    let blocked_hosts = [
        "169.254.169.254", // Cloud metadata
        "metadata.google.internal",
        "metadata.internal",
        "0.0.0.0",
    ];
    if blocked_hosts.contains(&host) {
        tracing::warn!(url = url_str, "webhook URL targets blocked host");
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Block common private IP ranges
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        let is_private = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.octets()[0] == 169 && v4.octets()[1] == 254 // link-local
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        };
        if is_private {
            tracing::warn!(url = url_str, "webhook URL targets private IP");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    Ok(())
}
