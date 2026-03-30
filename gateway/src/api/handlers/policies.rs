use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde_json::json;
use uuid::Uuid;

use super::dtos::{
    CreatePolicyRequest, DeleteResponse, PaginationParams, PolicyResponse, UpdatePolicyRequest,
};
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::errors::AppError;
use crate::store::postgres::PolicyRow;
use crate::AppState;

pub async fn list_policies(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<PolicyRow>>, AppError> {
    auth.require_scope("policies:read")
        .map_err(|_| AppError::Forbidden("policies:read scope required".to_string()))?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    let policies = state.db.list_policies(project_id, limit, offset).await?;

    Ok(Json(policies))
}

/// POST /api/v1/policies — create a new policy
pub async fn create_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreatePolicyRequest>,
) -> Result<(StatusCode, Json<PolicyResponse>), AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("policies:write")
        .map_err(|_| AppError::Forbidden("policies:write scope required".to_string()))?;
    let project_id = payload
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    // SEC: verify project isolation
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let mode = payload.mode.unwrap_or_else(|| "enforce".to_string());
    let phase = payload.phase.unwrap_or_else(|| "pre".to_string());

    // Validate mode
    if mode != "enforce" && mode != "shadow" {
        tracing::warn!("create_policy: invalid mode: {}", mode);
        return Err(AppError::ValidationError {
            message: format!("Invalid mode: {}. Must be 'enforce' or 'shadow'", mode),
        });
    }

    // Validate phase
    if phase != "pre" && phase != "post" {
        tracing::warn!("create_policy: invalid phase: {}", phase);
        return Err(AppError::ValidationError {
            message: format!("Invalid phase: {}. Must be 'pre' or 'post'", phase),
        });
    }

    // SEC: enforce max size on rules JSON to prevent oversized payloads clogging DB+memory
    const MAX_RULES_BYTES: usize = 64 * 1024; // 64KB
    let rules_str = payload.rules.to_string();
    if rules_str.len() > MAX_RULES_BYTES {
        tracing::warn!(
            "create_policy: rules JSON too large: {} bytes",
            rules_str.len()
        );
        return Err(AppError::ValidationError {
            message: format!("Rules JSON exceeds maximum size of {}KB", MAX_RULES_BYTES / 1024),
        });
    }

    // Validate model patterns in routing actions
    if let Err(e) = validate_routing_actions(&payload.rules) {
        tracing::warn!("create_policy: invalid routing action: {}", e);
        return Err(AppError::ValidationError { message: e });
    }

    // Validate phase-action compatibility
    match validate_phase_actions(&phase, &payload.rules) {
        Ok(warnings) => {
            // Log warnings but allow creation
            for warning in &warnings {
                tracing::warn!("create_policy: phase-action warning: {}", warning);
            }
        }
        Err(errors) => {
            tracing::warn!(
                "create_policy: phase-action validation failed: {:?}",
                errors
            );
            return Err(AppError::ValidationError {
                message: format!(
                    "Policy contains actions incompatible with selected phase: {}",
                    errors.join(", ")
                ),
            });
        }
    }

    // Fetch the token to validate scope
    let token = state
        .db
        .get_token(&payload.token_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Token {}", payload.token_id)))?;

    // Verify token belongs to the same project
    if token.project_id != project_id {
        return Err(AppError::ValidationError {
            message: "Token must belong to the same project as the policy".to_string(),
        });
    }

    // Extract routing models from rules and validate against token's scope
    let routing_models = crate::middleware::policy_scope::extract_routing_models_from_json(&payload.rules);

    if let Err(violations) = crate::middleware::policy_scope::validate_policy_scope_detailed(
        &routing_models,
        token.allowed_providers.as_deref(),
        token.allowed_models.as_ref(),
    ) {
        let violation_strs: Vec<String> = violations
            .iter()
            .map(|v| format!("{} (provider: {})", v.model, v.detected_provider))
            .collect();
        return Err(AppError::ValidationError {
            message: format!(
                "Policy routing targets exceed token's allowed scope: {}",
                violation_strs.join(", ")
            ),
        });
    }

    let id = state
        .db
        .insert_policy(
            project_id,
            &payload.name,
            &mode,
            &phase,
            payload.rules,
            payload.retry,
            &payload.token_id,
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(PolicyResponse {
            id,
            name: payload.name,
            message: "Policy created".to_string(),
        }),
    ))
}

