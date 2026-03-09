//! Guardrail Presets API — `POST /api/v1/guardrails/enable`
//!
//! Provides one-call enablement of pre-configured guardrail policies.
//! Instead of making admins craft raw JSON policy rules, they pick named
//! presets like `"pii_redaction"` or `"prompt_injection"` and this handler
//! expands them into proper policy rules and attaches the policy to a token.
//!
//! # Available Presets (22 total)
//! | Preset               | What it does                                                    |
//! |----------------------|-----------------------------------------------------------------|
//! | `pii_redaction`      | Redact 8 PII types in both directions (silent scrub)            |
//! | `pii_block`          | Block requests containing PII (reject with 400)                 |
//! | `prompt_injection`   | Block jailbreak + harmful content with a strict 0.3 threshold   |
//! | `hipaa`              | PII redaction tailored for healthcare: SSN, phone, DOB, email   |
//! | `pci_pan_only`       | Redact credit card + API key patterns (PAN only, not full PCI)   |
//! | `topic_fence`        | Allowlist-based topic restrictor (requires custom config)        |
//! | `toxicity`           | Block profanity + bias + hate speech (strict)                   |
//! | `profanity_filter`   | Block profanity/slurs only (lighter)                            |
//! | `competitor_block`   | Block competitor mentions (configurable names)                  |
//! | `sensitive_topics`   | Block political/religious/medical/legal/financial advice        |
//! | `gibberish_filter`   | Block encoding smuggling and random-char attacks                |
//! | `contact_info_block` | Block phone/address/URL exposure                                |
//! | `ip_protection`      | Block trade secret / confidentiality leaks                      |
//! | `strict_enterprise`  | All-in-one: injection + toxicity + PII + IP protection          |

use crate::api::AuthContext;
use crate::AppState;
use axum::{extract::State, http::StatusCode, response::Json, Extension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

// ── Request / Response Types ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EnableGuardrailsRequest {
    /// Token ID to attach the guardrail policy to.
    pub token_id: String,
    /// List of preset names to enable.
    pub presets: Vec<String>,
    /// Source of the request: "sdk", "dashboard", or "header".
    /// Used for drift detection: the dashboard warns when overriding SDK-set guardrails.
    #[serde(default = "default_source")]
    pub source: String,
    /// Optional: topic allowlist (required for `topic_fence` preset).
    #[serde(default)]
    pub topic_allowlist: Vec<String>,
    /// Optional: topic denylist (used with `topic_fence` preset).
    #[serde(default)]
    pub topic_denylist: Vec<String>,
}

fn default_source() -> String {
    "sdk".to_string()
}

