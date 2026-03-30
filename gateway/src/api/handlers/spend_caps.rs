use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::UpsertSpendCapRequest;
use super::helpers::verify_token_ownership;
use crate::api::AuthContext;
use crate::errors::AppError;
use crate::AppState;

/// GET /api/v1/tokens/:id/spend — current spend status + caps for a token
pub async fn get_spend_caps(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
) -> Result<Json<crate::middleware::spend::SpendStatus>, AppError> {
    // SEC-04: scope check
    auth.require_scope("tokens:read")
        .map_err(|_| AppError::Forbidden("tokens:read scope required".to_string()))?;
    // SEC-05: ownership check
    verify_token_ownership(&state, &token_id, &auth).await?;

    crate::middleware::spend::get_spend_status(state.db.pool(), &state.cache, &token_id)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("get_spend_caps failed: {}", e);
            AppError::Internal(e)
        })
}

/// PUT /api/v1/tokens/:id/spend — set or update a spend cap
pub async fn upsert_spend_cap(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Json(payload): Json<UpsertSpendCapRequest>,
) -> Result<StatusCode, AppError> {
    // SEC-04: scope check
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;
    // SEC-05: ownership check
    verify_token_ownership(&state, &token_id, &auth).await?;

    if payload.period != "daily" && payload.period != "monthly" && payload.period != "lifetime" {
        return Err(AppError::ValidationError {
            message: "period must be 'daily', 'monthly', or 'lifetime'".to_string(),
        });
    }
    let limit = rust_decimal::Decimal::try_from(payload.limit_usd)
        .map_err(|_| AppError::ValidationError {
            message: "invalid limit_usd value".to_string(),
        })?;
    // BUG-02: reject zero or negative limits
    if limit <= rust_decimal::Decimal::ZERO {
        return Err(AppError::ValidationError {
            message: "limit_usd must be greater than 0".to_string(),
        });
    }
    let project_id = auth.default_project_id();

    crate::middleware::spend::upsert_spend_cap(
        &state.cache,
        state.db.pool(),
        &token_id,
        project_id,
        &payload.period,
        limit,
    )
    .await
    .map(|_| StatusCode::NO_CONTENT)
    .map_err(|e| {
        tracing::error!("upsert_spend_cap failed: {}", e);
        AppError::Internal(e)
    })
}

/// DELETE /api/v1/tokens/:id/spend/:period — remove a spend cap
pub async fn delete_spend_cap(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((token_id, period)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    // SEC-04: scope check
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("tokens:write")
        .map_err(|_| AppError::Forbidden("tokens:write scope required".to_string()))?;
    // SEC-05: ownership check
    verify_token_ownership(&state, &token_id, &auth).await?;

    crate::middleware::spend::delete_spend_cap(&state.cache, state.db.pool(), &token_id, &period)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| {
            tracing::error!("delete_spend_cap failed: {}", e);
            AppError::Internal(e)
        })
}