/// PUT /api/v1/policies/:id — update a policy
pub async fn update_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
    Json(payload): Json<UpdatePolicyRequest>,
) -> Result<(StatusCode, Json<PolicyResponse>), AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("policies:write")
        .map_err(|_| AppError::Forbidden("policies:write scope required".to_string()))?;
    let id = Uuid::parse_str(&id_str).map_err(|_| AppError::ValidationError {
        message: "Invalid policy ID format".to_string(),
    })?;
    // HIGH-3: Accept explicit project_id from query params
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    // Validate mode if provided
    if let Some(ref mode) = payload.mode {
        if mode != "enforce" && mode != "shadow" {
            return Err(AppError::ValidationError {
                message: format!("Invalid mode: {}. Must be 'enforce' or 'shadow'", mode),
            });
        }
    }

    // Validate phase if provided
    if let Some(ref phase) = payload.phase {
        if phase != "pre" && phase != "post" {
            return Err(AppError::ValidationError {
                message: format!("Invalid phase: {}. Must be 'pre' or 'post'", phase),
            });
        }
    }

    // Validate model patterns in routing actions if rules are being updated
    if let Some(ref rules) = payload.rules {
        if let Err(e) = validate_routing_actions(rules) {
            tracing::warn!("update_policy: invalid routing action: {}", e);
            return Err(AppError::ValidationError { message: e });
        }
    }

    // Validate phase-action compatibility
    // We need to validate with the effective phase and rules combination
    // If only one is being updated, fetch the existing policy to get the other
    let needs_existing_policy = payload.rules.is_some() || payload.phase.is_some();
    let effective_phase: String;
    let effective_rules: serde_json::Value;

    if needs_existing_policy {
        // Fetch existing policy to determine effective values
        let existing = state.db.get_policy_by_id(id, project_id).await?;

        match existing {
            Some(existing_policy) => {
                // Determine effective phase: use payload if provided, else existing
                effective_phase = payload
                    .phase
                    .clone()
                    .unwrap_or_else(|| existing_policy.phase.clone());
                // Determine effective rules: use payload if provided, else existing
                effective_rules = payload
                    .rules
                    .clone()
                    .unwrap_or_else(|| existing_policy.rules.clone());

                // Validate scope against token if rules are being changed
                if payload.rules.is_some() && !existing_policy.token_id.is_empty() {
                    // Fetch the token to get allowed scope
                    let token = state.db.get_token(&existing_policy.token_id).await?;

                    if let Some(token) = token {
                        // Extract routing models from the new rules
                        let routing_models = crate::middleware::policy_scope::extract_routing_models_from_json(&effective_rules);

                        if !routing_models.is_empty() {
                            // Validate against token's allowed scope
                            if let Err(violations) = crate::middleware::policy_scope::validate_policy_scope_detailed(
                                &routing_models,
                                token.allowed_providers.as_deref(),
                                token.allowed_models.as_ref(),
                            ) {
                                tracing::warn!(
                                    "update_policy: scope validation failed for policy {}: {:?}",
                                    id,
                                    violations
                                );
                                let violation_strs: Vec<String> = violations
                                    .iter()
                                    .map(|v| format!("{} (provider: {})", v.model, v.detected_provider))
                                    .collect();
                                return Err(AppError::ValidationError {
                                    message: format!(
                                        "Policy routing targets exceed token's allowed scope: {}",
                                        violation_strs.join(", ")
                                    ),
                                });
                            }
                        }
                    }
                }

                match validate_phase_actions(&effective_phase, &effective_rules) {
                    Ok(warnings) => {
                        for warning in &warnings {
                            tracing::warn!("update_policy: phase-action warning: {}", warning);
                        }
                    }
                    Err(errors) => {
                        tracing::warn!(
                            "update_policy: phase-action validation failed: {:?}",
                            errors
                        );
                        return Err(AppError::ValidationError {
                            message: format!(
                                "Policy contains actions incompatible with selected phase: {}",
                                errors.join(", ")
                            ),
                        });
                    }
                }
            }
            None => {
                return Err(AppError::NotFound(format!("Policy {}", id)));
            }
        }
    }

    let updated = state
        .db
        .update_policy(
            id,
            project_id,
            payload.mode.as_deref(),
            payload.phase.as_deref(),
            payload.rules,
            payload.retry,
            payload.name.as_deref(),
            None, // No optimistic locking for this API endpoint
        )
        .await?;

    match updated {
        Ok(true) => {}
        Ok(false) => {
            return Err(AppError::NotFound(format!("Policy {}", id)));
        }
        Err(()) => {
            return Err(AppError::ValidationError {
                message: "Version conflict - policy was modified by another request".to_string(),
            });
        }
    }

    Ok((
        StatusCode::OK,
        Json(PolicyResponse {
            id,
            name: payload.name.unwrap_or_default(),
            message: "Policy updated".to_string(),
        }),
    ))
}

/// DELETE /api/v1/policies/:id — soft-delete a policy
pub async fn delete_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<DeleteResponse>, AppError> {
    auth.require_role("admin")
        .map_err(|_| AppError::Forbidden("admin role required".to_string()))?;
    auth.require_scope("policies:write")
        .map_err(|_| AppError::Forbidden("policies:write scope required".to_string()))?;
    let id = Uuid::parse_str(&id_str).map_err(|_| AppError::ValidationError {
        message: "Invalid policy ID format".to_string(),
    })?;
    // HIGH-3: Accept explicit project_id from query params
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let deleted = state.db.delete_policy(id, project_id).await?;

    if !deleted {
        tracing::warn!(
            policy_id = %id,
            project_id = %project_id,
            "HIGH-3: Policy deletion failed - not found or cross-project access attempt"
        );
    }

    Ok(Json(DeleteResponse { id, deleted }))
}

