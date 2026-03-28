//! Experiment Management API handlers.
//!
//! A/B experiments are a convenience layer over the policy engine's `Action::Split`.
//! Creating an experiment creates a policy with a `Split` action; stopping one
//! soft-deletes that policy. Results are aggregated from audit log data.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AuthContext;
use crate::AppState;

/// Prefix for auto-generated experiment policies.
const EXPERIMENT_PREFIX: &str = "__experiment__";

// ── Request Types ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateExperimentRequest {
    /// Human-readable experiment name (e.g. "gpt4o-vs-claude").
    pub name: String,
    /// Token ID this experiment is bound to (string like "tf_v1_xxx").
    pub token_id: String,
    /// Variants with weights and model overrides.
    pub variants: Vec<ExperimentVariant>,
    /// Optional condition scope. Default: all requests (catch-all).
    #[serde(default)]
    pub condition: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ExperimentVariant {
    /// Variant label (e.g. "control", "treatment").
    pub name: String,
    /// Relative weight (does not need to sum to 100).
    pub weight: u32,
    /// Model to route to for this variant.
    #[serde(default)]
    pub model: Option<String>,
    /// Arbitrary body field overrides.
    #[serde(default)]
    pub set_body_fields: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExperimentRequest {
    /// Updated variants with new weights.
    pub variants: Vec<ExperimentVariant>,
}

// ── Handlers ─────────────────────────────────────────────────

/// POST /experiments — create an A/B experiment.
///
/// Internally creates a policy with `Action::Split`.
pub async fn create_experiment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateExperimentRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    auth.require_scope("experiments:write")?;
    let project_id = auth.default_project_id();

    // Validate experiment name
    if payload.name.is_empty() || payload.name.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Prevent confusion with the reserved prefix
    if payload.name.contains(EXPERIMENT_PREFIX) {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Validate variants
    if payload.variants.len() < 2 {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Validate variant weights are non-zero
    for v in &payload.variants {
        if v.weight == 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Build Split variants
    let split_variants: Vec<serde_json::Value> = payload
        .variants
        .iter()
        .map(|v| {
            let mut fields = v.set_body_fields.clone().unwrap_or_default();
            if let Some(ref model) = v.model {
                fields.insert("model".to_string(), serde_json::json!(model));
            }
            serde_json::json!({
                "name": v.name,
                "weight": v.weight,
                "set_body_fields": fields,
            })
        })
        .collect();

    // Build the condition (default: always match)
    let condition = payload
        .condition
        .unwrap_or_else(|| serde_json::json!({"always": true}));

    // Build the policy rules JSON
    let rules = serde_json::json!([{
        "when": condition,
        "then": {
            "action": "split",
            "experiment": payload.name,
            "variants": split_variants,
        }
    }]);

    let policy_name = format!("{}{}", EXPERIMENT_PREFIX, payload.name);

    let policy_id = state
        .db
        .insert_policy(project_id, &policy_name, "enforce", "pre", rules, None, &payload.token_id)
        .await
        .map_err(|e| {
            tracing::error!("create_experiment failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": policy_id,
            "name": payload.name,
            "status": "running",
            "variants": split_variants,
        })),
    ))
}

/// GET /experiments — list all running experiments.
///
/// Filters policies by the `__experiment__` prefix.
pub async fn list_experiments(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    auth.require_scope("experiments:read")?;
    let project_id = auth.default_project_id();

    let policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!("list_experiments failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let experiments: Vec<serde_json::Value> = policies
        .into_iter()
        .filter(|p| p.is_active && p.name.starts_with(EXPERIMENT_PREFIX))
        .map(|p| {
            let experiment_name = p.name.strip_prefix(EXPERIMENT_PREFIX).unwrap_or(&p.name);
            serde_json::json!({
                "id": p.id,
                "name": experiment_name,
                "status": "running",
                "created_at": p.created_at,
                "rules": p.rules,
            })
        })
        .collect();

    Ok(Json(experiments))
}

/// GET /experiments/:id — get a single experiment with its analytics.
pub async fn get_experiment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("experiments:read")?;
    let project_id = auth.default_project_id();

    // Find the policy
    let policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!("get_experiment failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let policy = policies
        .into_iter()
        .find(|p| p.id == id && p.name.starts_with(EXPERIMENT_PREFIX))
        .ok_or(StatusCode::NOT_FOUND)?;

    let experiment_name = policy
        .name
        .strip_prefix(EXPERIMENT_PREFIX)
        .unwrap_or(&policy.name);

    // Fetch analytics
    let analytics = state
        .db
        .get_analytics_experiments(project_id)
        .await
        .unwrap_or_default();

    let variant_results: Vec<&crate::models::analytics::ExperimentSummary> = analytics
        .iter()
        .filter(|a| a.experiment_name == experiment_name)
        .collect();

    let status = if policy.is_active {
        "running"
    } else {
        "stopped"
    };

    Ok(Json(serde_json::json!({
        "id": policy.id,
        "name": experiment_name,
        "status": status,
        "created_at": policy.created_at,
        "rules": policy.rules,
        "results": variant_results.iter().map(|r| serde_json::json!({
            "variant": r.variant_name,
            "total_requests": r.total_requests,
            "avg_latency_ms": r.avg_latency_ms,
            "total_cost_usd": r.total_cost_usd,
            "avg_tokens": r.avg_tokens,
            "error_count": r.error_count,
        })).collect::<Vec<_>>(),
    })))
}

/// GET /experiments/:id/results — per-variant breakdown.
pub async fn get_experiment_results(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("experiments:read")?;
    let project_id = auth.default_project_id();

    // Find the policy to get the experiment name
    let policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!("get_experiment_results failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let policy = policies
        .into_iter()
        .find(|p| p.id == id && p.name.starts_with(EXPERIMENT_PREFIX))
        .ok_or(StatusCode::NOT_FOUND)?;

    let experiment_name = policy
        .name
        .strip_prefix(EXPERIMENT_PREFIX)
        .unwrap_or(&policy.name);

    let analytics = state
        .db
        .get_analytics_experiments(project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_experiment_results analytics failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let variants: Vec<serde_json::Value> = analytics
        .iter()
        .filter(|a| a.experiment_name == experiment_name)
        .map(|r| {
            let error_rate = if r.total_requests > 0 {
                r.error_count as f64 / r.total_requests as f64
            } else {
                0.0
            };
            serde_json::json!({
                "variant": r.variant_name,
                "total_requests": r.total_requests,
                "avg_latency_ms": r.avg_latency_ms,
                "total_cost_usd": r.total_cost_usd,
                "avg_tokens": r.avg_tokens,
                "error_count": r.error_count,
                "error_rate": error_rate,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "experiment": experiment_name,
        "status": if policy.is_active { "running" } else { "stopped" },
        "variants": variants,
    })))
}

/// POST /experiments/:id/stop — stop a running experiment.
///
/// Soft-deletes the underlying Split policy.
pub async fn stop_experiment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("experiments:write")?;
    let project_id = auth.default_project_id();

    let deleted = state.db.delete_policy(id, project_id).await.map_err(|e| {
        tracing::error!("stop_experiment failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({
        "id": id,
        "status": "stopped",
    })))
}

/// PUT /experiments/:id — update variant weights.
pub async fn update_experiment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateExperimentRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("experiments:write")?;
    let project_id = auth.default_project_id();

    if payload.variants.len() < 2 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Find the existing policy to get the experiment name
    let policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!("update_experiment failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let policy = policies
        .into_iter()
        .find(|p| p.id == id && p.name.starts_with(EXPERIMENT_PREFIX) && p.is_active)
        .ok_or(StatusCode::NOT_FOUND)?;

    let experiment_name = policy
        .name
        .strip_prefix(EXPERIMENT_PREFIX)
        .unwrap_or(&policy.name);

    // Extract the original condition from the existing policy rules
    // This preserves targeting conditions like model filters or user segments
    let original_condition = policy
        .rules
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|rule| rule.get("when"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({"always": true}));

    // Build updated Split variants
    let split_variants: Vec<serde_json::Value> = payload
        .variants
        .iter()
        .map(|v| {
            let mut fields = v.set_body_fields.clone().unwrap_or_default();
            if let Some(ref model) = v.model {
                fields.insert("model".to_string(), serde_json::json!(model));
            }
            serde_json::json!({
                "name": v.name,
                "weight": v.weight,
                "set_body_fields": fields,
            })
        })
        .collect();

    // Preserve the original condition when updating
    let rules = serde_json::json!([{
        "when": original_condition,
        "then": {
            "action": "split",
            "experiment": experiment_name,
            "variants": split_variants,
        }
    }]);

    let updated = state
        .db
        .update_policy(id, project_id, None, None, Some(rules), None, None, None)
        .await
        .map_err(|e| {
            tracing::error!("update_experiment failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match updated {
        Ok(true) => {}
        Ok(false) => return Err(StatusCode::NOT_FOUND),
        Err(()) => return Err(StatusCode::CONFLICT),
    }

    Ok(Json(serde_json::json!({
        "id": id,
        "name": experiment_name,
        "status": "running",
        "variants": split_variants,
    })))
}

// ── Timeseries Handler ─────────────────────────────────────────────

/// Query parameters for timeseries endpoint
#[derive(Debug, Deserialize)]
pub struct TimeseriesQuery {
    /// Number of hours to look back (default: 24, max: 168)
    #[serde(default = "default_hours")]
    pub range: i32,
}

fn default_hours() -> i32 {
    24
}

/// Timeseries response point
#[derive(Debug, Serialize)]
pub struct TimeseriesPoint {
    pub bucket: chrono::DateTime<chrono::Utc>,
    pub variant_name: String,
    pub request_count: i64,
    pub avg_latency_ms: f64,
    pub total_cost_usd: f64,
}

/// GET /experiments/:id/timeseries — timeseries data for charts.
pub async fn get_experiment_timeseries(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Query(query): Query<TimeseriesQuery>,
) -> Result<Json<Vec<TimeseriesPoint>>, StatusCode> {
    auth.require_scope("experiments:read")?;
    let project_id = auth.default_project_id();

    // Clamp hours to reasonable range
    let hours = query.range.clamp(1, 168);

    // Find the policy to get the experiment name
    let policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!("get_experiment_timeseries failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let policy = policies
        .into_iter()
        .find(|p| p.id == id && p.name.starts_with(EXPERIMENT_PREFIX))
        .ok_or(StatusCode::NOT_FOUND)?;

    let experiment_name = policy
        .name
        .strip_prefix(EXPERIMENT_PREFIX)
        .unwrap_or(&policy.name);

    // Fetch timeseries data
    let timeseries = state
        .db
        .get_experiment_timeseries(project_id, experiment_name, hours)
        .await
        .map_err(|e| {
            tracing::error!("get_experiment_timeseries query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Convert to response format
    let points: Vec<TimeseriesPoint> = timeseries
        .into_iter()
        .map(|p| TimeseriesPoint {
            bucket: p.bucket,
            variant_name: p.variant_name,
            request_count: p.request_count,
            avg_latency_ms: p.avg_latency_ms,
            total_cost_usd: p.total_cost_usd,
        })
        .collect();

    Ok(Json(points))
}