#[derive(Debug, Deserialize)]
pub struct DisableGuardrailsRequest {
    /// Token ID to detach guardrail policies from.
    pub token_id: String,
    /// Policy name prefix to remove.
    /// If omitted, removes all guardrails:*:{token_id} policies from this token.
    #[serde(default)]
    pub policy_name_prefix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GuardrailsResponse {
    pub success: bool,
    pub applied_presets: Vec<String>,
    pub policy_id: Option<Uuid>,
    pub policy_name: String,
    pub skipped: Vec<String>,
    /// If guardrails were already configured, this shows who set them last.
    /// Useful for drift detection (e.g. dashboard overriding SDK-set guardrails).
    pub previous_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GuardrailsStatus {
    pub token_id: String,
    pub has_guardrails: bool,
    pub source: Option<String>,
    pub policy_id: Option<Uuid>,
    pub policy_name: Option<String>,
    pub presets: Vec<String>,
}

// ── Preset Expansion ─────────────────────────────────────────

/// A single policy rule in JSON form.
type RuleJson = serde_json::Value;

/// Expand a preset name into its policy rules.
/// Returns `None` if the preset name is unknown.
fn expand_preset(
    name: &str,
    topic_allowlist: &[String],
    topic_denylist: &[String],
) -> Option<Vec<RuleJson>> {
    let rules = match name {
        "pii_redaction" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "redact",
                "direction": "both",
                "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4"],
                "fields": [],
                "on_match": "redact"
            }
        })],

        "pii_block" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "redact",
                "direction": "request",
                "patterns": ["ssn", "email", "credit_card", "phone"],
                "fields": [],
                "on_match": "block"
            }
        })],

        "prompt_injection" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": true,
                "block_harmful": true,
                "block_code_injection": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "hipaa" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "redact",
                "direction": "both",
                // SEC 3C-1 FIX: Added `ipv4` (HIPAA PHI category 15) and `iban`.
                // NOTE: HIPAA Safe Harbor has 18 identifier categories. This preset
                // covers: SSN, email, phone/fax, DOB, MRN, IP addresses, IBANs.
                // STILL NOT COVERED by pattern matching (requires custom fields config):
                // geographic sub-state data, non-DOB dates, account numbers,
                // certificate/license numbers, vehicle/device identifiers,
                // biometrics, health plan numbers.
                "patterns": ["ssn", "email", "phone", "dob", "mrn", "ipv4", "iban"],
                "fields": [],
                "on_match": "redact"
            }
        })],

        "pci_pan_only" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "redact",
                "direction": "both",
                "patterns": ["credit_card", "api_key"],
                "fields": [],
                "on_match": "redact"
            }
        })],

        "topic_fence" => {
            if topic_allowlist.is_empty() && topic_denylist.is_empty() {
                // Return without rules — caller will skip this preset
                return None;
            }
            vec![json!({
                "when": {"always": true},
                "then": {
                    "action": "content_filter",
                    "block_jailbreak": false,
                    "block_harmful": false,
                    "topic_allowlist": topic_allowlist,
                    "topic_denylist": topic_denylist,
                    "custom_patterns": [],
                    "risk_threshold": 0.5
                }
            })]
        }

        "code_injection" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "pii_enterprise" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "redact",
                "direction": "both",
                "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4", "passport", "aws_key", "drivers_license", "mrn"],
                "fields": [],
                "on_match": "redact"
            }
        })],

        "length_limit" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.1,
                "max_content_length": 50000
            }
        })],

        // ── NEW: Toxicity & Profanity Presets ──
        "toxicity" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": true,
                "block_code_injection": false,
                "block_profanity": true,
                "block_bias": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "profanity_filter" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "block_profanity": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        // ── NEW: Business & Compliance Presets ──
        "competitor_block" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "block_competitor_mention": true,
                "competitor_names": topic_denylist,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "sensitive_topics" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "block_sensitive_topics": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "gibberish_filter" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "block_gibberish": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "contact_info_block" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "block_contact_info": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "ip_protection" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": false,
                "block_ip_leakage": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "strict_enterprise" => {
            // All-in-one enterprise preset: injection + toxicity + PII + IP
            vec![
                json!({
                    "when": {"always": true},
                    "then": {
                        "action": "content_filter",
                        "block_jailbreak": true,
                        "block_harmful": true,
                        "block_code_injection": true,
                        "block_profanity": true,
                        "block_bias": true,
                        "block_sensitive_topics": true,
                        "block_gibberish": true,
                        "block_ip_leakage": true,
                        "topic_allowlist": [],
                        "topic_denylist": [],
                        "custom_patterns": [],
                        "risk_threshold": 0.3,
                        "max_content_length": 100000
                    }
                }),
                json!({
                    "when": {"always": true},
                    "then": {
                        "action": "redact",
                        "direction": "both",
                        "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4", "passport", "aws_key", "drivers_license", "mrn"],
                        "fields": [],
                        "on_match": "redact"
                    }
                }),
            ]
        }

        // ── Output Guardrail Presets ── (these create response-phase policies)
        "output_content_filter" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": true,
                "block_harmful": true,
                "block_code_injection": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "output_pii_redaction" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "redact",
                "direction": "response",
                "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4"],
                "fields": [],
                "on_match": "redact"
            }
        })],

        "output_code_filter" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": false,
                "block_code_injection": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        "output_toxicity" => vec![json!({
            "when": {"always": true},
            "then": {
                "action": "content_filter",
                "block_jailbreak": false,
                "block_harmful": true,
                "block_profanity": true,
                "block_bias": true,
                "topic_allowlist": [],
                "topic_denylist": [],
                "custom_patterns": [],
                "risk_threshold": 0.3,
                "max_content_length": 0
            }
        })],

        _ => return None,
    };

    Some(rules)
}

