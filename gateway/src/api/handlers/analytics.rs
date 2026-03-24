use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};

use super::dtos::{PaginationParams, SpendBreakdownParams, SpendBreakdownResponse};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::models::analytics::{
    CacheHitRatePoint, CacheLatencyComparison, CacheSummaryStats, CachedQueryRow,
    CostLatencyScatterPoint, DataResidencyStats, ErrorLogRow, ErrorTimeseriesPoint,
    ErrorTypeBreakdown, GuardrailTriggerStat, HitlLatencyStats,
    HitlSummaryStats, HitlVolumePoint, ModelCacheEfficiency, ModelErrorRate, ModelLatencyStat,
    ModelStatsRow, ModelUsageTimeseriesPoint, PiiBreakdownStat, PolicyActionStat,
    RejectionReason, SecuritySummaryStats, ShadowPolicyStat,
};
use crate::store::postgres::{
    EngagementTiersResponse, RateLimitedToken, RequestsPerUserPoint, TokenAlertsResponse,
    TokenLatencyStat, TokenStatusStat, TokenSummary, TokenVolumeStat, UserGrowthPoint,
};
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

/// GET /api/v1/analytics/spend/breakdown?group_by=model|token|tag:KEY|external_user_id|team&hours=720
///
/// Returns spend grouped by a chosen dimension over a time window.
/// - `group_by=model`   → spend per LLM model (gpt-4o, claude-3, etc.)
/// - `group_by=token`   → spend per virtual token (agent key)
/// - `group_by=tag:team` → spend per custom tag value (from X-Properties header)
/// - `group_by=external_user_id` → spend per external user/customer (SaaS builders)
/// - `group_by=team`    → spend per team
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
    } else if group_by == "external_user_id" {
        (
            "external_user_id",
            state.db.get_spend_by_external_user(project_id, hours).await,
        )
    } else if group_by == "team" {
        (
            "team",
            state.db.get_spend_by_team(project_id, hours).await,
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

/// GET /api/v1/analytics/users?hours=720
///
/// Returns spend aggregated by external_user_id (customer-level analytics).
/// Designed for SaaS builders who need to track spend per customer.
pub async fn get_user_spend(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<SpendBreakdownParams>,
) -> Result<Json<super::dtos::UserSpendResponse>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = params.hours.unwrap_or(720);
    if hours <= 0 || hours > 8760 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let users = state
        .db
        .get_user_spend_summary(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_user_spend failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total_users = users.len() as i64;
    let total_cost_usd: f64 = users.iter().map(|u| u.total_cost_usd).sum();

    Ok(Json(super::dtos::UserSpendResponse {
        hours,
        total_users,
        total_cost_usd,
        users,
    }))
}

// ── Traffic Analytics Endpoints (Traffic Tab) ──────────────────────────────────

/// GET /api/v1/analytics/traffic/timeseries?range=168
///
/// Returns traffic timeseries with status breakdown by policy_result.
/// Bucket size is hourly for <=24h, daily otherwise.
pub async fn get_traffic_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::TrafficTimeseriesPoint>>, StatusCode> {
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
        .get_traffic_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_traffic_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/latency/timeseries?range=168
///
/// Returns latency timeseries with p50/p90/p99 percentile breakdown.
/// Bucket size is hourly for <=24h, daily otherwise.
pub async fn get_latency_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::LatencyTimeseriesPoint>>, StatusCode> {
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
        .get_latency_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_latency_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

// ── Cost Analytics Endpoints (Cost Tab) ──────────────────────────────────

/// GET /api/v1/analytics/budget-health
///
/// Returns budget health status for the alert strip.
/// Shows counts of tokens above 80% cap and without caps.
pub async fn get_budget_health(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::models::analytics::BudgetHealthStatus>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let status = state
        .db
        .get_budget_health_status(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_budget_health failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(status))
}

/// GET /api/v1/analytics/spend/timeseries?group_by=provider|model|token&range=168
///
/// Returns spend timeseries grouped by a dimension.
/// Used for the Spend Over Time chart on the Cost tab.
pub async fn get_spend_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::SpendTimeseriesPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168) // default: 7 days
        .clamp(1, 8760);

    let group_by = range
        .get("group_by")
        .map(|s| s.as_str())
        .unwrap_or("provider");

    // Validate group_by
    if !matches!(group_by, "provider" | "model" | "token") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let points = state
        .db
        .get_spend_timeseries(project_id, hours, group_by)
        .await
        .map_err(|e| {
            tracing::error!("get_spend_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/cost-efficiency?range=168
///
/// Returns cost efficiency (cost per 1K tokens) by model over time.
/// Used for the Cost Efficiency Trend chart on the Cost tab.
pub async fn get_cost_efficiency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::CostEfficiencyPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168) // default: 7 days
        .clamp(1, 8760);

    let points = state
        .db
        .get_cost_efficiency_trend(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_cost_efficiency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/burn-rate
///
/// Returns budget burn rate calculation for the current month.
/// Used for the Budget Burn Rate card on the Cost tab.
pub async fn get_budget_burn_rate(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::models::analytics::BudgetBurnRate>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let rate = state
        .db
        .get_budget_burn_rate(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_budget_burn_rate failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(rate))
}

/// GET /api/v1/analytics/token-spend?range=168
///
/// Returns token spend breakdown with cap usage percentages.
/// Used for the Cost Breakdown Table on the Cost tab.
pub async fn get_token_spend(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::models::analytics::TokenSpendWithCap>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168) // default: 7 days
        .clamp(1, 8760);

    let tokens = state
        .db
        .get_token_spend_with_caps(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_token_spend failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(tokens))
}

// ── Users & Tokens Analytics Endpoints (Users & Tokens Tab) ──────────────────────────────────

/// GET /api/v1/analytics/users/growth?range=720
///
/// Returns user growth timeseries (distinct external_user_id per day).
/// Used by the User Growth Chart on the Users & Tokens tab.
pub async fn get_user_growth(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<UserGrowthPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(720) // default: 30 days
        .clamp(1, 8760);

    let points = state
        .db
        .get_user_growth_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_user_growth failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/users/engagement?range=720
///
/// Returns engagement tiers (Power/Regular/Light users by request volume).
/// Used by the Engagement Tiers card on the Users & Tokens tab.
pub async fn get_user_engagement(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<EngagementTiersResponse>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(720) // default: 30 days
        .clamp(1, 8760);

    let tiers = state
        .db
        .get_engagement_tiers(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_user_engagement failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(tiers))
}

/// GET /api/v1/analytics/tokens/alerts
///
/// Returns active token count and tokens hitting rate limits.
/// Used by the Active Tokens Card and Rate Limit Alert on the Users & Tokens tab.
pub async fn get_token_alerts(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<TokenAlertsResponse>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(24) // default: 24 hours
        .clamp(1, 8760);

    let alerts = state.db.get_token_alerts(project_id, hours).await.map_err(|e| {
        tracing::error!("get_token_alerts failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(alerts))
}

/// GET /api/v1/analytics/users/requests?range=168
///
/// Returns requests per user timeseries.
/// Used by the Requests Per User chart on the Users & Tokens tab.
pub async fn get_requests_per_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<RequestsPerUserPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168) // default: 7 days
        .clamp(1, 8760);

    let points = state
        .db
        .get_requests_per_user_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_requests_per_user failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

// ── Cache Analytics Endpoints (Cache Tab) ──────────────────────────────────

/// GET /api/v1/analytics/cache/summary
///
/// Returns cache summary statistics for the Cache tab ribbon.
pub async fn get_cache_summary(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<CacheSummaryStats>, StatusCode> {
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
        .clamp(1, 8760);

    // Get cache stats from audit logs
    let summary = state
        .db
        .get_cache_summary_stats(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_cache_summary failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/cache/hit-rate-timeseries?range=168
///
/// Returns cache hit rate timeseries for the Cache tab chart.
pub async fn get_cache_hit_rate_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<CacheHitRatePoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168) // default: 7 days
        .clamp(1, 8760);

    let points = state
        .db
        .get_cache_hit_rate_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_cache_hit_rate_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/cache/top-queries?limit=25
///
/// Returns top cached queries for the Cache tab table.
pub async fn get_top_cached_queries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CachedQueryRow>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let limit = params.limit.unwrap_or(25).clamp(1, 100);

    let queries = state.db.get_top_cached_queries(project_id, limit).await.map_err(|e| {
        tracing::error!("get_top_cached_queries failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(queries))
}

/// GET /api/v1/analytics/cache/model-efficiency?range=168
///
/// Returns model-level cache efficiency for the Cache tab.
pub async fn get_model_cache_efficiency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ModelCacheEfficiency>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let efficiency = state
        .db
        .get_model_cache_efficiency(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_model_cache_efficiency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(efficiency))
}

/// GET /api/v1/analytics/cache/latency-comparison?range=168
///
/// Returns cache latency comparison for the Cache tab.
pub async fn get_cache_latency_comparison(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<CacheLatencyComparison>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let comparison = state
        .db
        .get_cache_latency_comparison(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_cache_latency_comparison failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(comparison))
}

// ── Model Analytics Endpoints (Models Tab) ──────────────────────────────────

/// GET /api/v1/analytics/models/usage-timeseries?group_by=requests|cost|cache_hits&range=168
///
/// Returns model usage over time grouped by the specified metric.
/// Used by the Model Usage Over Time chart on the Models tab.
pub async fn get_model_usage_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ModelUsageTimeseriesPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168) // default: 7 days
        .clamp(1, 8760);

    let group_by = range
        .get("group_by")
        .map(|s| s.as_str())
        .unwrap_or("requests");

    // Validate group_by
    if !matches!(group_by, "requests" | "cost" | "cache_hits") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let points = state
        .db
        .get_model_usage_timeseries(project_id, hours, group_by)
        .await
        .map_err(|e| {
            tracing::error!("get_model_usage_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/models/error-rates?range=168
///
/// Returns error rate per model for the Models tab.
pub async fn get_model_error_rates(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ModelErrorRate>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let rates = state
        .db
        .get_model_error_rates(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_model_error_rates failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(rates))
}

/// GET /api/v1/analytics/models/latency?range=168
///
/// Returns latency stats per model for the Models tab.
pub async fn get_model_latency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ModelLatencyStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let stats = state
        .db
        .get_model_latency(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_model_latency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/models/stats?range=168
///
/// Returns combined model stats for the Models tab table.
pub async fn get_model_stats(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ModelStatsRow>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let stats = state
        .db
        .get_model_stats(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_model_stats failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/models/cost-latency-scatter?range=168
///
/// Returns cost vs latency data for the Models tab bubble chart.
pub async fn get_cost_latency_scatter(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<CostLatencyScatterPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let points = state
        .db
        .get_cost_latency_scatter(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_cost_latency_scatter failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

// ── Security Analytics Endpoints (Security Tab) ──────────────────────────────────

/// GET /api/v1/analytics/security/summary?range=168
///
/// Returns security KPI summary for the Security tab ribbon.
pub async fn get_security_summary(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<SecuritySummaryStats>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let summary = state
        .db
        .get_security_summary(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_security_summary failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/security/guardrail-triggers?range=168
///
/// Returns guardrail triggers grouped by category for the Security tab.
pub async fn get_guardrail_triggers(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<GuardrailTriggerStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let triggers = state
        .db
        .get_guardrail_triggers(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_guardrail_triggers failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(triggers))
}

/// GET /api/v1/analytics/security/pii-breakdown?range=168
///
/// Returns PII breakdown by pattern type for the Security tab.
pub async fn get_pii_breakdown(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<PiiBreakdownStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let breakdown = state
        .db
        .get_pii_breakdown(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_pii_breakdown failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(breakdown))
}

/// GET /api/v1/analytics/security/policy-actions?range=168
///
/// Returns policy action counts for the Security tab.
pub async fn get_policy_actions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<PolicyActionStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let actions = state
        .db
        .get_policy_actions(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_policy_actions failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(actions))
}

/// GET /api/v1/analytics/security/shadow-policies?range=168
///
/// Returns shadow mode policies with violation stats for the Security tab.
pub async fn get_shadow_policies(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ShadowPolicyStat>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let policies = state
        .db
        .get_shadow_policies(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_shadow_policies failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(policies))
}

/// GET /api/v1/analytics/security/data-residency?range=168
///
/// Returns data residency stats for the Security tab.
pub async fn get_data_residency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<DataResidencyStats>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let stats = state
        .db
        .get_data_residency(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_data_residency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

// ── HITL Analytics Endpoints (HITL Tab) ──────────────────────────────────

/// GET /api/v1/analytics/hitl/summary?range=168
///
/// Returns HITL KPI summary for the HITL tab ribbon.
pub async fn get_hitl_summary(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<HitlSummaryStats>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let summary = state
        .db
        .get_hitl_summary(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_hitl_summary failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/hitl/volume?range=168
///
/// Returns HITL volume timeseries for the HITL tab chart.
pub async fn get_hitl_volume(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<HitlVolumePoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let points = state
        .db
        .get_hitl_volume_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_hitl_volume failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/hitl/latency?range=168
///
/// Returns HITL latency stats for the SLA card.
pub async fn get_hitl_latency(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<HitlLatencyStats>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let stats = state
        .db
        .get_hitl_latency_stats(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_hitl_latency failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// GET /api/v1/analytics/hitl/reasons?range=168
///
/// Returns rejection reasons breakdown for the HITL tab.
/// Note: Returns mock data until rejection_reason column is added.
pub async fn get_hitl_rejection_reasons(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<RejectionReason>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let reasons = state
        .db
        .get_hitl_rejection_reasons(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_hitl_rejection_reasons failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(reasons))
}

// ── Error Analytics Endpoints (Errors Tab) ──────────────────────────────────

/// GET /api/v1/analytics/errors/summary?range=168
///
/// Returns error KPI summary for the Errors tab ribbon.
pub async fn get_error_summary(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<crate::models::analytics::ErrorSummaryStats>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let summary = state
        .db
        .get_error_summary(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_error_summary failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(summary))
}

/// GET /api/v1/analytics/errors/timeseries?range=168
///
/// Returns error timeseries for the Errors tab chart.
pub async fn get_error_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ErrorTimeseriesPoint>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let points = state
        .db
        .get_error_timeseries(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_error_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(points))
}

/// GET /api/v1/analytics/errors/breakdown?range=168
///
/// Returns error type breakdown for the Errors tab bar chart.
pub async fn get_error_breakdown(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
    Query(range): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ErrorTypeBreakdown>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let hours = range
        .get("range")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(168)
        .clamp(1, 8760);

    let breakdown = state
        .db
        .get_error_type_breakdown(project_id, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_error_breakdown failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(breakdown))
}

/// GET /api/v1/analytics/errors/logs?limit=50
///
/// Returns recent error logs for the Errors tab table.
pub async fn get_error_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<ErrorLogRow>>, StatusCode> {
    auth.require_scope("analytics:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    let logs = state
        .db
        .get_error_logs(project_id, limit)
        .await
        .map_err(|e| {
            tracing::error!("get_error_logs failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(logs))
}
