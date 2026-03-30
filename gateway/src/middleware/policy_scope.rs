// ── Policy-Token Scope Validation ─────────────────────────────────────
//
// Validates that policies attached to a token don't route outside the
// token's allowed_providers and allowed_models scope.
//
// Called at:
// - Token creation (create_token)
// - Token policy update (set_token_policy_ids, update_token_config)
// - Bulk token creation
//
// This catches misconfiguration early, before requests fail at runtime.

use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

/// Represents a model found in a policy's routing action.
#[derive(Debug)]
pub struct PolicyModelRef {
    pub policy_id: Uuid,
    pub policy_name: String,
    pub model: String,
    pub action_type: String, // "dynamic_route" or "conditional_route"
}

/// Result of policy-token scope validation.
#[derive(Debug)]
pub struct PolicyScopeValidationResult {
    pub violations: Vec<PolicyScopeViolation>,
}

#[derive(Debug)]
pub struct PolicyScopeViolation {
    pub policy_id: Uuid,
    pub policy_name: String,
    pub model: String,
    pub detected_provider: String,
    pub violation_type: ViolationType,
}

#[derive(Debug, Clone)]
pub enum ViolationType {
    ProviderNotAllowed { allowed: Vec<String> },
    ModelNotAllowed { allowed_patterns: Vec<String> },
}

/// Detailed violation type for structured UI error responses.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum DetailedViolationType {
    ProviderNotAllowed { allowed: Vec<String> },
    ModelNotAllowed { allowed_patterns: Vec<String> },
}

/// Detailed scope violation for structured error responses (UI-friendly).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetailedScopeViolation {
    pub model: String,
    pub detected_provider: String,
    pub violation_type: DetailedViolationType,
}

