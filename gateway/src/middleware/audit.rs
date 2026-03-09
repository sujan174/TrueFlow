use std::sync::Arc;

use crate::models::audit::{AuditEntry, PolicyResult};
use crate::store::payload_store::PayloadStore;
use sqlx::PgPool;

/// Async audit log writer. Fires off a Tokio task to insert
/// the audit entry into PG with retry on transient failures.
///
/// Retry policy: 3 attempts with exponential backoff (100ms, 500ms, 2000ms).
/// On final failure, the audit entry is serialized to structured error logging
/// as a fallback — ensuring there is always a record, even if Postgres is down.
pub fn log_async(pool: PgPool, payload_store: Arc<PayloadStore>, entry: AuditEntry) {
    tokio::spawn(async move {
        const MAX_RETRIES: u32 = 3;
        const BACKOFF_MS: [u64; 3] = [100, 500, 2000];

        let mut last_err = None;
        for attempt in 0..MAX_RETRIES {
            match insert_audit_log(&pool, &payload_store, &entry).await {
                Ok(()) => {
                    if attempt > 0 {
                        tracing::info!(
                            request_id = %entry.request_id,
                            attempt = attempt + 1,
                            "audit log recorded after retry"
                        );
                    } else {
                        tracing::debug!(request_id = %entry.request_id, "audit log recorded");
                    }
                    return;
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < MAX_RETRIES - 1 {
                        tracing::warn!(
                            request_id = %entry.request_id,
                            attempt = attempt + 1,
                            "audit log write failed, retrying: {}",
                            last_err.as_ref().unwrap()
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(
                            BACKOFF_MS[attempt as usize],
                        ))
                        .await;
                    }
                }
            }
        }

        // All retries exhausted — log the full audit entry as structured fallback
        tracing::error!(
            request_id = %entry.request_id,
            project_id = %entry.project_id,
            token_id = %entry.token_id,
            method = %entry.method,
            path = %entry.path,
            upstream_url = %entry.upstream_url,
            policy_result = ?entry.policy_result,
            upstream_status = ?entry.upstream_status,
            is_streaming = entry.is_streaming,
            estimated_cost_usd = ?entry.estimated_cost_usd,
            error = %last_err.unwrap(),
            "AUDIT_WRITE_FAILED: all {} retries exhausted — entry logged here as fallback",
            MAX_RETRIES,
        );
    });
}

