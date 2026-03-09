use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::{PaginationParams, SpendBreakdownParams, SpendBreakdownResponse};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::store::postgres::{TokenLatencyStat, TokenStatusStat, TokenSummary, TokenVolumeStat};
use crate::AppState;

pub async fn get_org_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("billing:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    use chrono::{Datelike, Utc};
    let now = Utc::now();
    let period = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();

    // Try the pre-aggregated usage_meters table first
    let existing = state.db.get_usage(auth.org_id, period).await.map_err(|e| {
        tracing::error!("get_org_usage (usage_meters) failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (total_requests, total_tokens, total_spend) = if let Some(row) = existing {
        (
            row.total_requests,
            row.total_tokens_used,
            row.total_spend_usd,
        )
    } else {
        // Fall back: aggregate live from audit_logs
        state
            .db
            .get_usage_from_audit_logs(auth.org_id, period)
            .await
            .map_err(|e| {
                tracing::error!("get_org_usage (audit_logs fallback) failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    };

    let period_str = format!("{}-{:02}", now.year(), now.month());
    let resp = serde_json::json!({
        "org_id": auth.org_id,
        "period": period_str,
        "total_requests": total_requests,
        "total_tokens_used": total_tokens,
        "total_spend_usd": total_spend,
        "updated_at": now.to_rfc3339(),
    });

    Ok(Json(resp))
}

// ── Per-Token Analytics ──────────────────────────────────────

/// GET /api/v1/analytics/tokens — summary of all tokens
pub async fn get_token_analytics(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenSummary>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let summary = state.db.get_token_summary(project_id).await.map_err(|e| {
        tracing::error!("get_token_analytics failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/tokens/:id/volume — hourly volume for a token
pub async fn get_token_volume(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenVolumeStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_volume_24h(project_id, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_volume failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/tokens/:id/status — status distribution for a token
pub async fn get_token_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TokenStatusStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_status_distribution_24h(project_id, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_status failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/tokens/:id/latency — latency percentiles for a token
pub async fn get_token_latency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(token_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<TokenLatencyStat>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let stats = state
        .db
        .get_token_latency_percentiles_24h(project_id, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("get_token_latency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/health/upstreams — current status of all tracked upstreams
pub async fn get_upstream_health(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<crate::proxy::loadbalancer::UpstreamStatus>>, StatusCode> {
    auth.require_scope("system:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    Ok(Json(state.lb.get_all_status()))
}

pub async fn get_analytics_summary(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<crate::models::analytics::AnalyticsSummary>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(24)
        .clamp(1, 8760); // 1 hour minimum, 1 year maximum

    let summary = state
        .db
        .get_analytics_summary(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_analytics_summary failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/timeseries — timeseries data for charts
pub async fn get_analytics_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::AnalyticsTimeseriesPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(24)
        .clamp(1, 8760); // 1 hour minimum, 1 year maximum

    let points = state
        .db
        .get_analytics_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_analytics_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/experiments — A/B testing experiment data
pub async fn get_analytics_experiments(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<crate::models::analytics::ExperimentSummary>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let experiments = state
        .db
        .get_analytics_experiments(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_analytics_experiments failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(experiments))
}

/// GET /api/v1/analytics/spend/breakdown?group_by=model|token|tag:KEY&hours=720
///
/// Returns spend grouped by a chosen dimension over a time window.
/// - `group_by=model`   → spend per LLM model (gpt-4o, claude-3, etc.)
/// - `group_by=token`   → spend per virtual token (agent key)
/// - `group_by=tag:team` → spend per custom tag value (from X-Properties header)
///
/// Default: group_by=model, hours=720 (30 days)
pub async fn get_spend_breakdown(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<SpendBreakdownParams>,
) -> Result<Json<SpendBreakdownResponse>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = params.hours.unwrap_or(720); // default: 30 days
    if hours <= 0 || hours > 8760 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let group_by = params.group_by.as_deref().unwrap_or("model");

    let (dimension_label, rows) = if group_by == "model" {
        (
            "model",
            state.db.get_spend_by_model(project_id, hours).await,
        )
    } else if group_by == "token" {
        (
            "token",
            state.db.get_spend_by_token(project_id, hours).await,
        )
    } else if let Some(tag_key) = group_by.strip_prefix("tag:") {
        if tag_key.is_empty() || tag_key.len() > 64 {
            return Err(StatusCode::BAD_REQUEST);
        }
        (
            tag_key,
            state.db.get_spend_by_tag(project_id, hours, tag_key).await,
        )
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let rows = rows.map_err(|e| {
        tracing::error!("get_spend_breakdown failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total_cost_usd: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
    let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

    Ok(Json(SpendBreakdownResponse {
        group_by: group_by.to_string(),
        dimension_label: dimension_label.to_string(),
        hours,
        total_cost_usd,
        total_requests,
        breakdown: rows,
    }))
}