/// Returns true if the preset name is an output (response-phase) guardrail.
fn is_output_preset(name: &str) -> bool {
    name.starts_with("output_")
}

// ── Handlers ──────────────────────────────────────────────────

/// `POST /api/v1/guardrails/enable`
///
/// Creates a guardrail policy from one or more presets and attaches it to the token.
pub async fn enable_guardrails(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<EnableGuardrailsRequest>,
) -> Result<Json<GuardrailsResponse>, StatusCode> {
    // Admin-only operation
    auth.require_role("admin")?;

    if payload.presets.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let project_id = auth.default_project_id();
    let mut input_rules: Vec<RuleJson> = Vec::new();
    let mut output_rules: Vec<RuleJson> = Vec::new();
    let mut applied: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for preset in &payload.presets {
        match expand_preset(preset, &payload.topic_allowlist, &payload.topic_denylist) {
            Some(rules) => {
                if is_output_preset(preset) {
                    output_rules.extend(rules);
                } else {
                    input_rules.extend(rules);
                }
                applied.push(preset.clone());
            }
            None => {
                tracing::warn!(preset, "guardrails/enable: unknown or misconfigured preset");
                skipped.push(preset.clone());
            }
        }
    }

    if input_rules.is_empty() && output_rules.is_empty() {
        return Ok(Json(GuardrailsResponse {
            success: false,
            applied_presets: applied,
            policy_id: None,
            policy_name: String::new(),
            skipped,
            previous_source: None,
        }));
    }

    // Sanitise source to one of the known values
    let source = match payload.source.as_str() {
        "dashboard" => "dashboard",
        "header" => "header",
        _ => "sdk",
    };

    // ── Check for existing guardrails (idempotent upsert) ─────
    let all_policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "guardrails/enable: failed to list policies");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Match any guardrails:*:{token_id} policy (request-phase)
    let token_suffix = format!(":{}", payload.token_id);
    let existing_input = all_policies.iter().find(|p| {
        p.name.starts_with("guardrails:") && p.name.ends_with(&token_suffix) && p.phase == "request"
    });
    // Match any guardrails-out:*:{token_id} policy (response-phase)
    let existing_output = all_policies
        .iter()
        .find(|p| p.name.starts_with("guardrails-out:") && p.name.ends_with(&token_suffix));

    let previous_source = existing_input.and_then(|p| {
        p.name
            .strip_prefix("guardrails:")
            .and_then(|rest| rest.split(':').next())
            .map(|s| s.to_string())
    });

    // ── Input (request-phase) policy ──────────────────────────
    let policy_id =
        if !input_rules.is_empty() {
            let policy_name = format!("guardrails:{}:{}", source, payload.token_id);
            let rules_value = serde_json::Value::Array(input_rules);

            if let Some(existing_policy) = existing_input {
                state.db.update_policy(
                existing_policy.id, project_id,
                None, None, Some(rules_value), None, Some(&policy_name), None,
            ).await.map_err(|e| {
                tracing::error!(error = %e, "guardrails/enable: failed to update input policy");
                StatusCode::INTERNAL_SERVER_ERROR
            })?.map(|_| true).unwrap_or(false);
                Some(existing_policy.id)
            } else {
                let id = state.db.insert_policy(
                project_id, &policy_name, "enforce", "request", rules_value, None,
            ).await.map_err(|e| {
                tracing::error!(error = %e, "guardrails/enable: failed to create input policy");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
                Some(id)
            }
        } else {
            // No input presets — if there was an existing input policy, keep it
            existing_input.map(|p| p.id)
        };

    // ── Output (response-phase) policy ────────────────────────
    let output_policy_id = if !output_rules.is_empty() {
        let out_policy_name = format!("guardrails-out:{}:{}", source, payload.token_id);
        let out_rules_value = serde_json::Value::Array(output_rules);

        if let Some(existing_policy) = existing_output {
            state.db.update_policy(
                existing_policy.id, project_id,
                None, None, Some(out_rules_value), None, Some(&out_policy_name), None,
            ).await.map_err(|e| {
                tracing::error!(error = %e, "guardrails/enable: failed to update output policy");
                StatusCode::INTERNAL_SERVER_ERROR
            })?.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Some(existing_policy.id)
        } else {
            let id = state.db.insert_policy(
                project_id, &out_policy_name, "enforce", "response", out_rules_value, None,
            ).await.map_err(|e| {
                tracing::error!(error = %e, "guardrails/enable: failed to create output policy");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Some(id)
        }
    } else {
        None
    };

    // Attach policies to the token (only if not already attached)
    let token_row = state
        .db
        .get_token(&payload.token_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut policy_ids: Vec<Uuid> = token_row.policy_ids.clone();
    if let Some(pid) = policy_id {
        if !policy_ids.contains(&pid) {
            policy_ids.push(pid);
        }
    }
    if let Some(opid) = output_policy_id {
        if !policy_ids.contains(&opid) {
            policy_ids.push(opid);
        }
    }

    state
        .db
        .set_token_policy_ids(&payload.token_id, project_id, &policy_ids)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "guardrails/enable: failed to attach policy to token");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let primary_policy_id = policy_id.or(output_policy_id);
    let policy_name = primary_policy_id
        .map(|_| format!("guardrails:{}:{}", source, payload.token_id))
        .unwrap_or_default();

    tracing::info!(
        token_id = %payload.token_id,
        input_policy_id = ?policy_id,
        output_policy_id = ?output_policy_id,
        source = source,
        presets = ?applied,
        previous_source = ?previous_source,
        "guardrails enabled"
    );

    Ok(Json(GuardrailsResponse {
        success: true,
        applied_presets: applied,
        policy_id: primary_policy_id,
        policy_name,
        skipped,
        previous_source,
    }))
}

/// `DELETE /api/v1/guardrails/disable`
///
/// Removes all auto-generated guardrail policies from a token.
pub async fn disable_guardrails(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<DisableGuardrailsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    let project_id = auth.default_project_id();
    let prefix = payload
        .policy_name_prefix
        .unwrap_or_else(|| "guardrails:".to_string());
    // Match both old "guardrails-auto-{token_id}" and new "guardrails:{src}:{token_id}" formats
    let token_suffix = format!(":{}", payload.token_id);

    // List all policies for this project and find guardrail-auto ones
    let all_policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "guardrails/disable: failed to list policies");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let guardrail_ids: Vec<Uuid> = all_policies
        .iter()
        .filter(|p| {
            // Match new input format: guardrails:{source}:{token_id}
            (p.name.starts_with(&prefix) && p.name.ends_with(&token_suffix))
            // Match new output format: guardrails-out:{source}:{token_id}
            || (p.name.starts_with("guardrails-out:") && p.name.ends_with(&token_suffix))
            // Also match legacy format: guardrails-auto-{token_id}
            || p.name == format!("guardrails-auto-{}", payload.token_id)
        })
        .map(|p| p.id)
        .collect();

    if guardrail_ids.is_empty() {
        return Ok(Json(json!({ "success": true, "removed": 0 })));
    }

    // Remove from token's policy_ids
    let token_row = state
        .db
        .get_token(&payload.token_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let remaining_ids: Vec<Uuid> = token_row
        .policy_ids
        .into_iter()
        .filter(|id| !guardrail_ids.contains(id))
        .collect();

    state
        .db
        .set_token_policy_ids(&payload.token_id, project_id, &remaining_ids)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "guardrails/disable: failed to update token");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Deactivate the guardrail policies
    for id in &guardrail_ids {
        let _ = state.db.delete_policy(*id, project_id).await;
    }

    tracing::info!(
        token_id = %payload.token_id,
        removed_count = guardrail_ids.len(),
        "guardrails disabled"
    );

    Ok(Json(json!({
        "success": true,
        "removed": guardrail_ids.len()
    })))
}

/// `GET /api/v1/guardrails/status?token_id=X`
///
/// Returns current guardrail state for a token: whether guardrails are active,
/// which source set them (sdk/dashboard/header), and the policy details.
/// Used by the dashboard for drift detection warnings.
pub async fn guardrails_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<GuardrailsStatus>, StatusCode> {
    auth.require_role("admin")?;

    let token_id = params.get("token_id").ok_or(StatusCode::BAD_REQUEST)?;
    let project_id = auth.default_project_id();

    let all_policies = state
        .db
        .list_policies(project_id, 1000, 0)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "guardrails/status: failed to list policies");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Find guardrails policy for this token (new or legacy format)
    let token_suffix = format!(":{}", token_id);
    let guardrail_policy = all_policies.iter().find(|p| {
        (p.name.starts_with("guardrails:") && p.name.ends_with(&token_suffix))
            || p.name == format!("guardrails-auto-{}", token_id)
    });

    match guardrail_policy {
        Some(policy) => {
            // Extract source from "guardrails:{source}:{token_id}"
            let source = policy
                .name
                .strip_prefix("guardrails:")
                .and_then(|rest| rest.split(':').next())
                .map(|s| s.to_string())
                // Legacy format has no source
                .or_else(|| {
                    if policy.name.starts_with("guardrails-auto-") {
                        Some("unknown".to_string())
                    } else {
                        None
                    }
                });

            // Try to extract preset names from the policy rules
            let presets = extract_preset_hints(&policy.rules);

            Ok(Json(GuardrailsStatus {
                token_id: token_id.clone(),
                has_guardrails: true,
                source,
                policy_id: Some(policy.id),
                policy_name: Some(policy.name.clone()),
                presets,
            }))
        }
        None => Ok(Json(GuardrailsStatus {
            token_id: token_id.clone(),
            has_guardrails: false,
            source: None,
            policy_id: None,
            policy_name: None,
            presets: vec![],
        })),
    }
}

/// Best-effort extraction of preset hints from policy rules JSON.
/// This looks at the action types to infer which presets were applied.
fn extract_preset_hints(rules: &serde_json::Value) -> Vec<String> {
    let mut hints = Vec::new();
    if let Some(arr) = rules.as_array() {
        for rule in arr {
            if let Some(then) = rule.get("then") {
                match then.get("action").and_then(|a| a.as_str()) {
                    Some("content_filter") => {
                        let has_jailbreak = then
                            .get("block_jailbreak")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let has_code = then
                            .get("block_code_injection")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let has_length = then
                            .get("max_content_length")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0)
                            > 0;

                        if has_jailbreak && has_code {
                            hints.push("prompt_injection".to_string());
                        } else if has_code {
                            hints.push("code_injection".to_string());
                        } else if has_jailbreak {
                            hints.push("prompt_injection".to_string());
                        }
                        if has_length {
                            hints.push("length_limit".to_string());
                        }
                    }
                    Some("redact") => {
                        let patterns = then
                            .get("patterns")
                            .and_then(|p| p.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let on_match = then
                            .get("on_match")
                            .and_then(|v| v.as_str())
                            .unwrap_or("redact");

                        if on_match == "block" {
                            hints.push("pii_block".to_string());
                        } else if patterns >= 12 {
                            hints.push("pii_enterprise".to_string());
                        } else if patterns >= 5 {
                            // Could be hipaa or pii_redaction
                            hints.push("pii_redaction".to_string());
                        } else {
                            hints.push("pci_pan_only".to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    hints
}

/// `GET /api/v1/guardrails/presets`
///
/// Returns the list of available presets with descriptions.
pub async fn list_presets() -> Json<serde_json::Value> {
    Json(json!({
        "presets": [
            // ── Privacy ──
            {
                "name": "pii_redaction",
                "description": "Silently redact 8 PII types (SSN, email, credit card, phone, API key, IBAN, DOB, IP) from both requests and responses.",
                "category": "privacy",
                "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4"]
            },
            {
                "name": "pii_enterprise",
                "description": "Enterprise-grade: redact all 12 PII types including passport, AWS key, driver's license, and MRN.",
                "category": "privacy",
                "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4", "passport", "aws_key", "drivers_license", "mrn"]
            },
            {
                "name": "pii_block",
                "description": "Reject requests that contain PII (returns HTTP 400 with detected types). For strict no-PII policies.",
                "category": "privacy",
                "patterns": ["ssn", "email", "credit_card", "phone"]
            },
            // ── Safety ──
            {
                "name": "prompt_injection",
                "description": "Block jailbreak attempts, harmful content, and code injection using 100+ regex patterns with a strict 0.3 risk threshold.",
                "category": "safety"
            },
            {
                "name": "code_injection",
                "description": "Block SQL injection, shell commands, Python exec, JS eval, XSS, and data exfiltration attempts.",
                "category": "safety"
            },
            {
                "name": "toxicity",
                "description": "Block profanity, slurs, hate speech, and biased/discriminatory language. Combines profanity + bias detection.",
                "category": "safety"
            },
            {
                "name": "profanity_filter",
                "description": "Block profanity and slurs only (lighter than full toxicity). 17 patterns for offensive language.",
                "category": "safety"
            },
            {
                "name": "gibberish_filter",
                "description": "Block encoding smuggling attacks (long base64 blocks, hex dumps, unicode escapes, repeated characters).",
                "category": "safety"
            },
            {
                "name": "topic_fence",
                "description": "Restrict the model to specific topics. Requires `topic_allowlist` or `topic_denylist` in the request.",
                "category": "safety",
                "required_fields": ["topic_allowlist OR topic_denylist"]
            },
            {
                "name": "length_limit",
                "description": "Block requests with content exceeding 50,000 characters to prevent abuse.",
                "category": "safety"
            },
            // ── Business ──
            {
                "name": "competitor_block",
                "description": "Block mentions of competitor products/services. Pass competitor names in `topic_denylist` array.",
                "category": "business",
                "required_fields": ["topic_denylist (competitor names)"]
            },
            {
                "name": "sensitive_topics",
                "description": "Block medical advice, legal advice, financial recommendations, political opinions, and religious prescriptions.",
                "category": "compliance"
            },
            {
                "name": "contact_info_block",
                "description": "Block exposure of phone numbers, physical addresses, email addresses, auth-token URLs, and social handles.",
                "category": "privacy"
            },
            // ── Compliance ──
            {
                "name": "hipaa",
                "description": "Healthcare-focused PII redaction: SSN, email, phone, date-of-birth, MRN, IP addresses, IBANs.",
                "category": "compliance",
                "patterns": ["ssn", "email", "phone", "dob", "mrn", "ipv4", "iban"],
                "warning": "Covers 7 of 18 HIPAA Safe Harbor identifiers. Missing: geographic data, non-DOB dates, account numbers, URLs, biometrics. Does not constitute HIPAA compliance without supplemental field-based configuration."
            },
            {
                "name": "pci_pan_only",
                "description": "Redact credit card numbers (PAN) and API keys. Does NOT cover CVV, expiry, or cardholder name.",
                "category": "compliance",
                "patterns": ["credit_card", "api_key"],
                "warning": "Redacts PAN only. CVV, expiry, cardholder name cannot be reliably regex-detected. This preset does not constitute PCI-DSS compliance."
            },
            // ── Enterprise ──
            {
                "name": "ip_protection",
                "description": "Block intellectual property leakage: trade secrets, NDA content, confidential markers, internal-only documents.",
                "category": "enterprise"
            },
            {
                "name": "strict_enterprise",
                "description": "All-in-one enterprise bundle: prompt injection + toxicity + PII redaction + IP protection + content length limit. Creates 2 rules.",
                "category": "enterprise"
            },
            // ── Output (Response-Phase) ──
            {
                "name": "output_content_filter",
                "description": "Scan LLM responses for jailbreak, harmful content, and code injection. Blocks unsafe output before it reaches the client.",
                "category": "output_safety",
                "phase": "response"
            },
            {
                "name": "output_pii_redaction",
                "description": "Redact PII (SSN, email, credit card, phone, etc.) from LLM responses before returning to the client.",
                "category": "output_privacy",
                "phase": "response",
                "patterns": ["ssn", "email", "credit_card", "phone", "api_key", "iban", "dob", "ipv4"]
            },
            {
                "name": "output_code_filter",
                "description": "Block responses containing executable code injection patterns (SQL, shell, eval) from the LLM.",
                "category": "output_safety",
                "phase": "response"
            },
            {
                "name": "output_toxicity",
                "description": "Block LLM responses containing profanity, bias, or harmful content before returning to the client.",
                "category": "output_safety",
                "phase": "response"
            }
        ]
    }))
}

// ── Unit Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_pii_redaction() {
        let rules = expand_preset("pii_redaction", &[], &[]).unwrap();
        assert_eq!(rules.len(), 1);
        let action = &rules[0]["then"]["action"];
        assert_eq!(action, "redact");
        let patterns = rules[0]["then"]["patterns"].as_array().unwrap();
        assert!(patterns.iter().any(|p| p == "ssn"));
        assert!(patterns.iter().any(|p| p == "iban"));
        assert!(patterns.iter().any(|p| p == "dob"));
    }

    #[test]
    fn test_expand_pii_block() {
        let rules = expand_preset("pii_block", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["on_match"], "block");
        assert_eq!(rules[0]["then"]["direction"], "request");
    }

    #[test]
    fn test_expand_prompt_injection() {
        let rules = expand_preset("prompt_injection", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_jailbreak"], true);
        assert_eq!(rules[0]["then"]["risk_threshold"], 0.3);
    }

    #[test]
    fn test_expand_hipaa() {
        let rules = expand_preset("hipaa", &[], &[]).unwrap();
        let patterns = rules[0]["then"]["patterns"].as_array().unwrap();
        assert!(patterns.iter().any(|p| p == "dob"));
        assert!(patterns.iter().any(|p| p == "ssn"));
    }

    #[test]
    fn test_expand_topic_fence_without_topics_returns_none() {
        let result = expand_preset("topic_fence", &[], &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_expand_topic_fence_with_topics() {
        let allow = vec!["coding".to_string(), "rust".to_string()];
        let rules = expand_preset("topic_fence", &allow, &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
    }

    #[test]
    fn test_expand_unknown_preset() {
        let result = expand_preset("does_not_exist", &[], &[]);
        assert!(result.is_none());
    }

    // ── NEW: Tests for expanded presets ──

    #[test]
    fn test_expand_toxicity() {
        let rules = expand_preset("toxicity", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_profanity"], true);
        assert_eq!(rules[0]["then"]["block_bias"], true);
        assert_eq!(rules[0]["then"]["block_harmful"], true);
    }

    #[test]
    fn test_expand_profanity_filter() {
        let rules = expand_preset("profanity_filter", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_profanity"], true);
        // Should not enable bias (lighter filter)
        assert!(
            rules[0]["then"]["block_bias"].is_null() || rules[0]["then"]["block_bias"] == false
        );
    }

    #[test]
    fn test_expand_competitor_block() {
        let competitors = vec!["Portkey".to_string(), "LiteLLM".to_string()];
        let rules = expand_preset("competitor_block", &[], &competitors).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_competitor_mention"], true);
        let names = rules[0]["then"]["competitor_names"].as_array().unwrap();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_expand_sensitive_topics() {
        let rules = expand_preset("sensitive_topics", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_sensitive_topics"], true);
    }

    #[test]
    fn test_expand_gibberish_filter() {
        let rules = expand_preset("gibberish_filter", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_gibberish"], true);
    }

    #[test]
    fn test_expand_contact_info_block() {
        let rules = expand_preset("contact_info_block", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_contact_info"], true);
    }

    #[test]
    fn test_expand_ip_protection() {
        let rules = expand_preset("ip_protection", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_ip_leakage"], true);
    }

    #[test]
    fn test_expand_strict_enterprise() {
        let rules = expand_preset("strict_enterprise", &[], &[]).unwrap();
        // Should produce 2 rules: content filter + PII redaction
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_jailbreak"], true);
        assert_eq!(rules[0]["then"]["block_profanity"], true);
        assert_eq!(rules[0]["then"]["block_ip_leakage"], true);
        assert_eq!(rules[1]["then"]["action"], "redact");
        let patterns = rules[1]["then"]["patterns"].as_array().unwrap();
        assert!(patterns.len() >= 12);
    }

    #[test]
    fn test_expand_output_toxicity() {
        let rules = expand_preset("output_toxicity", &[], &[]).unwrap();
        assert_eq!(rules[0]["then"]["action"], "content_filter");
        assert_eq!(rules[0]["then"]["block_profanity"], true);
        assert_eq!(rules[0]["then"]["block_bias"], true);
    }

    #[test]
    fn test_is_output_preset_new_presets() {
        assert!(is_output_preset("output_toxicity"));
        assert!(!is_output_preset("toxicity"));
        assert!(!is_output_preset("strict_enterprise"));
    }

    // ── Issue 1: PCI Preset Rename ──

    #[test]
    fn test_expand_pci_pan_only() {
        let rules = expand_preset("pci_pan_only", &[], &[]).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["then"]["action"], "redact");
        let patterns = rules[0]["then"]["patterns"].as_array().unwrap();
        assert!(patterns.iter().any(|p| p == "credit_card"));
        assert!(patterns.iter().any(|p| p == "api_key"));
    }

    #[test]
    fn test_old_pci_name_returns_none() {
        // After the rename, the old "pci" name must not resolve.
        // This is a breaking change — callers using "pci" will get it in `skipped`.
        let result = expand_preset("pci", &[], &[]);
        assert!(
            result.is_none(),
            "old 'pci' name should return None after rename"
        );
    }

    #[tokio::test]
    async fn test_list_presets_pci_pan_only_has_warning() {
        let response = list_presets().await.0;
        let presets = response["presets"].as_array().unwrap();
        let pci_preset = presets.iter().find(|p| p["name"] == "pci_pan_only");
        assert!(
            pci_preset.is_some(),
            "pci_pan_only must appear in preset list"
        );
        let pci = pci_preset.unwrap();
        assert!(
            pci["warning"].is_string(),
            "pci_pan_only preset must include a warning field"
        );
        let warning = pci["warning"].as_str().unwrap();
        assert!(
            warning.contains("PAN only"),
            "warning must mention PAN only"
        );
        assert!(warning.contains("PCI-DSS"), "warning must mention PCI-DSS");
    }

    // ── Issue 2: HIPAA Preset Warning ──

    #[tokio::test]
    async fn test_list_presets_hipaa_has_warning() {
        let response = list_presets().await.0;
        let presets = response["presets"].as_array().unwrap();
        let hipaa_preset = presets.iter().find(|p| p["name"] == "hipaa");
        assert!(hipaa_preset.is_some(), "hipaa must appear in preset list");
        let hipaa = hipaa_preset.unwrap();
        assert!(
            hipaa["warning"].is_string(),
            "hipaa preset must include a warning field"
        );
        let warning = hipaa["warning"].as_str().unwrap();
        assert!(
            warning.contains("7 of 18"),
            "warning must mention 7 of 18 identifiers"
        );
        assert!(warning.contains("HIPAA"), "warning must mention HIPAA");
    }
}
