//! Model Access Control — per-token model-level RBAC.
//!
//! Restricts which models a virtual key can call:
//! - `allowed_models`: direct list of model name patterns (globs)
//! - `allowed_model_group_ids`: references to named groups (resolved at request time)
//!
//! Pattern matching:
//! - Exact: `"gpt-4o"` matches only `gpt-4o`
//! - Glob: `"gpt-4*"` matches `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, etc.
//! - Glob: `"claude-*"` matches `claude-3-opus`, `claude-3-haiku`, etc.
//! - Wildcard: `"*"` matches any model (equivalent to no restriction)
//!
//! If `allowed_models` is NULL/empty AND `allowed_model_group_ids` is NULL/empty,
//! all models are allowed (backwards compatible with existing tokens).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A named model access group (stored in `model_access_groups` table).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelAccessGroup {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub models: serde_json::Value, // JSON array of model patterns
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Check whether a requested model is allowed by the token's access rules.
///
/// Returns `Ok(())` if the model is allowed, or `Err(reason)` if denied.
pub fn check_model_access(
    requested_model: &str,
    allowed_models: Option<&serde_json::Value>,
    resolved_group_models: &[String],
) -> Result<(), String> {
    // If no model specified in request, skip check (non-model endpoints)
    if requested_model.is_empty() {
        return Ok(());
    }

    // Collect all allowed patterns from both sources
    let mut patterns: Vec<String> = Vec::new();

    // 1. Direct allowed_models from token
    if let Some(models_json) = allowed_models {
        if let Some(arr) = models_json.as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    patterns.push(s.to_string());
                }
            }
        }
    }

    // 2. Models from resolved groups
    patterns.extend_from_slice(resolved_group_models);

    // If no patterns configured at all, allow everything (backwards compatible)
    if patterns.is_empty() {
        return Ok(());
    }

    // Check if any pattern matches
    for pattern in &patterns {
        if model_matches(requested_model, pattern) {
            return Ok(());
        }
    }

    Err(format!(
        "Model '{}' is not allowed by this API key. Allowed: [{}]",
        requested_model,
        patterns.join(", ")
    ))
}

/// Check if a model name matches a pattern.
///
/// Supports:
/// - Exact match (case-insensitive): `"gpt-4o"` matches `"gpt-4o"`, `"GPT-4o"`
/// - Prefix glob: `"gpt-4*"` matches `"gpt-4o"`, `"gpt-4o-mini"`
/// - Suffix glob: `"*-preview"` matches `"gpt-4o-preview"`
/// - Full wildcard: `"*"` matches anything
/// - Contains glob: `"*turbo*"` matches `"gpt-4-turbo-preview"`
pub fn model_matches(model: &str, pattern: &str) -> bool {
    let model_lower = model.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    // Full wildcard
    if pattern_lower == "*" {
        return true;
    }

    // No glob characters — exact match
    if !pattern_lower.contains('*') {
        return model_lower == pattern_lower;
    }

    // Split by '*' and match segments in order
    let parts: Vec<&str> = pattern_lower.split('*').collect();

    // Single trailing '*' (prefix match): "gpt-4*"
    if parts.len() == 2 && parts[1].is_empty() {
        return model_lower.starts_with(parts[0]);
    }

    // Single leading '*' (suffix match): "*-preview"
    if parts.len() == 2 && parts[0].is_empty() {
        return model_lower.ends_with(parts[1]);
    }

    // General glob matching: "*turbo*", "gpt-*-mini", etc.
    let mut remaining = model_lower.as_str();

    // First part must be a prefix (unless it's empty = leading *)
    if !parts[0].is_empty() {
        if !remaining.starts_with(parts[0]) {
            return false;
        }
        remaining = &remaining[parts[0].len()..];
    }

    // Middle parts
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        match remaining.find(part) {
            Some(idx) => remaining = &remaining[idx + part.len()..],
            None => return false,
        }
    }

    // Last part must be a suffix (unless it's empty = trailing *)
    let last = parts[parts.len() - 1];
    if !last.is_empty() {
        return remaining.ends_with(last);
    }

    true
}

