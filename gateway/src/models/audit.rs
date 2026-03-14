use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub request_id: Uuid,
    pub project_id: Uuid,
    pub token_id: String,
    pub agent_name: Option<String>,
    pub method: String,
    pub path: String,
    pub upstream_url: String,
    pub request_body_hash: Option<String>,
    pub policies_evaluated: Option<serde_json::Value>,
    pub policy_result: PolicyResult,
    pub hitl_required: bool,
    pub hitl_decision: Option<String>,
    pub hitl_latency_ms: Option<i32>,
    pub upstream_status: Option<u16>,
    pub response_latency_ms: u64,
    pub fields_redacted: Option<Vec<String>>,
    pub shadow_violations: Option<Vec<String>>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub timestamp: DateTime<Utc>,

    // ── Phase 4: Observability ────────────────────────────────
    /// Privacy level: 0 = metadata only, 1 = redacted bodies, 2 = full debug
    pub log_level: u8,
    /// Request body (None at level 0, PII-scrubbed at level 1, raw at level 2)
    pub request_body: Option<String>,
    /// Response body (same gating as request_body)
    pub response_body: Option<String>,
    /// Request headers as JSON (level 2 only)
    pub request_headers: Option<serde_json::Value>,
    /// Response headers as JSON (level 2 only)
    pub response_headers: Option<serde_json::Value>,
    /// Prompt (input) token count from upstream response
    pub prompt_tokens: Option<u32>,
    /// Completion (output) token count from upstream response
    pub completion_tokens: Option<u32>,
    /// Model name (e.g., "gpt-4o")
    pub model: Option<String>,
    /// Tokens per second (completion_tokens / elapsed_secs)
    pub tokens_per_second: Option<f32>,
    /// Caller-supplied user ID from X-User-ID header
    pub user_id: Option<String>,
    /// Caller-supplied tenant ID from X-Tenant-ID header
    pub tenant_id: Option<String>,
    /// Caller-supplied request ID from X-Request-ID header
    pub external_request_id: Option<String>,

    // ── Phase 5: LLM Observability ───────────────────────────
    /// Tool calls extracted from LLM response (JSON array)
    pub tool_calls: Option<serde_json::Value>,
    /// Number of tool calls in this response
    pub tool_call_count: u16,
    /// Finish reason (stop, tool_calls, length, content_filter, etc.)
    pub finish_reason: Option<String>,
    /// Session ID for grouping conversations (from X-Session-ID)
    pub session_id: Option<String>,
    /// Parent span ID for nested calls (from X-Parent-Span-ID)
    pub parent_span_id: Option<String>,
    /// Classified error type (rate_limit, context_too_long, etc.)
    pub error_type: Option<String>,
    /// Whether this was a streaming (SSE) response
    pub is_streaming: bool,
    /// Time to first token in milliseconds
    pub ttft_ms: Option<u64>,
    /// Whether this response was served from cache
    pub cache_hit: bool,
    // ── A/B Experiment Tracking (Split action) ───────────────────
    /// Experiment name from the Split policy action (for grouping in analytics).
    pub experiment_name: Option<String>,
    /// Variant name selected for this request (e.g., "control" or "experiment").
    pub variant_name: Option<String>,
    // ── Just Enough Observability ─────────────────────────────
    /// Arbitrary key-value properties from X-Properties header (GIN-indexed JSONB).
    /// Example: {"env": "prod", "customer": "acme", "run_id": "agent-run-42"}
    pub custom_properties: Option<serde_json::Value>,
    /// Object store URL when request/response bodies were offloaded from Postgres.
    /// When set, fetch bodies from PayloadStore::get(url) instead of audit_log_bodies.
    pub payload_url: Option<String>,
    /// Whether this request exceeded the spend cap due to a race condition.
    /// The upstream API call was made before the cap was checked, so the cost was incurred
    /// but not tracked. This is a billing anomaly that requires attention.
    pub spend_cap_overrun: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyResult {
    Allow,
    Deny { policy: String, reason: String },
    ShadowDeny { policy: String, reason: String },
    HitlApproved,
    HitlRejected,
    HitlTimeout,
}

// ── Feature 8: Async Guardrail Violation ─────────────────────────────────────

/// A guardrail violation detected asynchronously after the response was sent.
#[derive(Debug, Clone)]
pub struct AsyncGuardrailViolation {
    pub token_id: String,
    pub policy_name: String,
    pub matched_patterns: Vec<String>,
    pub risk_score: f32,
}

/// Emit an async guardrail violation to structured logs + optional webhook.
pub async fn emit_async_violation(violation: AsyncGuardrailViolation) {
    tracing::warn!(
        event_type = "async_guardrail_violation",
        token_id = %violation.token_id,
        policy = %violation.policy_name,
        patterns = ?violation.matched_patterns,
        risk_score = %violation.risk_score,
        "async guardrail violation — response already sent"
    );

    // Notify TRUEFLOW_ASYNC_GUARDRAIL_WEBHOOK if configured (best-effort)
    if let Ok(webhook_url) = std::env::var("TRUEFLOW_ASYNC_GUARDRAIL_WEBHOOK") {
        let payload = serde_json::json!({
            "event_type": "async_guardrail_violation",
            "token_id": violation.token_id,
            "policy_name": violation.policy_name,
            "matched_patterns": violation.matched_patterns,
            "risk_score": violation.risk_score,
        });
        let client = reqwest::Client::new();
        let _ = client
            .post(&webhook_url)
            .timeout(std::time::Duration::from_secs(5))
            .json(&payload)
            .send()
            .await;
    }
}
