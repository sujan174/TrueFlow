use uuid::Uuid;

use crate::AppState;

/// Builder for audit log entries. Avoids 25+ positional arguments.
#[derive(Default)]
pub(crate) struct AuditBuilder {
    pub(super) req_id: Option<Uuid>,
    pub(super) project_id: Option<Uuid>,
    pub(super) token_id: String,
    pub(super) agent_name: Option<String>,
    pub(super) method: String,
    pub(super) path: String,
    pub(super) upstream_url: String,
    pub(super) policies: Vec<String>,
    pub(super) policy_result: Option<crate::models::audit::PolicyResult>,
    pub(super) hitl_required: bool,
    pub(super) hitl_decision: Option<String>,
    pub(super) hitl_latency_ms: Option<i32>,
    pub(super) upstream_status: Option<u16>,
    pub(super) response_latency_ms: u64,
    pub(super) fields_redacted: Option<Vec<String>>,
    pub(super) shadow_violations: Option<Vec<String>>,
    pub(super) estimated_cost_usd: Option<rust_decimal::Decimal>,
    // Phase 4
    pub(super) log_level: u8,
    pub(super) request_body: Option<String>,
    pub(super) response_body: Option<String>,
    pub(super) request_headers: Option<serde_json::Value>,
    pub(super) response_headers: Option<serde_json::Value>,
    pub(super) prompt_tokens: Option<u32>,
    pub(super) completion_tokens: Option<u32>,
    pub(super) model: Option<String>,
    pub(super) tokens_per_second: Option<f32>,
    pub(super) user_id: Option<String>,
    pub(super) tenant_id: Option<String>,
    pub(super) external_request_id: Option<String>,
    // Phase 5: LLM Observability
    pub(super) tool_calls: Option<serde_json::Value>,
    pub(super) tool_call_count: u16,
    pub(super) finish_reason: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) parent_span_id: Option<String>,
    pub(super) error_type: Option<String>,
    pub(super) is_streaming: bool,
    pub(super) ttft_ms: Option<u64>,
    pub(super) cache_hit: bool,
    // A/B experiment tracking
    pub(super) experiment_name: Option<String>,
    pub(super) variant_name: Option<String>,
    // Phase 6: Just Enough Observability
    pub(super) custom_properties: Option<serde_json::Value>,
}

impl AuditBuilder {
    pub(super) fn emit(self, state: &AppState) {
        let entry = crate::models::audit::AuditEntry {
            request_id: self.req_id.unwrap_or_else(Uuid::new_v4),
            project_id: self.project_id.unwrap_or_default(),
            token_id: self.token_id,
            agent_name: self.agent_name,
            method: self.method,
            path: self.path,
            upstream_url: self.upstream_url,
            request_body_hash: None,
            policies_evaluated: Some(serde_json::json!(self.policies)),
            policy_result: self
                .policy_result
                .unwrap_or(crate::models::audit::PolicyResult::Allow),
            hitl_required: self.hitl_required,
            hitl_decision: self.hitl_decision,
            hitl_latency_ms: self.hitl_latency_ms,
            upstream_status: self.upstream_status,
            response_latency_ms: self.response_latency_ms,
            fields_redacted: self.fields_redacted,
            shadow_violations: self.shadow_violations,
            estimated_cost_usd: self.estimated_cost_usd,
            timestamp: chrono::Utc::now(),
            log_level: self.log_level,
            request_body: self.request_body,
            response_body: self.response_body,
            request_headers: self.request_headers,
            response_headers: self.response_headers,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            model: self.model,
            tokens_per_second: self.tokens_per_second,
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            external_request_id: self.external_request_id,
            tool_calls: self.tool_calls,
            tool_call_count: self.tool_call_count,
            finish_reason: self.finish_reason,
            session_id: self.session_id,
            parent_span_id: self.parent_span_id,
            error_type: self.error_type,
            is_streaming: self.is_streaming,
            ttft_ms: self.ttft_ms,
            cache_hit: self.cache_hit,
            experiment_name: self.experiment_name,
            variant_name: self.variant_name,
            custom_properties: self.custom_properties,
            payload_url: None, // set by audit middleware after potential offload
        };
        // ── Observability Export ──────────────────────────────────────
        // Fan out to Prometheus, Langfuse, and DataDog (non-blocking).
        state.observer.record(&entry);

        crate::middleware::audit::log_async(
            state.db.pool().clone(),
            state.payload_store.clone(),
            entry,
        );
    }
}

/// Helper to create a pre-populated AuditBuilder from shared request context.
#[allow(clippy::too_many_arguments)]
pub(crate) fn base_audit(
    req_id: Uuid,
    project_id: Uuid,
    token_id: &str,
    agent_name: Option<String>,
    method: &str,
    path: &str,
    upstream_url: &str,
    policies: &[crate::models::policy::Policy],
    hitl_required: bool,
    hitl_decision: Option<String>,
    hitl_latency_ms: Option<i32>,
    user_id: Option<String>,
    tenant_id: Option<String>,
    external_request_id: Option<String>,
    session_id: Option<String>,
    parent_span_id: Option<String>,
    custom_properties: Option<serde_json::Value>,
) -> AuditBuilder {
    AuditBuilder {
        req_id: Some(req_id),
        project_id: Some(project_id),
        token_id: token_id.to_string(),
        agent_name,
        method: method.to_string(),
        path: path.to_string(),
        upstream_url: upstream_url.to_string(),
        policies: policies.iter().map(|p| p.name.clone()).collect(),
        hitl_required,
        hitl_decision,
        hitl_latency_ms,
        user_id,
        tenant_id,
        external_request_id,
        session_id,
        parent_span_id,
        custom_properties,
        ..Default::default()
    }
}