/// GET /api/v1/policies/:id/versions — list policy version history
pub async fn list_policy_versions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Result<Json<Vec<crate::store::postgres::PolicyVersionRow>>, AppError> {
    auth.require_scope("policies:read")
        .map_err(|_| AppError::Forbidden("policies:read scope required".to_string()))?;
    let id = Uuid::parse_str(&id_str).map_err(|_| AppError::ValidationError {
        message: "Invalid policy ID format".to_string(),
    })?;

    // SEC-03: Enforce project isolation
    let project_id = auth.default_project_id();
    verify_project_ownership(&state, auth.org_id, project_id).await?;

    let versions = state.db.list_policy_versions(id, project_id).await?;

    Ok(Json(versions))
}

/// Validate model patterns in routing actions within policy rules.
/// Checks dynamic_route and conditional_route actions for valid model patterns.
fn validate_routing_actions(rules: &serde_json::Value) -> Result<(), String> {
    if let Some(arr) = rules.as_array() {
        for rule in arr {
            if let Some(actions) = rule.get("actions").and_then(|a| a.as_array()) {
                for action in actions {
                    // Check dynamic_route action
                    if let Some(pool) = action.get("pool").and_then(|p| p.as_array()) {
                        for entry in pool {
                            // Validate model field if present
                            if let Some(model) = entry.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    crate::proxy::loadbalancer::validate_model_pattern(model)?;
                                }
                            }
                        }
                    }
                    // Check conditional_route action
                    if let Some(routes) = action.get("routes").and_then(|r| r.as_array()) {
                        for route in routes {
                            // Validate model field if present
                            if let Some(model) = route.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    crate::proxy::loadbalancer::validate_model_pattern(model)?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Pre-flight only actions - these can only be executed before the upstream request
const PREFLIGHT_ONLY_ACTIONS: &[&str] = &[
    "rate_limit",
    "override",
    "dynamic_route",
    "conditional_route",
    "split",
    "tool_scope",
    "require_approval",
];

/// Post-flight only actions - these can only be executed after the upstream response
const POSTFLIGHT_ONLY_ACTIONS: &[&str] = &["validate_schema"];

/// Validate that actions are compatible with the selected policy phase.
///
/// Returns `Ok(warnings)` on success (warnings may be empty) or `Err(errors)` if
/// there are phase-action mismatches that should block policy creation/update.
///
/// # Validation Rules
/// - Pre-flight only actions (rate_limit, override, dynamic_route, conditional_route,
///   split, tool_scope, require_approval) error if used in post-flight phase
/// - Post-flight only actions (validate_schema) error if used in pre-flight phase
/// - Redact with direction=response in pre-flight or direction=request in post-flight
///   generates a warning (not an error)
fn validate_phase_actions(
    phase: &str,
    rules: &serde_json::Value,
) -> Result<Vec<String>, Vec<String>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if let Some(arr) = rules.as_array() {
        for (rule_idx, rule) in arr.iter().enumerate() {
            let rule_num = rule_idx + 1;
            if let Some(actions) = rule.get("actions").and_then(|a| a.as_array()) {
                for (action_idx, action) in actions.iter().enumerate() {
                    let action_num = action_idx + 1;

                    // Check each action key
                    if let Some(obj) = action.as_object() {
                        for action_key in obj.keys() {
                            // Check pre-flight only actions in post-flight phase
                            if phase == "post"
                                && PREFLIGHT_ONLY_ACTIONS.contains(&action_key.as_str())
                            {
                                errors.push(format!(
                                    "Rule {}, Action {}: '{}' action is pre-flight only and cannot be used in post-flight phase",
                                    rule_num, action_num, action_key
                                ));
                            }

                            // Check post-flight only actions in pre-flight phase
                            if phase == "pre"
                                && POSTFLIGHT_ONLY_ACTIONS.contains(&action_key.as_str())
                            {
                                errors.push(format!(
                                    "Rule {}, Action {}: '{}' action is post-flight only and cannot be used in pre-flight phase",
                                    rule_num, action_num, action_key
                                ));
                            }

                            // Check redact action direction mismatches (warning only)
                            if action_key == "redact" {
                                if let Some(direction) = action
                                    .get("redact")
                                    .and_then(|r| r.get("direction"))
                                    .and_then(|d| d.as_str())
                                {
                                    if phase == "pre" && direction == "response" {
                                        warnings.push(format!(
                                            "Rule {}, Action {}: 'redact' with direction=response has no effect in pre-flight phase (response is not yet available)",
                                            rule_num, action_num
                                        ));
                                    } else if phase == "post" && direction == "request" {
                                        warnings.push(format!(
                                            "Rule {}, Action {}: 'redact' with direction=request has no effect in post-flight phase (request has already been sent)",
                                            rule_num, action_num
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(warnings)
    } else {
        Err(errors)
    }
}