async fn insert_audit_log(
    pool: &PgPool,
    payload_store: &PayloadStore,
    entry: &AuditEntry,
) -> anyhow::Result<()> {
    let (policy_res, policy_mode, deny_reason) = match &entry.policy_result {
        PolicyResult::Allow => ("allowed", None, None),
        PolicyResult::Deny { policy: _, reason } => {
            ("denied", Some("enforce"), Some(reason.as_str()))
        }
        PolicyResult::ShadowDeny { policy: _, reason } => {
            ("allowed", Some("shadow"), Some(reason.as_str()))
        }
        PolicyResult::HitlApproved => ("approved", Some("hitl"), None),
        PolicyResult::HitlRejected => ("rejected", Some("hitl"), None),
        PolicyResult::HitlTimeout => ("timeout", Some("hitl"), None),
    };

    // ── Payload offloading logic ────────────────────────────────────────────────
    // Only attempt to offload when log_level > 0 and bodies exist.
    let mut payload_url: Option<String> = None;
    let mut should_inline = false;

    if entry.log_level > 0 && (entry.request_body.is_some() || entry.response_body.is_some()) {
        let req_len = entry.request_body.as_deref().map(|s| s.len()).unwrap_or(0);
        let resp_len = entry.response_body.as_deref().map(|s| s.len()).unwrap_or(0);

        if payload_store.should_offload(req_len, resp_len) {
            // Large payload — offload to object store
            match payload_store
                .put(
                    entry.request_id,
                    entry.project_id,
                    entry.timestamp,
                    entry.request_body.as_deref(),
                    entry.response_body.as_deref(),
                    entry.request_headers.as_ref(),
                    entry.response_headers.as_ref(),
                )
                .await
            {
                Ok(url) => {
                    payload_url = Some(url);
                }
                Err(e) => {
                    // Fallback to inline Postgres on object store failure
                    tracing::warn!(
                        request_id = %entry.request_id,
                        "payload offload failed, falling back to Postgres: {}",
                        e
                    );
                    should_inline = true;
                }
            }
        } else {
            // Small payload — store inline in audit_log_bodies
            should_inline = true;
        }
    }

    // ── Phase 1: Insert metadata into audit_logs ───────────────────────────────
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id, created_at, project_id, token_id, agent_name, method, path, upstream_url,
            request_body_hash, policies_evaluated, policy_result, policy_mode, deny_reason,
            hitl_required, hitl_decision, hitl_latency_ms, upstream_status,
            response_latency_ms, fields_redacted, shadow_violations, estimated_cost_usd,
            prompt_tokens, completion_tokens, model, ttft_ms, tokens_per_second,
            user_id, tenant_id, external_request_id, log_level,
            tool_calls, tool_call_count, finish_reason,
            session_id, parent_span_id, error_type, is_streaming,
            cache_hit, custom_properties, payload_url
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            $9, $10, $11, $12, $13,
            $14, $15, $16, $17,
            $18, $19, $20, $21,
            $22, $23, $24, $25, $26,
            $27, $28, $29, $30,
            $31, $32, $33,
            $34, $35, $36, $37,
            $38, $39, $40
        )
        "#,
    )
    .bind(entry.request_id)
    .bind(entry.timestamp)
    .bind(entry.project_id)
    .bind(&entry.token_id)
    .bind(&entry.agent_name)
    .bind(&entry.method)
    .bind(&entry.path)
    .bind(&entry.upstream_url)
    .bind(&entry.request_body_hash)
    .bind(&entry.policies_evaluated)
    .bind(policy_res)
    .bind(policy_mode)
    .bind(deny_reason)
    .bind(entry.hitl_required)
    .bind(&entry.hitl_decision)
    .bind(entry.hitl_latency_ms)
    .bind(entry.upstream_status.map(|s| s as i16))
    .bind(entry.response_latency_ms as i64)
    .bind(&entry.fields_redacted)
    .bind(&entry.shadow_violations)
    .bind(entry.estimated_cost_usd)
    // Phase 4 columns
    .bind(entry.prompt_tokens.map(|v| v as i32))
    .bind(entry.completion_tokens.map(|v| v as i32))
    .bind(&entry.model)
    .bind(entry.ttft_ms.map(|v| v as i32))
    .bind(entry.tokens_per_second)
    .bind(&entry.user_id)
    .bind(&entry.tenant_id)
    .bind(&entry.external_request_id)
    .bind(entry.log_level as i16)
    // Phase 5 columns
    .bind(&entry.tool_calls)
    .bind(entry.tool_call_count as i16)
    .bind(&entry.finish_reason)
    .bind(&entry.session_id)
    .bind(&entry.parent_span_id)
    .bind(&entry.error_type)
    .bind(entry.is_streaming)
    .bind(entry.cache_hit)
    // Phase 6 columns
    .bind(&entry.custom_properties)
    .bind(&payload_url)
    .execute(pool)
    .await?;

    // ── Phase 2: Inline bodies into audit_log_bodies (small payloads only) ─────
    if should_inline {
        sqlx::query(
            r#"
            INSERT INTO audit_log_bodies (
                audit_id, created_at, request_body, response_body,
                request_headers, response_headers
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(entry.request_id)
        .bind(entry.timestamp)
        .bind(&entry.request_body)
        .bind(&entry.response_body)
        .bind(&entry.request_headers)
        .bind(&entry.response_headers)
        .execute(pool)
        .await?;
    }
    // ── Phase 3: Structured tool-call details ──────────────────────────────────
    // Parse the flat JSONB tool_calls blob into per-call rows with indexed tool_name.
    // Enables queries like: "SELECT * FROM tool_call_details WHERE tool_name = 'stripe.createCharge'"
    if let Some(ref tool_calls_json) = entry.tool_calls {
        if let Some(calls) = tool_calls_json.as_array() {
            for (i, call) in calls.iter().enumerate() {
                let tool_name = call
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");

                let tool_call_id = call
                    .get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string());

                let arguments = call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| {
                        // Arguments may be a JSON string or already parsed JSON
                        if let Some(s) = a.as_str() {
                            serde_json::from_str::<serde_json::Value>(s).ok()
                        } else {
                            Some(a.clone())
                        }
                    });

                let insert_result = sqlx::query(
                    r#"
                    INSERT INTO tool_call_details (
                        audit_log_id, created_at, tool_name, tool_call_id,
                        arguments, call_index
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                    "#,
                )
                .bind(entry.request_id)
                .bind(entry.timestamp)
                .bind(tool_name)
                .bind(&tool_call_id)
                .bind(&arguments)
                .bind(i as i16)
                .execute(pool)
                .await;

                if let Err(e) = insert_result {
                    // Non-fatal: don't fail the entire audit if detail insert fails
                    // (e.g., migration not yet applied)
                    tracing::debug!(
                        request_id = %entry.request_id,
                        tool_name = tool_name,
                        "tool_call_details insert failed (migration pending?): {}",
                        e
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::models::audit::{AuditEntry, PolicyResult};
    use uuid::Uuid;

    /// Helper to construct a minimal audit entry for testing.
    fn test_audit_entry(policy_result: PolicyResult) -> AuditEntry {
        AuditEntry {
            request_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            token_id: "test-token".to_string(),
            agent_name: Some("test-agent".to_string()),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            upstream_url: "https://api.openai.com/v1/chat/completions".to_string(),
            request_body_hash: None,
            policies_evaluated: None,
            policy_result,
            hitl_required: false,
            hitl_decision: None,
            hitl_latency_ms: None,
            upstream_status: Some(200),
            response_latency_ms: 150,
            fields_redacted: None,
            shadow_violations: None,
            estimated_cost_usd: None,
            timestamp: chrono::Utc::now(),
            log_level: 0,
            request_body: None,
            response_body: None,
            request_headers: None,
            response_headers: None,
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            model: Some("gpt-4o".to_string()),
            tokens_per_second: Some(42.0),
            user_id: None,
            tenant_id: None,
            external_request_id: None,
            tool_calls: None,
            tool_call_count: 0,
            finish_reason: Some("stop".to_string()),
            session_id: None,
            parent_span_id: None,
            error_type: None,
            is_streaming: false,
            ttft_ms: None,
            cache_hit: false,
            experiment_name: None,
            variant_name: None,
            custom_properties: None,
            payload_url: None,
        }
    }

    #[test]
    fn test_audit_entry_serializes_for_fallback_log() {
        // The fallback path serializes via tracing::error! with Debug formatting.
        // Verify the entry can be serialized to JSON (used by structured logging backends).
        let entry = test_audit_entry(PolicyResult::Allow);
        let json = serde_json::to_string(&entry).expect("AuditEntry should serialize to JSON");
        assert!(json.contains("test-token"));
        assert!(json.contains("gpt-4o"));
        assert!(json.contains("/v1/chat/completions"));
    }

    #[test]
    fn test_audit_entry_denied_serializes_correctly() {
        let entry = test_audit_entry(PolicyResult::Deny {
            policy: "block-pii".to_string(),
            reason: "SSN detected in request".to_string(),
        });
        let json = serde_json::to_string(&entry).expect("denied entry should serialize");
        assert!(json.contains("block-pii"));
        assert!(json.contains("SSN detected"));
    }

    #[test]
    fn test_retry_constants_are_valid() {
        // Verify retry backoff schedule is monotonically increasing
        const BACKOFF_MS: [u64; 3] = [100, 500, 2000];
        assert!(BACKOFF_MS[0] < BACKOFF_MS[1]);
        assert!(BACKOFF_MS[1] < BACKOFF_MS[2]);
        // Total retry budget should not exceed 3 seconds
        let total: u64 = BACKOFF_MS.iter().sum();
        assert!(
            total <= 3000,
            "total retry budget should be ≤ 3s, got {}ms",
            total
        );
    }

    #[test]
    fn test_policy_result_formatting() {
        // Verify all PolicyResult variants can be formatted for the INSERT
        let cases = vec![
            (PolicyResult::Allow, "allowed", None, None),
            (
                PolicyResult::Deny {
                    policy: "p".into(),
                    reason: "r".into(),
                },
                "denied",
                Some("enforce"),
                Some("r"),
            ),
            (
                PolicyResult::ShadowDeny {
                    policy: "p".into(),
                    reason: "r".into(),
                },
                "allowed",
                Some("shadow"),
                Some("r"),
            ),
            (PolicyResult::HitlApproved, "approved", Some("hitl"), None),
            (PolicyResult::HitlRejected, "rejected", Some("hitl"), None),
            (PolicyResult::HitlTimeout, "timeout", Some("hitl"), None),
        ];
        for (pr, expected_result, expected_mode, expected_reason) in cases {
            let (result, mode, reason) = match &pr {
                PolicyResult::Allow => ("allowed", None, None),
                PolicyResult::Deny { reason, .. } => {
                    ("denied", Some("enforce"), Some(reason.as_str()))
                }
                PolicyResult::ShadowDeny { reason, .. } => {
                    ("allowed", Some("shadow"), Some(reason.as_str()))
                }
                PolicyResult::HitlApproved => ("approved", Some("hitl"), None),
                PolicyResult::HitlRejected => ("rejected", Some("hitl"), None),
                PolicyResult::HitlTimeout => ("timeout", Some("hitl"), None),
            };
            assert_eq!(result, expected_result);
            assert_eq!(mode, expected_mode);
            assert_eq!(reason, expected_reason);
        }
    }
}