/// Check whether a detected provider is allowed by the token's provider restrictions.
///
/// Returns `Ok(())` if the provider is allowed, or `Err(reason)` if denied.
pub fn check_provider_access(
    detected_provider: &str,
    allowed_providers: Option<&[String]>,
) -> Result<(), String> {
    // If no provider restriction configured, allow all providers (backwards compatible)
    if let Some(allowed) = allowed_providers {
        if allowed.is_empty() {
            return Ok(()); // Empty array means no restriction
        }

        let detected_lower = detected_provider.to_lowercase();
        for allowed_provider in allowed {
            if allowed_provider.to_lowercase() == detected_lower {
                return Ok(());
            }
        }

        return Err(format!(
            "Provider '{}' is not allowed by this API key. Allowed providers: [{}]",
            detected_provider,
            allowed.join(", ")
        ));
    }

    Ok(())
}

/// Resolve model patterns from a list of group IDs by querying the database.
/// 6C-3 FIX: Scoped by project_id to prevent cross-tenant group resolution.
pub async fn resolve_group_models(
    pool: &sqlx::PgPool,
    group_ids: &[Uuid],
    project_id: Uuid,
) -> Vec<String> {
    if group_ids.is_empty() {
        return Vec::new();
    }

    #[derive(sqlx::FromRow)]
    struct ModelsRow {
        models: serde_json::Value,
    }

    let rows = match sqlx::query_as::<_, ModelsRow>(
        "SELECT models FROM model_access_groups WHERE id = ANY($1) AND project_id = $2",
    )
    .bind(group_ids)
    .bind(project_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to resolve model access groups: {}", e);
            return Vec::new();
        }
    };

    let mut patterns = Vec::new();
    for row in rows {
        if let Some(arr) = row.models.as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    patterns.push(s.to_string());
                }
            }
        }
    }
    patterns
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── model_matches ──────────────────────────────────────────

    #[test]
    fn test_exact_match() {
        assert!(model_matches("gpt-4o", "gpt-4o"));
        assert!(model_matches("GPT-4o", "gpt-4o")); // case insensitive
        assert!(!model_matches("gpt-4o-mini", "gpt-4o"));
    }

    #[test]
    fn test_wildcard_all() {
        assert!(model_matches("gpt-4o", "*"));
        assert!(model_matches("claude-3-opus", "*"));
    }

    #[test]
    fn test_prefix_glob() {
        assert!(model_matches("gpt-4o", "gpt-4*"));
        assert!(model_matches("gpt-4o-mini", "gpt-4*"));
        assert!(model_matches("gpt-4-turbo", "gpt-4*"));
        assert!(!model_matches("claude-3-opus", "gpt-4*"));
    }

    #[test]
    fn test_suffix_glob() {
        assert!(model_matches("gpt-4o-preview", "*-preview"));
        assert!(model_matches("claude-3-opus-preview", "*-preview"));
        assert!(!model_matches("gpt-4o", "*-preview"));
    }

    #[test]
    fn test_contains_glob() {
        assert!(model_matches("gpt-4-turbo-preview", "*turbo*"));
        assert!(model_matches("turbo-model", "*turbo*"));
        assert!(!model_matches("gpt-4o", "*turbo*"));
    }

    #[test]
    fn test_complex_glob() {
        assert!(model_matches("gpt-4o-mini", "gpt-*-mini"));
        assert!(model_matches("gpt-4-turbo-mini", "gpt-*-mini"));
        assert!(!model_matches("gpt-4o", "gpt-*-mini"));
    }

    // ── check_model_access ─────────────────────────────────────

    #[test]
    fn test_null_allowed_models_permits_all() {
        assert!(check_model_access("gpt-4o", None, &[]).is_ok());
        assert!(check_model_access("claude-3-opus", None, &[]).is_ok());
    }

    #[test]
    fn test_empty_array_permits_all() {
        let empty = serde_json::json!([]);
        assert!(check_model_access("gpt-4o", Some(&empty), &[]).is_ok());
    }

    #[test]
    fn test_exact_model_restriction() {
        let allowed = serde_json::json!(["gpt-4o", "gpt-4o-mini"]);
        assert!(check_model_access("gpt-4o", Some(&allowed), &[]).is_ok());
        assert!(check_model_access("gpt-4o-mini", Some(&allowed), &[]).is_ok());
        assert!(check_model_access("claude-3-opus", Some(&allowed), &[]).is_err());
    }

    #[test]
    fn test_glob_model_restriction() {
        let allowed = serde_json::json!(["gpt-4*", "claude-3-haiku*"]);
        assert!(check_model_access("gpt-4o", Some(&allowed), &[]).is_ok());
        assert!(check_model_access("gpt-4o-mini", Some(&allowed), &[]).is_ok());
        assert!(check_model_access("claude-3-haiku-20240307", Some(&allowed), &[]).is_ok());
        assert!(check_model_access("claude-3-opus", Some(&allowed), &[]).is_err());
    }

    #[test]
    fn test_group_models_combined_with_direct() {
        let allowed = serde_json::json!(["gpt-4o"]);
        let group = vec!["claude-3-haiku*".to_string()];
        assert!(check_model_access("gpt-4o", Some(&allowed), &group).is_ok());
        assert!(check_model_access("claude-3-haiku-20240307", Some(&allowed), &group).is_ok());
        assert!(check_model_access("claude-3-opus", Some(&allowed), &group).is_err());
    }

    #[test]
    fn test_group_models_only() {
        let group = vec!["gpt-4*".to_string(), "claude-*".to_string()];
        assert!(check_model_access("gpt-4o", None, &group).is_ok());
        assert!(check_model_access("claude-3-opus", None, &group).is_ok());
        assert!(check_model_access("llama-3-70b", None, &group).is_err());
    }

    #[test]
    fn test_empty_model_name_always_allowed() {
        let allowed = serde_json::json!(["gpt-4o"]);
        assert!(check_model_access("", Some(&allowed), &[]).is_ok());
    }

    #[test]
    fn test_denied_model_error_message() {
        let allowed = serde_json::json!(["gpt-4o"]);
        let err = check_model_access("claude-3-opus", Some(&allowed), &[]).unwrap_err();
        assert!(err.contains("claude-3-opus"));
        assert!(err.contains("not allowed"));
        assert!(err.contains("gpt-4o"));
    }

    // ── check_provider_access ─────────────────────────────────────

    #[test]
    fn test_null_allowed_providers_permits_all() {
        assert!(check_provider_access("openai", None).is_ok());
        assert!(check_provider_access("anthropic", None).is_ok());
        assert!(check_provider_access("gemini", None).is_ok());
    }

    #[test]
    fn test_empty_provider_array_permits_all() {
        let empty: Vec<String> = vec![];
        assert!(check_provider_access("openai", Some(&empty)).is_ok());
    }

    #[test]
    fn test_allowed_provider() {
        let allowed = vec!["openai".to_string(), "anthropic".to_string()];
        assert!(check_provider_access("openai", Some(&allowed)).is_ok());
        assert!(check_provider_access("anthropic", Some(&allowed)).is_ok());
        assert!(check_provider_access("OPENAI", Some(&allowed)).is_ok()); // case insensitive
    }

    #[test]
    fn test_denied_provider() {
        let allowed = vec!["openai".to_string()];
        let err = check_provider_access("anthropic", Some(&allowed)).unwrap_err();
        assert!(err.contains("anthropic"));
        assert!(err.contains("not allowed"));
        assert!(err.contains("openai"));
    }
}
