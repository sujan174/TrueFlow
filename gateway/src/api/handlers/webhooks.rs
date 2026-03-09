use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::{CreateWebhookRequest, TestWebhookRequest, TestWebhookResponse, WebhookRow};
use super::helpers::validate_webhook_url;
use crate::api::AuthContext;
use crate::AppState;

pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<WebhookRow>>, StatusCode> {
    // SEC-04: scope check
    auth.require_scope("webhooks:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let project_id = auth.default_project_id();
    let rows = sqlx::query_as::<_, WebhookRow>(
        // SEC: signing_secret intentionally omitted (shown only once on creation)
        "SELECT id, project_id, url, events, is_active, created_at, NULL::text AS signing_secret FROM webhooks WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!(project_id = %project_id, error = %e, "list_webhooks query failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

/// POST /api/v1/webhooks — create a new webhook
pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<WebhookRow>), StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("webhooks:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-09: validate webhook URL
    validate_webhook_url(&payload.url)?;

    let project_id = auth.default_project_id();
    let events = payload.events.unwrap_or_default();

    // Generate a 32-byte (256-bit) random signing secret shown once on creation.
    let signing_secret: String =
        (0..32)
            .map(|_| rand::random::<u8>())
            .fold(String::with_capacity(64), |mut acc, b| {
                use std::fmt::Write;
                let _ = write!(acc, "{:02x}", b);
                acc
            });

    tracing::info!(project_id = %project_id, url = %payload.url, "creating webhook with signing secret");

    // Fetch the auto-inserted row with signing_secret included
    let row = sqlx::query_as::<_, WebhookRow>(
        r#"
        INSERT INTO webhooks (project_id, url, events, signing_secret)
        VALUES ($1, $2, $3, $4)
        RETURNING id, project_id, url, events, is_active, created_at, signing_secret
        "#,
    )
    .bind(project_id)
    .bind(&payload.url)
    .bind(&events)
    .bind(&signing_secret)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!(project_id = %project_id, error = %e, "create_webhook DB insert failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(row)))
}

/// DELETE /api/v1/webhooks/:id — remove a webhook
pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<StatusCode, StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("webhooks:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let id = uuid::Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    sqlx::query("DELETE FROM webhooks WHERE id = $1 AND project_id = $2")
        .bind(id)
        .bind(project_id)
        .execute(state.db.pool())
        .await
        .map_err(|e| {
            tracing::error!("delete_webhook failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/webhooks/test — send a test event to a URL
pub async fn test_webhook(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<TestWebhookRequest>,
) -> Result<Json<TestWebhookResponse>, StatusCode> {
    // SEC-04: scope check
    auth.require_role("admin")?;
    auth.require_scope("webhooks:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    // SEC-02: validate URL before making outbound request
    validate_webhook_url(&payload.url)?;

    let test_event = crate::notification::webhook::WebhookEvent::policy_violation(
        "test-token-id",
        "Test Token",
        "test-project-id",
        "test-policy",
        "This is a test webhook delivery from TrueFlow Gateway",
    );

    match state.webhook.send(&payload.url, &test_event).await {
        Ok(_) => Ok(Json(TestWebhookResponse {
            success: true,
            message: format!("Test event delivered to {}", payload.url),
        })),
        Err(e) => Ok(Json(TestWebhookResponse {
            success: false,
            message: format!("Delivery failed: {}", e),
        })),
    }
}
