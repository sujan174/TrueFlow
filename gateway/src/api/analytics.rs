use crate::api::handlers::{verify_project_ownership, PaginationParams};
use crate::api::AuthContext;
use crate::AppState;
use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

/// GET /api/v1/analytics/volume — 24h request volume bucketed by hour
pub async fn get_request_volume(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::analytics::VolumeStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_request_volume_24h(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_request_volume failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/status — 24h status code distribution
pub async fn get_status_distribution(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::analytics::StatusStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_status_code_distribution_24h(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_status_distribution failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/latency — 24h latency percentiles (P50, P90, P99)
pub async fn get_latency_percentiles(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::models::analytics::LatencyStat>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_latency_percentiles_24h(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_latency_percentiles failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/models — Model usage breakdown
pub async fn get_model_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<Vec<crate::models::analytics::ModelUsageStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = params.range.unwrap_or(24);
    let stats = state
        .db
        .get_model_usage_stats(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_model_usage failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/spend/provider — Spend by provider
pub async fn get_spend_by_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<Vec<crate::models::analytics::ProviderSpendStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = params.range.unwrap_or(24);
    let stats = state
        .db
        .get_spend_by_provider(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_spend_by_provider failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/latency/provider — Latency by provider
pub async fn get_latency_by_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<Vec<crate::models::analytics::ProviderLatencyStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params.project_id.unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = params.range.unwrap_or(24);
    let stats = state
        .db
        .get_latency_by_provider(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_latency_by_provider failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

// Query params for analytics endpoints
#[derive(Debug, serde::Deserialize)]
pub struct AnalyticsQueryParams {
    pub project_id: Option<uuid::Uuid>,
    pub range: Option<i32>, // hours
}
