use super::types::{AuditLogDetailRow, AuditLogRow, SessionRequestRow, SessionSummaryRow};
use super::PgStore;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Filter parameters for audit log queries.
/// All fields are optional - None means no filter for that field.
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Filter by HTTP status code
    pub status: Option<i16>,
    /// Filter by token ID
    pub token_id: Option<String>,
    /// Filter by model name (ILIKE match)
    pub model: Option<String>,
    /// Filter by policy result
    pub policy_result: Option<String>,
    /// Filter by HTTP method
    pub method: Option<String>,
    /// Filter by path substring (ILIKE match)
    pub path_contains: Option<String>,
    /// Filter by agent name
    pub agent_name: Option<String>,
    /// Filter by error type
    pub error_type: Option<String>,
    /// Filter to records created at or after this timestamp
    pub start_time: Option<DateTime<Utc>>,
    /// Filter to records created at or before this timestamp
    pub end_time: Option<DateTime<Utc>>,
}

/// Internal aggregate row — result of the GROUP BY query.
#[derive(Debug, sqlx::FromRow)]
struct SessionAggrRow {
    pub session_id: Option<String>,
    pub total_requests: i64,
    pub total_cost_usd: Option<rust_decimal::Decimal>,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_latency_ms: i64,
    pub models_used: Option<Vec<String>>,
    pub first_request_at: DateTime<Utc>,
    pub last_request_at: DateTime<Utc>,
}

