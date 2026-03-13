//! Post-flight policy action execution.
//!
//! Extracted from `handler.rs` to enable isolated testing of post-flight
//! deny, redact, content filter, transform, schema validation, and
//! external guardrail actions without running the full proxy handler.

use std::time::Duration;

use super::handler::is_safe_webhook_url;
use crate::errors::AppError;
use crate::middleware;
use crate::models::policy::{Action, TriggeredAction};

/// Result of executing post-flight policy actions.
#[allow(dead_code)]
pub struct PostFlightResult {
    /// PII types redacted by policy (e.g. "email", "ssn")
    pub redacted_fields: Vec<String>,
    /// Shadow-mode violations detected
    pub shadow_violations: Vec<String>,
    /// Whether the response body was modified (redacted/transformed)
    pub body_modified: bool,
}

/// Execute all post-flight policy actions on the response body.
///
/// This function processes each triggered action in order:
/// - `Deny`: returns an error immediately (response suppressed)
/// - `Redact`: applies PII redaction patterns to the response JSON
/// - `ContentFilter`: scans response for harmful content
/// - `Transform`: applies JSONPath-based transformations
/// - `ValidateSchema`: validates response against JSON schema
/// - `ExternalGuardrail`: calls external moderation APIs
/// - `Log`, `Tag`, `Webhook`: observability actions (non-blocking)
///
/// Actions like `ConditionalRoute` are request-phase only and are skipped.
#[allow(dead_code)]
pub async fn execute_post_flight_actions(
    actions: &[TriggeredAction],
    parsed_resp_body: &Option<serde_json::Value>,
    resp_body_vec: &mut Vec<u8>,
    status_code: u16,
) -> Result<PostFlightResult, AppError> {
    let mut redacted_fields: Vec<String> = Vec::new();
    let mut body_modified = false;
    // FIX C-4: Keep a mutable copy of parsed body so that Redact updates
    // are visible to subsequent actions (ContentFilter, audit log, etc.).
    let mut live_body = parsed_resp_body.clone();

    for triggered in actions {
        match &triggered.action {
            Action::Deny { message, .. } => {
                tracing::warn!(
                    policy = %triggered.policy_name,
                    "post-flight deny: suppressing unsafe response"
                );
                return Err(AppError::PolicyDenied {
                    policy: triggered.policy_name.clone(),
                    reason: message.clone(),
                });
            }
            Action::Redact { .. } => {
                if let Some(mut resp_json) = live_body.clone() {
                    let result =
                        middleware::redact::apply_redact(&mut resp_json, &triggered.action, false);
                    if !result.matched_types.is_empty() {
                        tracing::info!(
                            policy = %triggered.policy_name,
                            patterns = ?result.matched_types,
                            "applied response-side redaction"
                        );
                        redacted_fields.extend(result.matched_types);
                        if let Ok(new_body) = serde_json::to_vec(&resp_json) {
                            *resp_body_vec = new_body;
                            body_modified = true;
                            // FIX C-4: Update live_body so subsequent actions see redacted content
                            live_body = Some(resp_json);
                        }
                    }
                }
            }
            Action::Log { level, tags } => match level.as_str() {
                "error" => {
                    tracing::error!(policy = %triggered.policy_name, tags = ?tags, "post-flight policy log")
                }
                "warn" => {
                    tracing::warn!(policy = %triggered.policy_name, tags = ?tags, "post-flight policy log")
                }
                _ => {
                    tracing::info!(policy = %triggered.policy_name, tags = ?tags, "post-flight policy log")
                }
            },
            Action::Tag { key, value } => {
                tracing::info!(
                    policy = %triggered.policy_name,
                    tag_key = %key, tag_value = %value,
                    "post-flight policy tag"
                );
            }
            Action::Webhook {
                url, timeout_ms, ..
            } => {
                // SEC: SSRF validation for policy-defined webhook URLs (async DNS resolution)
                if !is_safe_webhook_url(url).await {
                    tracing::warn!(
                        policy = %triggered.policy_name,
                        url = %url,
                        "post-flight policy webhook blocked: SSRF protection"
                    );
                } else {
                    let url = url.clone();
                    let timeout_ms = *timeout_ms;
                    let summary = serde_json::json!({
                        "phase": "post",
                        "policy": triggered.policy_name,
                        "response_status": status_code,
                    });
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        let _ = client
                            .post(&url)
                            .timeout(Duration::from_millis(timeout_ms))
                            .json(&summary)
                            .send()
                            .await;
                    });
                }
            }

            // ── ContentFilter (post-flight, response-side) ──
            Action::ContentFilter { .. } => {
                if let Some(ref resp_json) = live_body {
                    let result = middleware::guardrail::check_content(resp_json, &triggered.action);
                    if result.blocked {
                        let reason = result
                            .reason
                            .clone()
                            .unwrap_or_else(|| "Output guardrail blocked response".to_string());
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            risk_score = %result.risk_score,
                            patterns = ?result.matched_patterns,
                            "output content filter blocked response"
                        );
                        return Err(AppError::ContentBlocked {
                            reason: reason.clone(),
                            details: Some(serde_json::json!({
                                "phase": "response",
                                "policy": triggered.policy_name,
                                "reason": reason,
                                "matched_patterns": result.matched_patterns,
                                "confidence": result.risk_score,
                            })),
                        });
                    } else if !result.matched_patterns.is_empty() {
                        tracing::info!(
                            policy = %triggered.policy_name,
                            risk_score = %result.risk_score,
                            patterns = ?result.matched_patterns,
                            "output content filter: patterns matched but below threshold"
                        );
                    }
                }
            }

            // ── Transform (post-flight, response-side) ──
            Action::Transform { operations } => {
                if let Some(mut resp_json) = live_body.clone() {
                    let mut resp_header_mutations = middleware::redact::HeaderMutations::default();
                    for op in operations {
                        middleware::redact::apply_transform(
                            &mut resp_json,
                            &mut resp_header_mutations,
                            op,
                        );
                    }
                    tracing::info!(
                        policy = %triggered.policy_name,
                        ops = operations.len(),
                        "applied post-flight transform operations"
                    );
                    if let Ok(new_body) = serde_json::to_vec(&resp_json) {
                        *resp_body_vec = new_body;
                        body_modified = true;
                        // Keep live_body in sync so subsequent actions see transformed content
                        live_body = Some(resp_json);
                    }
                }
            }

            // ConditionalRoute is request-phase only — skip post-flight
            Action::ConditionalRoute { .. } => {
                tracing::debug!(
                    policy = %triggered.policy_name,
                    "ConditionalRoute is a request-phase action, skipping post-flight"
                );
            }

            // ── ValidateSchema (post-flight, response-side) ──
            Action::ValidateSchema {
                schema,
                not,
                message,
            } => {
                if let Some(ref resp_json) = live_body {
                    let result = middleware::guardrail::validate_schema(resp_json, schema);
                    let should_deny = if *not { result.valid } else { !result.valid };
                    if should_deny {
                        let default_msg = if *not {
                            "Response matches a forbidden schema pattern".to_string()
                        } else {
                            format!(
                                "Response failed JSON schema validation: {}",
                                result.errors.join("; ")
                            )
                        };
                        let reason = message.clone().unwrap_or(default_msg);
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            errors = ?result.errors,
                            not = not,
                            "schema validation blocked response"
                        );
                        return Err(AppError::PolicyDenied {
                            policy: triggered.policy_name.clone(),
                            reason,
                        });
                    } else {
                        tracing::debug!(
                            policy = %triggered.policy_name,
                            not = not,
                            "response passed schema validation"
                        );
                    }
                }
            }

            Action::ExternalGuardrail {
                vendor,
                endpoint,
                api_key_env,
                threshold,
                on_fail,
            } => {
                let text = live_body
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| String::from_utf8_lossy(resp_body_vec).to_string());
                match middleware::external_guardrail::check(
                    vendor,
                    endpoint,
                    api_key_env.as_deref(),
                    *threshold,
                    &text,
                )
                .await
                {
                    Ok(result) if result.blocked => {
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            vendor = ?vendor,
                            label = %result.label,
                            score = %result.score,
                            "ExternalGuardrail: post-flight violation detected"
                        );
                        if on_fail != "log" {
                            return Err(AppError::PolicyDenied {
                                policy: triggered.policy_name.clone(),
                                reason: format!(
                                    "external_guardrail({:?}): {}",
                                    vendor, result.label
                                ),
                            });
                        }
                    }
                    Ok(_) => {} // clean
                    Err(e) => {
                        tracing::error!(
                            policy = %triggered.policy_name,
                            vendor = ?vendor,
                            error = %e,
                            "ExternalGuardrail: post-flight vendor call failed (fail-open)"
                        );
                    }
                }
            }

            _ => {
                tracing::debug!(
                    policy = %triggered.policy_name,
                    action = ?triggered.action,
                    "post-flight action not applicable"
                );
            }
        }
    }

    Ok(PostFlightResult {
        redacted_fields,
        shadow_violations: Vec::new(), // shadow violations come from the policy evaluator, not here
        body_modified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::policy::TriggeredAction;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_post_flight_deny_returns_error() {
        let actions = vec![TriggeredAction {
            policy_id: Uuid::nil(),
            policy_name: "block-harmful".to_string(),
            rule_index: 0,
            action: Action::Deny {
                status: 403,
                message: "Response contains harmful content".to_string(),
            },
        }];
        let parsed_body = Some(serde_json::json!({"choices": []}));
        let mut body_vec = serde_json::to_vec(&parsed_body).unwrap();

        let result = execute_post_flight_actions(&actions, &parsed_body, &mut body_vec, 200).await;
        assert!(result.is_err(), "Deny action should return an error");
    }

    #[tokio::test]
    async fn test_post_flight_log_does_not_modify_body() {
        let actions = vec![TriggeredAction {
            policy_id: Uuid::nil(),
            policy_name: "audit-log".to_string(),
            rule_index: 0,
            action: Action::Log {
                level: "info".to_string(),
                tags: Default::default(),
            },
        }];
        let parsed_body = Some(serde_json::json!({"data": "hello"}));
        let original_bytes = serde_json::to_vec(&parsed_body.as_ref().unwrap()).unwrap();
        let mut body_vec = original_bytes.clone();

        let result = execute_post_flight_actions(&actions, &parsed_body, &mut body_vec, 200)
            .await
            .expect("Log action should succeed");

        assert!(!result.body_modified, "Log should not modify body");
        assert_eq!(body_vec, original_bytes, "Body bytes unchanged");
    }

    #[tokio::test]
    async fn test_post_flight_empty_actions_is_noop() {
        let actions: Vec<TriggeredAction> = vec![];
        let parsed_body = Some(serde_json::json!({"test": true}));
        let mut body_vec = serde_json::to_vec(&parsed_body.as_ref().unwrap()).unwrap();

        let result = execute_post_flight_actions(&actions, &parsed_body, &mut body_vec, 200)
            .await
            .expect("Empty actions should succeed");

        assert!(result.redacted_fields.is_empty());
        assert!(!result.body_modified);
    }
}