/// Extract all models referenced in routing actions from a policy's rules.
#[allow(dead_code)]
pub fn extract_routing_models(rules: &[crate::models::policy::Rule]) -> Vec<(String, String)> {
    // (model, action_type) pairs
    let mut models = Vec::new();

    for rule in rules {
        for action in &rule.then {
            match action {
                crate::models::policy::Action::DynamicRoute { pool, fallback, .. } => {
                    for target in pool {
                        if !target.model.is_empty() {
                            models.push((target.model.clone(), "dynamic_route".to_string()));
                        }
                    }
                    if let Some(fb) = fallback {
                        if !fb.model.is_empty() {
                            models.push((fb.model.clone(), "dynamic_route".to_string()));
                        }
                    }
                }
                crate::models::policy::Action::ConditionalRoute {
                    branches, fallback, ..
                } => {
                    for branch in branches {
                        if !branch.target.model.is_empty() {
                            models.push((
                                branch.target.model.clone(),
                                "conditional_route".to_string(),
                            ));
                        }
                    }
                    if let Some(fb) = fallback {
                        if !fb.model.is_empty() {
                            models.push((fb.model.clone(), "conditional_route".to_string()));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    models
}

/// Extract routing models from raw JSON rules (for validation before parsing).
/// Returns (model, action_type) pairs.
pub fn extract_routing_models_from_json(rules: &Value) -> Vec<(String, String)> {
    let mut models = Vec::new();

    if let Some(arr) = rules.as_array() {
        for rule in arr {
            if let Some(actions) = rule.get("then").and_then(|a| a.as_array()) {
                for action in actions {
                    // Check dynamic_route action
                    if let Some(pool) = action.get("dynamic_route").and_then(|dr| dr.get("pool")).and_then(|p| p.as_array()) {
                        for entry in pool {
                            if let Some(model) = entry.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    models.push((model.to_string(), "dynamic_route".to_string()));
                                }
                            }
                        }
                        // Check fallback
                        if let Some(fb) = action.get("dynamic_route").and_then(|dr| dr.get("fallback")) {
                            if let Some(model) = fb.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    models.push((model.to_string(), "dynamic_route".to_string()));
                                }
                            }
                        }
                    }
                    // Check conditional_route action
                    if let Some(routes) = action.get("conditional_route").and_then(|cr| cr.get("routes")).and_then(|r| r.as_array()) {
                        for route in routes {
                            if let Some(target) = route.get("target") {
                                if let Some(model) = target.get("model").and_then(|m| m.as_str()) {
                                    if !model.is_empty() {
                                        models.push((model.to_string(), "conditional_route".to_string()));
                                    }
                                }
                            }
                        }
                        // Check fallback
                        if let Some(fb) = action.get("conditional_route").and_then(|cr| cr.get("fallback")) {
                            if let Some(model) = fb.get("model").and_then(|m| m.as_str()) {
                                if !model.is_empty() {
                                    models.push((model.to_string(), "conditional_route".to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    models
}

/// Validate that all models referenced in policies' routing actions
/// are within the token's allowed scope.
///
/// Returns Ok(()) if all models are allowed, or Err with detailed message.
#[allow(dead_code)]
pub fn validate_policies_against_token_scope(
    policies: &[crate::models::policy::Policy],
    allowed_providers: Option<&[String]>,
    allowed_models: Option<&Value>,
) -> Result<(), String> {
    let start = Instant::now();

    // If no restrictions, everything is allowed
    let has_provider_restriction = allowed_providers.map_or(false, |p| !p.is_empty());
    let has_model_restriction =
        allowed_models.map_or(false, |v| v.as_array().map_or(false, |arr| !arr.is_empty()));

    if !has_provider_restriction && !has_model_restriction {
        let duration = start.elapsed().as_secs_f64();
        crate::middleware::metrics::observe_scope_validation_duration(duration);
        return Ok(());
    }

    let mut violations: Vec<String> = Vec::new();

    for policy in policies {
        let models = extract_routing_models(&policy.rules);

        for (model, action_type) in models {
            // Detect provider from model name
            let detected_provider = detect_provider_from_model(&model);

            // Check provider restriction
            if has_provider_restriction {
                if let Some(allowed) = allowed_providers {
                    let provider_lower = detected_provider.to_lowercase();
                    let is_allowed = allowed.iter().any(|p| p.to_lowercase() == provider_lower);

                    if !is_allowed {
                        let provider_note = if detected_provider == "unknown" {
                            " (unknown model prefix - must be explicitly allowed or use known model naming)"
                        } else {
                            ""
                        };
                        violations.push(format!(
                            "Policy '{}' (id: {}, action: {}) routes to model '{}' (provider: {}) but token only allows providers: [{}]{}",
                            policy.name,
                            policy.id,
                            action_type,
                            model,
                            detected_provider,
                            allowed.join(", "),
                            provider_note
                        ));
                        // Record provider violation metric (Task 36)
                        crate::middleware::metrics::record_scope_validation_failure("provider_not_allowed");
                        continue; // Skip model check if provider already violates
                    }
                }
            }

            // Check model restriction
            if has_model_restriction {
                if let Some(models_value) = allowed_models {
                    if let Some(patterns) = models_value.as_array() {
                        let model_allowed = patterns.iter().any(|p| {
                            p.as_str()
                                .map_or(false, |pattern| crate::utils::glob_match(pattern, &model))
                        });

                        if !model_allowed {
                            let pattern_strs: Vec<String> = patterns
                                .iter()
                                .filter_map(|p| p.as_str().map(|s| s.to_string()))
                                .collect();

                            violations.push(format!(
                                "Policy '{}' (id: {}, action: {}) routes to model '{}' which is not in token's allowed_models: [{}]",
                                policy.name,
                                policy.id,
                                action_type,
                                model,
                                pattern_strs.join(", ")
                            ));
                            // Record model violation metric (Task 36)
                            crate::middleware::metrics::record_scope_validation_failure("model_not_allowed");
                        }
                    }
                }
            }
        }
    }

    // Record validation duration (Task 36)
    let duration = start.elapsed().as_secs_f64();
    crate::middleware::metrics::observe_scope_validation_duration(duration);

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.join("; "))
    }
}

/// Validate policy routing targets against token's allowed scope.
/// Returns detailed violations for UI display.
///
/// # Arguments
/// * `routing_models` - List of (model, action_type) pairs from policy routing actions
/// * `allowed_providers` - Optional list of allowed provider names
/// * `allowed_models` - Optional JSON array of allowed model patterns (globs)
///
/// # Returns
/// * `Ok(())` if all models are within scope
/// * `Err(Vec<DetailedScopeViolation>)` with structured violations for UI display
pub fn validate_policy_scope_detailed(
    routing_models: &[(String, String)],
    allowed_providers: Option<&[String]>,
    allowed_models: Option<&Value>,
) -> Result<(), Vec<DetailedScopeViolation>> {
    let mut violations = Vec::new();

    let has_provider_restriction = allowed_providers.map_or(false, |p| !p.is_empty());
    let has_model_restriction =
        allowed_models.map_or(false, |v| v.as_array().map_or(false, |arr| !arr.is_empty()));

    for (model, _action_type) in routing_models {
        let detected_provider = detect_provider_from_model(model);

        // Check provider restriction
        if has_provider_restriction {
            if let Some(allowed) = allowed_providers {
                let provider_lower = detected_provider.to_lowercase();
                let is_allowed = allowed.iter().any(|p| p.to_lowercase() == provider_lower);

                if !is_allowed {
                    violations.push(DetailedScopeViolation {
                        model: model.clone(),
                        detected_provider: detected_provider.clone(),
                        violation_type: DetailedViolationType::ProviderNotAllowed {
                            allowed: allowed.to_vec(),
                        },
                    });
                    continue; // Skip model check if provider already violates
                }
            }
        }

        // Check model restriction
        if has_model_restriction {
            if let Some(models_value) = allowed_models {
                if let Some(patterns) = models_value.as_array() {
                    let model_allowed = patterns.iter().any(|p| {
                        p.as_str()
                            .map_or(false, |pattern| crate::utils::glob_match(pattern, model))
                    });

                    if !model_allowed {
                        let pattern_strs: Vec<String> = patterns
                            .iter()
                            .filter_map(|p| p.as_str().map(|s| s.to_string()))
                            .collect();

                        violations.push(DetailedScopeViolation {
                            model: model.clone(),
                            detected_provider,
                            violation_type: DetailedViolationType::ModelNotAllowed {
                                allowed_patterns: pattern_strs,
                            },
                        });
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// Detect provider from model name (simplified version).
/// Returns the provider name in lowercase, or "unknown" for unrecognized patterns.
pub fn detect_provider_from_model(model: &str) -> String {
    let model_lower = model.to_lowercase();

    if model_lower.starts_with("gpt-")
        || model_lower.starts_with("o1-")
        || model_lower.starts_with("o3-")
    {
        return "openai".to_string();
    }
    if model_lower.starts_with("claude-") {
        return "anthropic".to_string();
    }
    if model_lower.starts_with("gemini-") {
        return "google".to_string();
    }
    if model_lower.starts_with("bedrock-")
        || model_lower.starts_with("anthropic.claude")
        || model_lower.starts_with("amazon.")
    {
        return "aws".to_string();
    }
    if model_lower.starts_with("azure-") || model_lower.contains("azureopenai") {
        return "azure".to_string();
    }
    if model_lower.starts_with("command-") {
        return "cohere".to_string();
    }
    if model_lower.starts_with("mistral-") || model_lower.starts_with("mixtral-") {
        return "mistral".to_string();
    }
    if model_lower.starts_with("llama-") || model_lower.starts_with("groq-") {
        return "groq".to_string();
    }
    if model_lower.starts_with("deepseek-") {
        return "deepseek".to_string();
    }
    if model_lower.starts_with("qwen-") {
        return "alibaba".to_string();
    }

    // Unknown model pattern - must be explicitly allowed if provider restrictions exist
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_restrictions_allows_all() {
        let result = validate_policies_against_token_scope(&[], None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_provider_openai() {
        assert_eq!(detect_provider_from_model("gpt-4o"), "openai");
        assert_eq!(detect_provider_from_model("GPT-4O-MINI"), "openai");
        assert_eq!(detect_provider_from_model("o1-preview"), "openai");
    }

    #[test]
    fn test_detect_provider_anthropic() {
        assert_eq!(detect_provider_from_model("claude-3-opus"), "anthropic");
        assert_eq!(detect_provider_from_model("Claude-sonnet-4"), "anthropic");
    }

    #[test]
    fn test_detect_provider_google() {
        assert_eq!(detect_provider_from_model("gemini-2.0-flash"), "google");
        assert_eq!(detect_provider_from_model("Gemini-pro"), "google");
    }

    #[test]
    fn test_detect_provider_unknown() {
        // Unknown model prefixes should return "unknown"
        assert_eq!(detect_provider_from_model("custom-model-v1"), "unknown");
        assert_eq!(detect_provider_from_model("my-llm"), "unknown");
        assert_eq!(detect_provider_from_model("unknown-model"), "unknown");
    }

    #[test]
    fn test_detect_provider_deepseek_and_qwen() {
        assert_eq!(detect_provider_from_model("deepseek-coder"), "deepseek");
        assert_eq!(detect_provider_from_model("qwen-72b"), "alibaba");
    }
}
