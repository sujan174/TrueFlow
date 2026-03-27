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

use uuid::Uuid;
use serde_json::Value;

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

/// Extract all models referenced in routing actions from a policy's rules.
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
                crate::models::policy::Action::ConditionalRoute { branches, fallback, .. } => {
                    for branch in branches {
                        if !branch.target.model.is_empty() {
                            models.push((branch.target.model.clone(), "conditional_route".to_string()));
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

/// Validate that all models referenced in policies' routing actions
/// are within the token's allowed scope.
///
/// Returns Ok(()) if all models are allowed, or Err with detailed message.
pub fn validate_policies_against_token_scope(
    policies: &[crate::models::policy::Policy],
    allowed_providers: Option<&[String]>,
    allowed_models: Option<&Value>,
) -> Result<(), String> {
    // If no restrictions, everything is allowed
    let has_provider_restriction = allowed_providers.map_or(false, |p| !p.is_empty());
    let has_model_restriction = allowed_models.map_or(false, |v| {
        v.as_array().map_or(false, |arr| !arr.is_empty())
    });

    if !has_provider_restriction && !has_model_restriction {
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
                        continue; // Skip model check if provider already violates
                    }
                }
            }

            // Check model restriction
            if has_model_restriction {
                if let Some(models_value) = allowed_models {
                    if let Some(patterns) = models_value.as_array() {
                        let model_allowed = patterns.iter().any(|p| {
                            p.as_str().map_or(false, |pattern| {
                                crate::utils::glob_match(pattern, &model)
                            })
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
                        }
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.join("; "))
    }
}

/// Detect provider from model name (simplified version).
/// Returns the provider name in lowercase, or "unknown" for unrecognized patterns.
fn detect_provider_from_model(model: &str) -> String {
    let model_lower = model.to_lowercase();

    if model_lower.starts_with("gpt-") || model_lower.starts_with("o1-") || model_lower.starts_with("o3-") {
        return "openai".to_string();
    }
    if model_lower.starts_with("claude-") {
        return "anthropic".to_string();
    }
    if model_lower.starts_with("gemini-") {
        return "google".to_string();
    }
    if model_lower.starts_with("bedrock-") || model_lower.starts_with("anthropic.claude") || model_lower.starts_with("amazon.") {
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