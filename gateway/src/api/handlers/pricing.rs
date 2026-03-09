use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::{PricingEntryResponse, UpsertPricingRequest};
use crate::api::AuthContext;
use crate::AppState;

pub async fn list_pricing(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<PricingEntryResponse>>, StatusCode> {
    auth.require_scope("pricing:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let rows = state.db.list_model_pricing().await.map_err(|e| {
        tracing::error!("list_pricing failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let resp = rows
        .into_iter()
        .map(|r| PricingEntryResponse {
            id: r.id,
            provider: r.provider,
            model_pattern: r.model_pattern,
            input_per_m: r.input_per_m,
            output_per_m: r.output_per_m,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(Json(resp))
}

pub async fn upsert_pricing(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<UpsertPricingRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("pricing:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    if payload.provider.is_empty() || payload.model_pattern.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // SEC: Validate model_pattern as regex to prevent ReDoS when compiled during cost lookups
    if regex::RegexBuilder::new(&payload.model_pattern)
        .size_limit(1_000_000)
        .build()
        .is_err()
    {
        tracing::warn!(
            "upsert_pricing: invalid or too complex model_pattern regex: {}",
            payload.model_pattern
        );
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let _id = state
        .db
        .upsert_model_pricing(
            &payload.provider,
            &payload.model_pattern,
            payload.input_per_m,
            payload.output_per_m,
        )
        .await
        .map_err(|e| {
            tracing::error!("upsert_pricing failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Reload cache so cost calculations pick up the change immediately
    match state.db.list_model_pricing().await {
        Ok(rows) => {
            let entries = rows
                .into_iter()
                .map(|r| crate::models::pricing_cache::PricingEntry {
                    provider: r.provider,
                    model_pattern: r.model_pattern,
                    input_per_m: r.input_per_m,
                    output_per_m: r.output_per_m,
                })
                .collect();
            state.pricing.reload(entries).await;
        }
        Err(e) => tracing::warn!("Failed to reload pricing cache after upsert: {}", e),
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// DELETE /api/v1/pricing/:id — soft-delete a pricing entry
pub async fn delete_pricing(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("pricing:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let deleted = state.db.delete_model_pricing(id).await.map_err(|e| {
        tracing::error!("delete_pricing failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if deleted {
        // Reload cache so cost calculations pick up the change immediately
        match state.db.list_model_pricing().await {
            Ok(rows) => {
                let entries = rows
                    .into_iter()
                    .map(|r| crate::models::pricing_cache::PricingEntry {
                        provider: r.provider,
                        model_pattern: r.model_pattern,
                        input_per_m: r.input_per_m,
                        output_per_m: r.output_per_m,
                    })
                    .collect();
                state.pricing.reload(entries).await;
            }
            Err(e) => tracing::warn!("Failed to reload pricing cache after delete: {}", e),
        }
    }

    Ok(Json(serde_json::json!({ "id": id, "deleted": deleted })))
}