impl PgStore {
    pub async fn list_audit_logs(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<AuditLogRow>> {
        let rows = sqlx::query_as::<_, AuditLogRow>(
            r#"SELECT id, created_at, token_id, method, path, upstream_status,
                      response_latency_ms, agent_name, policy_result, estimated_cost_usd,
                      shadow_violations, fields_redacted,
                      prompt_tokens, completion_tokens, model, tokens_per_second,
                      user_id, tenant_id, external_request_id, log_level,
                      tool_call_count, finish_reason, error_type, is_streaming,
                      cache_hit
               FROM audit_logs
               WHERE project_id = $1
               ORDER BY created_at DESC, id DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// List audit logs with optional filters.
    /// Uses a single query with conditional WHERE clauses for efficient filtering.
    pub async fn list_audit_logs_filtered(
        &self,
        project_id: Uuid,
        filters: AuditFilter,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<AuditLogRow>> {
        let rows = sqlx::query_as::<_, AuditLogRow>(
            r#"SELECT id, created_at, token_id, method, path, upstream_status,
                      response_latency_ms, agent_name, policy_result, estimated_cost_usd,
                      shadow_violations, fields_redacted,
                      prompt_tokens, completion_tokens, model, tokens_per_second,
                      user_id, tenant_id, external_request_id, log_level,
                      tool_call_count, finish_reason, error_type, is_streaming,
                      cache_hit
               FROM audit_logs
               WHERE project_id = $1
                 AND ($2::int2 IS NULL OR upstream_status = $2)
                 AND ($3::text IS NULL OR token_id = $3)
                 AND ($4::text IS NULL OR model ILIKE '%' || $4 || '%')
                 AND ($5::text IS NULL OR policy_result = $5)
                 AND ($6::text IS NULL OR method = $6)
                 AND ($7::text IS NULL OR path ILIKE '%' || $7 || '%')
                 AND ($8::text IS NULL OR agent_name = $8)
                 AND ($9::text IS NULL OR error_type = $9)
                 AND ($10::timestamptz IS NULL OR created_at >= $10)
                 AND ($11::timestamptz IS NULL OR created_at <= $11)
               ORDER BY created_at DESC, id DESC
               LIMIT $12 OFFSET $13"#,
        )
        .bind(project_id)
        .bind(filters.status)
        .bind(filters.token_id)
        .bind(filters.model)
        .bind(filters.policy_result)
        .bind(filters.method)
        .bind(filters.path_contains)
        .bind(filters.agent_name)
        .bind(filters.error_type)
        .bind(filters.start_time)
        .bind(filters.end_time)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Fetch a single audit log with its bodies (if available).
    pub async fn get_audit_log_detail(
        &self,
        log_id: Uuid,
        project_id: Uuid,
    ) -> anyhow::Result<Option<AuditLogDetailRow>> {
        let row = sqlx::query_as::<_, AuditLogDetailRow>(
            r#"SELECT a.id, a.created_at, a.token_id, a.method, a.path,
                      a.upstream_url, a.upstream_status,
                      a.response_latency_ms, a.agent_name, a.policy_result,
                      a.policy_mode, a.deny_reason,
                      a.estimated_cost_usd, a.shadow_violations, a.fields_redacted,
                      a.prompt_tokens, a.completion_tokens, a.model,
                      a.tokens_per_second, a.user_id, a.tenant_id,
                      a.external_request_id, a.log_level,
                      a.tool_calls, a.tool_call_count, a.finish_reason,
                      a.session_id, a.parent_span_id, a.error_type,
                      a.is_streaming, a.ttft_ms,
                      a.cache_hit, a.router_info,
                      b.request_body, b.response_body,
                      b.request_headers, b.response_headers
               FROM audit_logs a
               LEFT JOIN audit_log_bodies b ON b.audit_id = a.id
               WHERE a.id = $1 AND a.project_id = $2"#,
        )
        .bind(log_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Aggregate all audit log entries for a session (by session_id).
    ///
    /// Returns total cost, tokens, latency, models used, and per-request breakdown.
    /// This is the foundation of the "session cost rollup" feature.
    pub async fn get_session_summary(
        &self,
        session_id: &str,
        project_id: Uuid,
    ) -> anyhow::Result<Option<SessionSummaryRow>> {
        // First get the aggregate summary
        let summary = sqlx::query_as::<_, SessionAggrRow>(
            r#"
            SELECT
                session_id,
                COUNT(*)::bigint                                   AS total_requests,
                COALESCE(SUM(estimated_cost_usd), 0)              AS total_cost_usd,
                COALESCE(SUM(prompt_tokens), 0)::bigint           AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint       AS total_completion_tokens,
                COALESCE(SUM(response_latency_ms), 0)::bigint     AS total_latency_ms,
                array_remove(array_agg(DISTINCT model), NULL)     AS models_used,
                MIN(created_at)                                   AS first_request_at,
                MAX(created_at)                                   AS last_request_at
            FROM audit_logs
            WHERE session_id = $1 AND project_id = $2
            GROUP BY session_id
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(agg) = summary else {
            return Ok(None);
        };

        // Then get per-request breakdown (ordered chronologically)
        let requests = sqlx::query_as::<_, SessionRequestRow>(
            r#"
            SELECT
                id,
                created_at,
                model,
                estimated_cost_usd,
                response_latency_ms::bigint,
                prompt_tokens::integer,
                completion_tokens::integer,
                tool_call_count::smallint,
                cache_hit,
                custom_properties,
                payload_url
            FROM audit_logs
            WHERE session_id = $1 AND project_id = $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(SessionSummaryRow {
            session_id: agg.session_id,
            total_requests: agg.total_requests,
            total_cost_usd: agg.total_cost_usd,
            total_prompt_tokens: agg.total_prompt_tokens,
            total_completion_tokens: agg.total_completion_tokens,
            total_latency_ms: agg.total_latency_ms,
            models_used: agg.models_used,
            first_request_at: agg.first_request_at,
            last_request_at: agg.last_request_at,
            requests,
        }))
    }

    /// List recent sessions with per-session aggregates (no per-request breakdown).
    ///
    /// Used by the Sessions list page to show a table of all agent runs.
    pub async fn list_sessions(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<SessionSummaryRow>> {
        let rows = sqlx::query_as::<_, SessionAggrRow>(
            r#"
            SELECT
                session_id,
                COUNT(*)::bigint                                   AS total_requests,
                COALESCE(SUM(estimated_cost_usd), 0)              AS total_cost_usd,
                COALESCE(SUM(prompt_tokens), 0)::bigint           AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint       AS total_completion_tokens,
                COALESCE(SUM(response_latency_ms), 0)::bigint     AS total_latency_ms,
                array_remove(array_agg(DISTINCT model), NULL)     AS models_used,
                MIN(created_at)                                   AS first_request_at,
                MAX(created_at)                                   AS last_request_at
            FROM audit_logs
            WHERE project_id = $1 AND session_id IS NOT NULL
            GROUP BY session_id
            ORDER BY MAX(created_at) DESC, session_id
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|agg| SessionSummaryRow {
                session_id: agg.session_id,
                total_requests: agg.total_requests,
                total_cost_usd: agg.total_cost_usd,
                total_prompt_tokens: agg.total_prompt_tokens,
                total_completion_tokens: agg.total_completion_tokens,
                total_latency_ms: agg.total_latency_ms,
                models_used: agg.models_used,
                first_request_at: agg.first_request_at,
                last_request_at: agg.last_request_at,
                requests: vec![], // No per-request breakdown in list view
            })
            .collect())
    }
}
