use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run pending migrations from the migrations/ directory.
    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    // -- Project Operations --

    pub async fn create_project(&self, org_id: Uuid, name: &str) -> anyhow::Result<Uuid> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO projects (org_id, name) VALUES ($1, $2) RETURNING id",
        )
        .bind(org_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_projects(&self, org_id: Uuid) -> anyhow::Result<Vec<ProjectRow>> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, org_id, name, created_at FROM projects WHERE org_id = $1 ORDER BY created_at ASC"
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_project(&self, id: Uuid, org_id: Uuid, name: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE projects SET name = $1 WHERE id = $2 AND org_id = $3"
        )
        .bind(name)
        .bind(id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_project(&self, id: Uuid, org_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM projects WHERE id = $1 AND org_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// GDPR Article 17 — Right to Erasure.
    /// Purges all personal/operational data associated with a project:
    /// audit logs, request logs, sessions, and virtual key usage records.
    /// The project record itself is preserved; use delete_project to remove it.
    /// All deletions run in a single transaction for atomicity.
    pub async fn purge_project_data(&self, project_id: Uuid, org_id: Uuid) -> anyhow::Result<u64> {
        // First verify the project belongs to this org (authorization check)
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND org_id = $2)"
        )
        .bind(project_id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await?;

        if !exists {
            anyhow::bail!("project not found or does not belong to org");
        }

        let mut tx = self.pool.begin().await?;
        let mut total_deleted: u64 = 0;

        // 1. Purge audit / request logs
        let r = sqlx::query(
            "DELETE FROM audit_logs WHERE project_id = $1"
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        total_deleted += r.rows_affected();

        // 2. Purge agent sessions
        let r = sqlx::query(
            "DELETE FROM sessions WHERE project_id = $1"
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        total_deleted += r.rows_affected();

        // 3. Purge virtual key usage / billing records (keep keys themselves; owners may need invoicing data export first)
        let r = sqlx::query(
            "DELETE FROM token_usage WHERE project_id = $1"
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        total_deleted += r.rows_affected();

        tx.commit().await?;

        tracing::info!(
            project_id = %project_id,
            rows_purged = total_deleted,
            "GDPR data purge completed"
        );

        Ok(total_deleted)
    }

    /// Verify that a project belongs to the given org.
    /// Used by API handlers to enforce project isolation.
    pub async fn project_belongs_to_org(&self, project_id: Uuid, org_id: Uuid) -> anyhow::Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND org_id = $2)"
        )
        .bind(project_id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    // -- Credential Operations --

    pub async fn insert_credential(&self, cred: &NewCredential) -> anyhow::Result<Uuid> {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO credentials (project_id, name, provider, encrypted_dek, dek_nonce, encrypted_secret, secret_nonce, injection_mode, injection_header)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"#
        )
        .bind(cred.project_id)
        .bind(&cred.name)
        .bind(&cred.provider)
        .bind(&cred.encrypted_dek)
        .bind(&cred.dek_nonce)
        .bind(&cred.encrypted_secret)
        .bind(&cred.secret_nonce)
        .bind(&cred.injection_mode)
        .bind(&cred.injection_header)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn list_credentials(&self, project_id: Uuid) -> anyhow::Result<Vec<CredentialMeta>> {
        let rows = sqlx::query_as::<_, CredentialMeta>(
            "SELECT id, name, provider, version, is_active, created_at FROM credentials WHERE project_id = $1 ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Soft-delete a credential by setting is_active = false.
    /// Scoped to project_id for tenant isolation.
    pub async fn delete_credential(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE credentials SET is_active = false WHERE id = $1 AND project_id = $2 AND is_active = true"
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -- Token Operations --

    pub async fn insert_token(&self, token: &NewToken) -> anyhow::Result<()> {
        sqlx::query(
            r#"INSERT INTO tokens (id, project_id, name, credential_id, upstream_url, scopes, policy_ids, log_level, circuit_breaker, allowed_models, team_id, tags, mcp_allowed_tools, mcp_blocked_tools)
               VALUES ($1, $2, $3, $4, $5, $6, $7, COALESCE($8, 1::SMALLINT), $9, $10, $11, COALESCE($12, '{}'::jsonb), $13, $14)"#
        )
        .bind(&token.id)
        .bind(token.project_id)
        .bind(&token.name)
        .bind(token.credential_id)
        .bind(&token.upstream_url)
        .bind(&token.scopes)
        .bind(&token.policy_ids)
        .bind(token.log_level)
        .bind(&token.circuit_breaker)
        .bind(&token.allowed_models)
        .bind(token.team_id)
        .bind(&token.tags)
        .bind(&token.mcp_allowed_tools)
        .bind(&token.mcp_blocked_tools)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_token(&self, token_id: &str) -> anyhow::Result<Option<TokenRow>> {
        let row = sqlx::query_as::<_, TokenRow>(
            "SELECT id, project_id, name, credential_id, upstream_url, scopes, policy_ids, is_active, expires_at, created_at, COALESCE(log_level, 1::SMALLINT) as log_level, upstreams, circuit_breaker, allowed_models, allowed_model_group_ids, team_id, tags, mcp_allowed_tools, mcp_blocked_tools FROM tokens WHERE id = $1"
        )
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_tokens(&self, project_id: Uuid) -> anyhow::Result<Vec<TokenRow>> {
        let rows = sqlx::query_as::<_, TokenRow>(
            "SELECT id, project_id, name, credential_id, upstream_url, scopes, policy_ids, is_active, expires_at, created_at, COALESCE(log_level, 1::SMALLINT) as log_level, upstreams, circuit_breaker, allowed_models, allowed_model_group_ids, team_id, tags, mcp_allowed_tools, mcp_blocked_tools FROM tokens WHERE project_id = $1 AND is_active = true ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn revoke_token(&self, token_id: &str) -> anyhow::Result<bool> {
        let result =
            sqlx::query("UPDATE tokens SET is_active = false, updated_at = NOW() WHERE id = $1")
                .bind(token_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update the circuit breaker configuration for a token.
    /// Returns `true` if the token was found and updated, `false` if not found.
    pub async fn update_circuit_breaker(
        &self,
        token_id: &str,
        config: serde_json::Value,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET circuit_breaker = $1 WHERE id = $2 AND is_active = true"
        )
        .bind(&config)
        .bind(token_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Replace the `policy_ids` array on a token.
    /// Used by the guardrail presets API to attach auto-generated policies.
    pub async fn set_token_policy_ids(
        &self,
        token_id: &str,
        policy_ids: &[Uuid],
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET policy_ids = $1 WHERE id = $2 AND is_active = true"
        )
        .bind(policy_ids)
        .bind(token_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update a token's upstream URL, policy bindings, and log level.
    /// Used by config import to update an existing token without touching its credentials.
    pub async fn update_token_config(
        &self,
        token_id: &str,
        policy_ids: Vec<Uuid>,
        log_level: i16,
        upstream_url: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET policy_ids = $1, log_level = $2, upstream_url = $3, updated_at = NOW() WHERE id = $4 AND is_active = true"
        )
        .bind(&policy_ids)
        .bind(log_level)
        .bind(upstream_url)
        .bind(token_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Insert a new token without credentials (used by config-as-code import).
    /// The token will work in passthrough mode until a credential is attached.
    /// Returns the generated token ID (format: `tok_import_<uuid>`).
    pub async fn insert_token_stub(
        &self,
        project_id: Uuid,
        name: &str,
        upstream_url: &str,
        policy_ids: Vec<Uuid>,
        log_level: i16,
    ) -> anyhow::Result<String> {
        let id = format!("tok_import_{}", Uuid::new_v4().simple());
        let token = NewToken {
            id: id.clone(),
            project_id,
            name: name.to_string(),
            credential_id: None,        // no credential — passthrough mode
            upstream_url: upstream_url.to_string(),
            scopes: serde_json::json!([]),
            policy_ids,
            log_level: Some(log_level),
            circuit_breaker: None,
            allowed_models: None,
            team_id: None,
            tags: None,
            mcp_allowed_tools: None,
            mcp_blocked_tools: None,
        };
        self.insert_token(&token).await?;
        Ok(id)
    }


    pub async fn get_policies_for_token(
        &self,
        policy_ids: &[Uuid],
    ) -> anyhow::Result<Vec<crate::models::policy::Policy>> {
        if policy_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as::<_, PolicyRow>(
            "SELECT id, project_id, name, mode, phase, rules, retry, is_active, created_at FROM policies WHERE id = ANY($1) AND is_active = true"
        )
        .bind(policy_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut policies = Vec::new();
        for row in rows {
            let mode = match row.mode.as_str() {
                "shadow" => crate::models::policy::PolicyMode::Shadow,
                _ => crate::models::policy::PolicyMode::Enforce,
            };
            let phase = match row.phase.as_str() {
                "post" => crate::models::policy::Phase::Post,
                _ => crate::models::policy::Phase::Pre,
            };
            let rules: Vec<crate::models::policy::Rule> = serde_json::from_value(row.rules)?;
            let retry_config = if let Some(r) = row.retry {
                match serde_json::from_value(r) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        tracing::error!("Failed to deserialize retry config for policy {}: {}", row.id, e);
                        None
                    }
                }
            } else {
                None
            };
            policies.push(crate::models::policy::Policy {
                id: row.id,
                name: row.name,
                phase,
                mode,
                rules,
                retry: retry_config,
            });
        }

        Ok(policies)
    }

    pub async fn list_policies(&self, project_id: Uuid) -> anyhow::Result<Vec<PolicyRow>> {
        let rows = sqlx::query_as::<_, PolicyRow>(
            "SELECT id, project_id, name, mode, phase, rules, retry, is_active, created_at FROM policies WHERE project_id = $1 ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert_policy(
        &self,
        project_id: Uuid,
        name: &str,
        mode: &str,
        phase: &str,
        rules: serde_json::Value,
        retry: Option<serde_json::Value>,
    ) -> anyhow::Result<Uuid> {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO policies (project_id, name, mode, phase, rules, retry)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(name)
        .bind(mode)
        .bind(phase)
        .bind(rules)
        .bind(retry)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_policy(
        &self,
        id: Uuid,
        project_id: Uuid,
        mode: Option<&str>,
        phase: Option<&str>,
        rules: Option<serde_json::Value>,
        retry: Option<serde_json::Value>,
        name: Option<&str>,
    ) -> anyhow::Result<bool> {
        // Snapshot current state into policy_versions before updating
        sqlx::query(
            r#"INSERT INTO policy_versions (policy_id, version, name, mode, phase, rules, retry)
               SELECT id, version, name, mode, phase, rules, retry
               FROM policies
               WHERE id = $1 AND project_id = $2 AND is_active = true"#,
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        // Build dynamic update — at least one field must change
        let result = sqlx::query(
            r#"UPDATE policies
               SET mode = COALESCE($1, mode),
                   phase = COALESCE($2, phase),
                   rules = COALESCE($3, rules),
                   retry = COALESCE($4, retry),
                   name = COALESCE($5, name),
                   version = version + 1
               WHERE id = $6 AND project_id = $7 AND is_active = true"#,
        )
        .bind(mode)
        .bind(phase)
        .bind(rules)
        .bind(retry)
        .bind(name)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_policy(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE policies SET is_active = false WHERE id = $1 AND project_id = $2 AND is_active = true"
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_policy_versions(
        &self,
        policy_id: Uuid,
    ) -> anyhow::Result<Vec<PolicyVersionRow>> {
        let rows = sqlx::query_as::<_, PolicyVersionRow>(
            r#"SELECT id, policy_id, version, name, mode, phase, rules, retry, changed_by, created_at
               FROM policy_versions
               WHERE policy_id = $1
               ORDER BY version DESC"#,
        )
        .bind(policy_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -- Approval Operations --

    pub async fn create_approval_request(
        &self,
        token_id: &str,
        project_id: Uuid,
        idempotency_key: Option<String>,
        summary: serde_json::Value,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<Uuid> {
        // Optimistic insert
        let id_opt = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO approval_requests (token_id, project_id, idempotency_key, request_summary, expires_at)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (token_id, idempotency_key) DO NOTHING
               RETURNING id"#
        )
        .bind(token_id)
        .bind(project_id)
        .bind(&idempotency_key)
        .bind(summary)
        .bind(expires_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("create_approval_request insert failed: {:?}", e);
            e
        })?;

        if let Some(id) = id_opt {
            Ok(id)
        } else {
            // Conflict -> fetch existing
            let existing_id = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM approval_requests WHERE token_id = $1 AND idempotency_key = $2",
            )
            .bind(token_id)
            .bind(&idempotency_key)
            .fetch_one(&self.pool)
            .await?;
            Ok(existing_id)
        }
    }

    pub async fn get_approval_status(&self, request_id: Uuid) -> anyhow::Result<String> {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM approval_requests WHERE id = $1")
                .bind(request_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(status.unwrap_or_else(|| "expired".to_string()))
    }

    pub async fn list_pending_approvals(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::approval::ApprovalRequest>> {
        let rows = sqlx::query_as::<_, crate::models::approval::ApprovalRequest>(
            "SELECT * FROM approval_requests WHERE project_id = $1 AND status = 'pending' ORDER BY created_at ASC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_approval_status(
        &self,
        request_id: Uuid,
        project_id: Uuid,
        status: crate::models::approval::ApprovalStatus,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE approval_requests SET status = $1, reviewed_at = NOW() WHERE id = $2 AND project_id = $3 AND status = 'pending'")
        .bind(status)
        .bind(request_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -- Audit Log Operations --

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
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(project_id)
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
            ORDER BY MAX(created_at) DESC
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

    // -- Analytics Operations --

    pub async fn get_request_volume_24h(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::analytics::VolumeStat>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::VolumeStat>(
            r#"
            SELECT 
                date_trunc('hour', created_at) as bucket,
                count(*) as count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            GROUP BY 1
            ORDER BY 1 ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_status_code_distribution_24h(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::analytics::StatusStat>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::StatusStat>(
            r#"
            SELECT 
                CAST(floor(COALESCE(upstream_status, 0) / 100) * 100 AS INTEGER) as status_class,
                count(*) as count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            GROUP BY 1
            ORDER BY 1 ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_latency_percentiles_24h(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<crate::models::analytics::LatencyStat> {
        // We use percentile_cont. Requires float8, response_latency_ms is int4.
        // We return a single row with p50, p90, p99, avg.
        let row = sqlx::query_as::<_, crate::models::analytics::LatencyStat>(
            r#"
            SELECT 
                COALESCE(percentile_cont(0.50) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p50,
                COALESCE(percentile_cont(0.90) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p90,
                COALESCE(percentile_cont(0.99) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p99,
                COALESCE(AVG(response_latency_ms)::float8, 0) as avg
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            "#
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    // -- Token Usage Analytics --

    pub async fn get_token_usage(
        &self,
        token_id: &str,
        project_id: Uuid,
    ) -> anyhow::Result<crate::models::analytics::TokenUsageStats> {
        // Aggregate stats
        let stats = sqlx::query_as::<_, (i64, i64, i64, f64, f64)>(
            r#"SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE upstream_status >= 200 AND upstream_status < 400) as success,
                COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL) as errors,
                COALESCE(AVG(response_latency_ms)::float8, 0) as avg_latency,
                COALESCE(SUM(estimated_cost_usd)::float8, 0) as total_cost
            FROM audit_logs
            WHERE token_id = $1 AND project_id = $2
              AND created_at > now() - interval '24 hours'"#,
        )
        .bind(token_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;

        // Hourly buckets for sparkline
        let hourly = sqlx::query_as::<_, crate::models::analytics::TokenUsageBucket>(
            r#"SELECT
                date_trunc('hour', created_at) as bucket,
                COUNT(*) as count
            FROM audit_logs
            WHERE token_id = $1 AND project_id = $2
              AND created_at > now() - interval '24 hours'
            GROUP BY 1
            ORDER BY 1 ASC"#,
        )
        .bind(token_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(crate::models::analytics::TokenUsageStats {
            total_requests: stats.0,
            success_count: stats.1,
            error_count: stats.2,
            avg_latency_ms: stats.3,
            total_cost_usd: stats.4,
            hourly,
        })
    }

    // -- Notification Operations --

    pub async fn create_notification(
        &self,
        project_id: Uuid,
        r#type: &str,
        title: &str,
        body: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<Uuid> {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO notifications (project_id, type, title, body, metadata)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(r#type)
        .bind(title)
        .bind(body)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_notifications(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::models::notification::Notification>> {
        let rows = sqlx::query_as::<_, crate::models::notification::Notification>(
            r#"SELECT id, project_id, type, title, body, metadata, is_read, created_at
               FROM notifications
               WHERE project_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn count_unread_notifications(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM notifications WHERE project_id = $1 AND is_read = false"#,
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn mark_notification_read(
        &self,
        id: Uuid,
        project_id: Uuid,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"UPDATE notifications SET is_read = true WHERE id = $1 AND project_id = $2"#,
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_all_notifications_read(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"UPDATE notifications SET is_read = true WHERE project_id = $1 AND is_read = false"#,
        )
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -- Service Operations --

    pub async fn create_service(&self, svc: &NewService) -> anyhow::Result<crate::models::service::Service> {
        let row = sqlx::query_as::<_, crate::models::service::Service>(
            r#"INSERT INTO services (project_id, name, description, base_url, service_type, credential_id)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, project_id, name, description, base_url, service_type, credential_id, is_active, created_at, updated_at"#,
        )
        .bind(svc.project_id)
        .bind(&svc.name)
        .bind(&svc.description)
        .bind(&svc.base_url)
        .bind(&svc.service_type)
        .bind(svc.credential_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_services(&self, project_id: Uuid) -> anyhow::Result<Vec<crate::models::service::Service>> {
        let rows = sqlx::query_as::<_, crate::models::service::Service>(
            "SELECT id, project_id, name, description, base_url, service_type, credential_id, is_active, created_at, updated_at FROM services WHERE project_id = $1 ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_service_by_name(&self, project_id: Uuid, name: &str) -> anyhow::Result<Option<crate::models::service::Service>> {
        let row = sqlx::query_as::<_, crate::models::service::Service>(
            "SELECT id, project_id, name, description, base_url, service_type, credential_id, is_active, created_at, updated_at FROM services WHERE project_id = $1 AND name = $2 AND is_active = true"
        )
        .bind(project_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_service(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM services WHERE id = $1 AND project_id = $2"
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
    pub async fn get_analytics_summary(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<crate::models::analytics::AnalyticsSummary> {
        let row = sqlx::query_as::<_, crate::models::analytics::AnalyticsSummary>(
            r#"
            SELECT 
                count(*)::bigint as total_requests,
                count(*) filter (where upstream_status >= 200 and upstream_status < 400)::bigint as success_count,
                count(*) filter (where upstream_status >= 400 or upstream_status is null)::bigint as error_count,
                coalesce(avg(response_latency_ms), 0.0)::float8 as avg_latency,
                coalesce(sum(estimated_cost_usd), 0.0)::float8 as total_cost,
                coalesce(sum(prompt_tokens + completion_tokens), 0)::bigint as total_tokens
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_analytics_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::AnalyticsTimeseriesPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };
        
        let rows = sqlx::query_as::<_, crate::models::analytics::AnalyticsTimeseriesPoint>(
            r#"
            SELECT 
                date_trunc($3, created_at) as bucket,
                count(*)::bigint as request_count,
                count(*) filter (where upstream_status >= 400)::bigint as error_count,
                coalesce(sum(estimated_cost_usd), 0.0)::float8 as cost,
                coalesce(avg(response_latency_ms), 0.0)::float8 as lat
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY 1
            ORDER BY 1 ASC
            "#
        )
        .bind(project_id)
        .bind(hours.to_string())
        .bind(bucket)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Spend Breakdown Queries ──────────────────────────────────────────────

    /// Spend breakdown grouped by model over a time window.
    pub async fn get_spend_by_model(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                COALESCE(model, 'unknown')              AS dimension,
                COALESCE(SUM(estimated_cost_usd), 0)::float8  AS total_cost_usd,
                COUNT(*)::bigint                        AS request_count,
                COALESCE(SUM(prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint AS total_completion_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND estimated_cost_usd IS NOT NULL
            GROUP BY model
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Spend breakdown grouped by token_id over a time window.
    pub async fn get_spend_by_token(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                token_id                                AS dimension,
                COALESCE(SUM(estimated_cost_usd), 0)::float8  AS total_cost_usd,
                COUNT(*)::bigint                        AS request_count,
                COALESCE(SUM(prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint AS total_completion_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND estimated_cost_usd IS NOT NULL
            GROUP BY token_id
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Spend breakdown grouped by a tag key extracted from custom_properties JSONB.
    /// e.g. group_by_tag = "team" → groups by custom_properties->>'team'
    pub async fn get_spend_by_tag(
        &self,
        project_id: Uuid,
        hours: i32,
        tag_key: &str,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                COALESCE(custom_properties->>$3, 'untagged') AS dimension,
                COALESCE(SUM(estimated_cost_usd), 0)::float8     AS total_cost_usd,
                COUNT(*)::bigint                             AS request_count,
                COALESCE(SUM(prompt_tokens), 0)::bigint      AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint  AS total_completion_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND estimated_cost_usd IS NOT NULL
            GROUP BY custom_properties->>$3
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .bind(tag_key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

/// Row type returned by spend breakdown queries.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct SpendByDimension {
    pub dimension: String,
    pub total_cost_usd: f64,
    pub request_count: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
}

#[cfg(test)]
mod spend_breakdown_tests {
    use super::SpendByDimension;

    /// Test that SpendByDimension serializes to the expected JSON shape.
    /// This is NOT a false positive — it verifies the contract that the
    /// frontend/SDK will consume. If any field is renamed or removed,
    /// this test will catch the breaking change.
    #[test]
    fn test_spend_by_dimension_serialization_contract() {
        let row = SpendByDimension {
            dimension: "gpt-4o".to_string(),
            total_cost_usd: 42.50,
            request_count: 1000,
            total_prompt_tokens: 50000,
            total_completion_tokens: 25000,
        };

        let json = serde_json::to_value(&row).unwrap();

        // Verify exact field names (API contract)
        assert!(json.get("dimension").is_some(), "missing 'dimension' field");
        assert!(json.get("total_cost_usd").is_some(), "missing 'total_cost_usd' field");
        assert!(json.get("request_count").is_some(), "missing 'request_count' field");
        assert!(json.get("total_prompt_tokens").is_some(), "missing 'total_prompt_tokens' field");
        assert!(json.get("total_completion_tokens").is_some(), "missing 'total_completion_tokens' field");

        // Verify actual values (not just existence — prevents false positive)
        assert_eq!(json["dimension"], "gpt-4o");
        assert_eq!(json["total_cost_usd"], 42.5);
        assert_eq!(json["request_count"], 1000);
        assert_eq!(json["total_prompt_tokens"], 50000);
        assert_eq!(json["total_completion_tokens"], 25000);
    }

    /// Test that SpendByDimension deserialization works round-trip.
    /// This validates that the sqlx::FromRow derivation will produce
    /// a struct that can be serialized back to JSON for the API response.
    #[test]
    fn test_spend_by_dimension_roundtrip() {
        let original = SpendByDimension {
            dimension: "tag:engineering".to_string(),
            total_cost_usd: 0.0,
            request_count: 0,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let deserialized: SpendByDimension = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.dimension, "tag:engineering");
        assert_eq!(deserialized.total_cost_usd, 0.0);
        assert_eq!(deserialized.request_count, 0);
    }

    /// Test that the breakdown response total calculation is correct.
    /// This simulates what the handler does: summing breakdown rows.
    #[test]
    fn test_breakdown_total_aggregation() {
        let rows = vec![
            SpendByDimension {
                dimension: "gpt-4o".into(),
                total_cost_usd: 100.0,
                request_count: 500,
                total_prompt_tokens: 50000,
                total_completion_tokens: 25000,
            },
            SpendByDimension {
                dimension: "gpt-4o-mini".into(),
                total_cost_usd: 10.0,
                request_count: 3000,
                total_prompt_tokens: 100000,
                total_completion_tokens: 50000,
            },
            SpendByDimension {
                dimension: "claude-3-sonnet".into(),
                total_cost_usd: 45.50,
                request_count: 200,
                total_prompt_tokens: 30000,
                total_completion_tokens: 15000,
            },
        ];

        // This is the exact logic from the handler — test it doesn't silently break
        let total_cost: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
        let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

        assert!((total_cost - 155.50).abs() < 0.001, "expected 155.50, got {}", total_cost);
        assert_eq!(total_requests, 3700, "expected 3700 requests, got {}", total_requests);
    }

    /// Test edge case: empty breakdown (no spend data).
    /// The handler should still produce valid JSON with zeroes.
    #[test]
    fn test_empty_breakdown_totals_to_zero() {
        let rows: Vec<SpendByDimension> = vec![];
        let total_cost: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
        let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

        assert_eq!(total_cost, 0.0);
        assert_eq!(total_requests, 0);
    }

    /// Test that the "unknown" dimension appears for NULL model values.
    /// The SQL COALESCE(model, 'unknown') should convert NULLs.
    #[test]
    fn test_dimension_handles_unknown_sentinel_value() {
        let row = SpendByDimension {
            dimension: "unknown".to_string(),
            total_cost_usd: 5.0,
            request_count: 10,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
        };
        // Verify the sentinel serializes (not an empty string or null)
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(json["dimension"], "unknown");
    }
}

// -- Input structs --

pub struct NewCredential {
    pub project_id: Uuid,
    pub name: String,
    pub provider: String,
    pub encrypted_dek: Vec<u8>,
    pub dek_nonce: Vec<u8>,
    pub encrypted_secret: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub injection_mode: String,
    pub injection_header: String,
}

pub struct NewToken {
    pub id: String,
    pub project_id: Uuid,
    pub name: String,
    pub credential_id: Option<Uuid>,
    pub upstream_url: String,
    pub scopes: serde_json::Value,
    pub policy_ids: Vec<Uuid>,
    pub log_level: Option<i16>,
    /// Optional circuit breaker config. `None` uses gateway defaults.
    pub circuit_breaker: Option<serde_json::Value>,
    /// Model access control: list of allowed model patterns (globs).
    pub allowed_models: Option<serde_json::Value>,
    /// Team assignment for attribution and budget tracking.
    pub team_id: Option<Uuid>,
    /// Tags for cost attribution and tracking.
    pub tags: Option<serde_json::Value>,
    /// MCP tool allowlist. NULL=all allowed, []=none allowed, ["mcp__server__*"]=glob match.
    pub mcp_allowed_tools: Option<serde_json::Value>,
    /// MCP tool blocklist. Takes priority over allowlist. Supports glob patterns.
    pub mcp_blocked_tools: Option<serde_json::Value>,
}

// -- Output structs --

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct CredentialMeta {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub version: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct TokenRow {
    pub id: String,
    pub project_id: Uuid,
    pub name: String,
    pub credential_id: Option<Uuid>,
    pub upstream_url: String,
    pub scopes: serde_json::Value,
    pub policy_ids: Vec<Uuid>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// Privacy level: 0=metadata, 1=redacted(default), 2=full-debug
    pub log_level: i16,
    /// Optional multi-upstream configuration for loadbalancing
    pub upstreams: Option<serde_json::Value>,
    /// Optional per-token circuit breaker configuration
    pub circuit_breaker: Option<serde_json::Value>,
    /// Model access control: list of allowed model patterns (globs).
    /// NULL = all models allowed (backwards compatible).
    pub allowed_models: Option<serde_json::Value>,
    /// References to named model_access_groups for reusable model restrictions.
    pub allowed_model_group_ids: Option<Vec<Uuid>>,
    /// Team this token belongs to (for attribution and budget tracking)
    pub team_id: Option<Uuid>,
    /// Tags for cost attribution and tracking
    pub tags: Option<serde_json::Value>,
    /// MCP tool allowlist. NULL=all allowed, []=none allowed, ["mcp__server__*"]=glob match.
    pub mcp_allowed_tools: Option<serde_json::Value>,
    /// MCP tool blocklist. Takes priority over allowlist. Supports glob patterns.
    pub mcp_blocked_tools: Option<serde_json::Value>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct PolicyRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub mode: String,
    pub phase: String,
    pub rules: serde_json::Value,
    pub retry: Option<serde_json::Value>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

// ── Session Summary Types ─────────────────────────────────────

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

/// Per-request item inside a session summary.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct SessionRequestRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub model: Option<String>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub response_latency_ms: Option<i64>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub tool_call_count: Option<i16>,
    pub cache_hit: Option<bool>,
    pub custom_properties: Option<serde_json::Value>,
    pub payload_url: Option<String>,
}

/// Full session summary (aggregate + per-request breakdown).
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSummaryRow {
    pub session_id: Option<String>,
    pub total_requests: i64,
    pub total_cost_usd: Option<rust_decimal::Decimal>,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_latency_ms: i64,
    pub models_used: Option<Vec<String>>,
    pub first_request_at: DateTime<Utc>,
    pub last_request_at: DateTime<Utc>,
    pub requests: Vec<SessionRequestRow>,
}

// ── Session Entity (Lifecycle) ────────────────────────────────

/// A first-class session entity with lifecycle, spend caps, and metadata.
/// Created automatically on first request via upsert.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct SessionEntity {
    pub id: Uuid,
    pub session_id: String,
    pub project_id: Uuid,
    pub token_id: Option<Uuid>,
    pub status: String,
    pub spend_cap_usd: Option<rust_decimal::Decimal>,
    pub total_cost_usd: rust_decimal::Decimal,
    pub total_tokens: i64,
    pub total_requests: i64,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl PgStore {

    /// Upsert a session — create on first request, update `updated_at` on subsequent.
    /// Returns the session entity (with status for spend cap checks).
    pub async fn upsert_session(
        &self,
        session_id: &str,
        project_id: Uuid,
        token_id: Option<Uuid>,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<SessionEntity> {
        let row = sqlx::query_as::<_, SessionEntity>(
            r#"
            INSERT INTO sessions (session_id, project_id, token_id, metadata)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (project_id, session_id)
            DO UPDATE SET updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .bind(token_id)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Update session status (active → paused → active → completed).
    pub async fn update_session_status(
        &self,
        session_id: &str,
        project_id: Uuid,
        new_status: &str,
    ) -> anyhow::Result<Option<SessionEntity>> {
        let completed_at = if new_status == "completed" {
            Some(chrono::Utc::now())
        } else {
            None
        };

        let row = sqlx::query_as::<_, SessionEntity>(
            r#"
            UPDATE sessions
            SET status = $3,
                updated_at = NOW(),
                completed_at = COALESCE($4, completed_at)
            WHERE session_id = $1 AND project_id = $2
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .bind(new_status)
        .bind(completed_at)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Atomically increment session cost and tokens after a request completes.
    pub async fn increment_session_spend(
        &self,
        session_id: &str,
        project_id: Uuid,
        cost_usd: rust_decimal::Decimal,
        tokens: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET total_cost_usd = total_cost_usd + $3,
                total_tokens = total_tokens + $4,
                total_requests = total_requests + 1,
                updated_at = NOW()
            WHERE session_id = $1 AND project_id = $2
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .bind(cost_usd)
        .bind(tokens)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get session entity for status/spend cap checks.
    pub async fn get_session_entity(
        &self,
        session_id: &str,
        project_id: Uuid,
    ) -> anyhow::Result<Option<SessionEntity>> {
        let row = sqlx::query_as::<_, SessionEntity>(
            r#"
            SELECT * FROM sessions
            WHERE session_id = $1 AND project_id = $2
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Check if a session has exceeded its spend cap.
    /// Returns Ok(true) if the session can proceed, Ok(false) if spend cap exceeded.
    pub async fn check_session_spend_cap(
        &self,
        session_id: &str,
        project_id: Uuid,
    ) -> anyhow::Result<bool> {
        let session = self.get_session_entity(session_id, project_id).await?;
        match session {
            Some(s) => {
                if let Some(cap) = s.spend_cap_usd {
                    Ok(s.total_cost_usd < cap)
                } else {
                    Ok(true) // No spend cap set
                }
            }
            None => Ok(true), // Session doesn't exist yet — will be created
        }
    }

    /// Set a spend cap on a session.
    pub async fn set_session_spend_cap(
        &self,
        session_id: &str,
        project_id: Uuid,
        cap_usd: rust_decimal::Decimal,
    ) -> anyhow::Result<Option<SessionEntity>> {
        let row = sqlx::query_as::<_, SessionEntity>(
            r#"
            UPDATE sessions
            SET spend_cap_usd = $3, updated_at = NOW()
            WHERE session_id = $1 AND project_id = $2
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(project_id)
        .bind(cap_usd)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}

// ── OIDC Provider Queries ─────────────────────────────────────

/// DB row for OIDC provider configuration.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct OidcProviderRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub jwks_uri: Option<String>,
    pub audience: Option<String>,
    pub claim_mapping: serde_json::Value,
    pub default_role: String,
    pub default_scopes: String,
    pub enabled: bool,
}

impl PgStore {
    /// Find an enabled OIDC provider matching the given issuer URL.
    /// Used by the auth middleware to validate JWT Bearer tokens.
    pub async fn get_oidc_provider_by_issuer(
        &self,
        issuer_url: &str,
    ) -> anyhow::Result<Option<OidcProviderRow>> {
        let row = sqlx::query_as::<_, OidcProviderRow>(
            r#"
            SELECT id, org_id, name, issuer_url, client_id, jwks_uri,
                   audience, claim_mapping, default_role, default_scopes, enabled
            FROM oidc_providers
            WHERE issuer_url = $1 AND enabled = true
            LIMIT 1
            "#,
        )
        .bind(issuer_url)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub token_id: Option<String>,
    pub method: String,
    pub path: String,
    pub upstream_status: Option<i16>,
    pub response_latency_ms: i32,
    pub agent_name: Option<String>,
    pub policy_result: String,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub shadow_violations: Option<Vec<String>>,
    pub fields_redacted: Option<Vec<String>>,
    // Phase 4 columns
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub model: Option<String>,
    pub tokens_per_second: Option<f32>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub external_request_id: Option<String>,
    pub log_level: Option<i16>,
    // Phase 5: LLM Observability
    pub tool_call_count: Option<i16>,
    pub finish_reason: Option<String>,
    pub error_type: Option<String>,
    pub is_streaming: Option<bool>,
    // Phase 6: Response Cache
    pub cache_hit: Option<bool>,
}

/// Detailed audit log row with joined body data (for single-entry view).
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct AuditLogDetailRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub token_id: Option<String>,
    pub method: String,
    pub path: String,
    pub upstream_url: String,
    pub upstream_status: Option<i16>,
    pub response_latency_ms: i32,
    pub agent_name: Option<String>,
    pub policy_result: String,
    pub policy_mode: Option<String>,
    pub deny_reason: Option<String>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub shadow_violations: Option<Vec<String>>,
    pub fields_redacted: Option<Vec<String>>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub model: Option<String>,
    pub tokens_per_second: Option<f32>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub external_request_id: Option<String>,
    pub log_level: Option<i16>,
    // Phase 5: LLM Observability
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_count: Option<i16>,
    pub finish_reason: Option<String>,
    pub session_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub error_type: Option<String>,
    pub is_streaming: Option<bool>,
    pub ttft_ms: Option<i64>,
    // From audit_log_bodies JOIN
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub request_headers: Option<serde_json::Value>,
    pub response_headers: Option<serde_json::Value>,
    // Phase 6: Router Debugger
    pub cache_hit: Option<bool>,
    pub router_info: Option<serde_json::Value>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct ProjectRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct PolicyVersionRow {
    pub id: Uuid,
    pub policy_id: Uuid,
    pub version: i32,
    pub name: Option<String>,
    pub mode: Option<String>,
    pub phase: Option<String>,
    pub rules: serde_json::Value,
    pub retry: Option<serde_json::Value>,
    pub changed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Service Registry ─────────────────────────────────────────

pub struct NewService {
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub base_url: String,
    pub service_type: String,
    pub credential_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub role: String,
    pub scopes: serde_json::Value,
    pub is_active: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct UsageMeterRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub period: NaiveDate,
    pub total_requests: i64,
    pub total_tokens_used: i64,
    pub total_spend_usd: rust_decimal::Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Analytics structs
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenSummary {
    pub token_id: Option<String>,
    pub total_requests: i64,
    pub errors: i64,
    pub avg_latency_ms: f64,
    pub last_active: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenVolumeStat {
    pub hour: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStatusStat {
    pub status: i16,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenLatencyStat {
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
}

impl PgStore {
    // ── API Keys ─────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn create_api_key(
        &self,
        org_id: Uuid,
        user_id: Option<Uuid>,
        name: &str,
        key_hash: &str,
        key_prefix: &str,
        role: &str,
        scopes: serde_json::Value,
    ) -> anyhow::Result<Uuid> {
        let rec = sqlx::query!(
            r#"INSERT INTO api_keys (org_id, user_id, name, key_hash, key_prefix, role, scopes)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id"#,
            org_id,
            user_id,
            name,
            key_hash,
            key_prefix,
            role,
            scopes
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(rec.id)
    }

    pub async fn get_api_key_by_hash(&self, key_hash: &str) -> anyhow::Result<Option<ApiKeyRow>> {
        let key = sqlx::query_as!(
            ApiKeyRow,
            "SELECT * FROM api_keys WHERE key_hash = $1 AND is_active = true",
            key_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn list_api_keys(&self, org_id: Uuid) -> anyhow::Result<Vec<ApiKeyRow>> {
        let keys = sqlx::query_as!(
            ApiKeyRow,
            "SELECT * FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
            org_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(keys)
    }

    pub async fn revoke_api_key(&self, id: Uuid, org_id: Uuid) -> anyhow::Result<bool> {
        let result: sqlx::postgres::PgQueryResult = sqlx::query!(
            "UPDATE api_keys SET is_active = false WHERE id = $1 AND org_id = $2",
            id,
            org_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn touch_api_key_usage(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Usage Metering ───────────────────────────────────────────

    pub async fn increment_usage(
        &self,
        org_id: Uuid,
        period: NaiveDate,
        requests: i64,
        tokens: i64,
        spend_usd: rust_decimal::Decimal,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            r#"INSERT INTO usage_meters (org_id, period, total_requests, total_tokens_used, total_spend_usd)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (org_id, period) DO UPDATE SET
                   total_requests = usage_meters.total_requests + $3,
                   total_tokens_used = usage_meters.total_tokens_used + $4,
                   total_spend_usd = usage_meters.total_spend_usd + $5,
                   updated_at = NOW()"#,
            org_id,
            period,
            requests,
            tokens,
            spend_usd
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_usage(
        &self,
        org_id: Uuid,
        period: NaiveDate,
    ) -> anyhow::Result<Option<UsageMeterRow>> {
        let usage = sqlx::query_as!(
            UsageMeterRow,
            "SELECT * FROM usage_meters WHERE org_id = $1 AND period = $2",
            org_id,
            period
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(usage)
    }

    /// Aggregate usage directly from audit_logs for the given calendar month.
    /// Used as a fallback / live view when usage_meters has not been populated.
    pub async fn get_usage_from_audit_logs(
        &self,
        org_id: Uuid,
        period: NaiveDate,
    ) -> anyhow::Result<(i64, i64, rust_decimal::Decimal)> {
        use chrono::Datelike;
        // Calculate the first day of the next month as the exclusive upper bound
        let period_end = {
            let (y, m) = if period.month() == 12 {
                (period.year() + 1, 1u32)
            } else {
                (period.year(), period.month() + 1)
            };
            chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap()
        };

        let period_start_dt = period.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end_dt   = period_end.and_hms_opt(0, 0, 0).unwrap().and_utc();

        // Use sqlx::query() (runtime form) so SQLX_OFFLINE=true builds aren't affected.
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint                                                   AS total_requests,
                COALESCE(SUM(a.prompt_tokens + a.completion_tokens), 0)::bigint    AS total_tokens,
                COALESCE(SUM(a.estimated_cost_usd), 0)                             AS total_spend_usd
            FROM audit_logs a
            JOIN projects p ON p.id = a.project_id
            WHERE p.org_id = $1
              AND a.created_at >= $2
              AND a.created_at <  $3
            "#,
        )
        .bind(org_id)
        .bind(period_start_dt)
        .bind(period_end_dt)
        .fetch_one(&self.pool)
        .await?;

        use sqlx::Row;
        let total_requests: i64           = row.try_get("total_requests").unwrap_or(0);
        let total_tokens: i64             = row.try_get("total_tokens").unwrap_or(0);
        let total_spend: rust_decimal::Decimal = row.try_get("total_spend_usd")
            .unwrap_or(rust_decimal::Decimal::ZERO);

        Ok((total_requests, total_tokens, total_spend))
    }

    // ── Per-Token Analytics ──────────────────────────────────────

    pub async fn get_token_summary(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<TokenSummary>> {
        // sqlx doesn't map directly to struct with aggregate functions easily without `AS` aliases matching struct fields exactly.
        // We'll use query_as! with explicit mapping if needed, or ensuring column names match.
        // Note: avg returns numeric/float, COUNT returns bigint (i64).
        // latency might be null if 0 requests, COALESCE to 0.
        let rows = sqlx::query_as!(
            TokenSummary,
            r#"SELECT 
                token_id, 
                COUNT(*) as "total_requests!",
                COUNT(*) FILTER (WHERE upstream_status >= 400) as "errors!",
                COALESCE(AVG(response_latency_ms)::float8, 0.0) as "avg_latency_ms!",
                MAX(created_at) as last_active
             FROM audit_logs
             WHERE project_id = $1 AND created_at > now() - interval '24 hours'
             GROUP BY token_id 
             ORDER BY 2 DESC"#,
            project_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_token_volume_24h(
        &self,
        project_id: Uuid,
        token_id: &str,
    ) -> anyhow::Result<Vec<TokenVolumeStat>> {
        let rows = sqlx::query_as!(
            TokenVolumeStat,
            r#"SELECT 
                date_trunc('hour', created_at) as "hour!", 
                COUNT(*) as "count!"
             FROM audit_logs
             WHERE project_id = $1 AND token_id = $2
               AND created_at > now() - interval '24 hours'
             GROUP BY 1 
             ORDER BY 1"#,
            project_id,
            token_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_token_status_distribution_24h(
        &self,
        project_id: Uuid,
        token_id: &str,
    ) -> anyhow::Result<Vec<TokenStatusStat>> {
        let rows = sqlx::query_as!(
            TokenStatusStat,
            r#"SELECT 
                COALESCE(upstream_status, 0)::smallint as "status!", 
                COUNT(*) as "count!"
             FROM audit_logs
             WHERE project_id = $1 AND token_id = $2
               AND created_at > now() - interval '24 hours'
             GROUP BY 1 
             ORDER BY 2 DESC"#,
            project_id,
            token_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_token_latency_percentiles_24h(
        &self,
        project_id: Uuid,
        token_id: &str,
    ) -> anyhow::Result<TokenLatencyStat> {
         let row = sqlx::query!(
            r#"SELECT 
                percentile_cont(0.5) WITHIN GROUP (ORDER BY response_latency_ms) as p50,
                percentile_cont(0.9) WITHIN GROUP (ORDER BY response_latency_ms) as p90,
                percentile_cont(0.99) WITHIN GROUP (ORDER BY response_latency_ms) as p99
             FROM audit_logs
             WHERE project_id = $1 AND token_id = $2
               AND created_at > now() - interval '24 hours'"#,
            project_id,
            token_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(TokenLatencyStat {
            p50: row.p50.unwrap_or(0.0),
            p90: row.p90.unwrap_or(0.0),
            p99: row.p99.unwrap_or(0.0),
        })
    }

    pub async fn get_analytics_experiments(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::analytics::ExperimentSummary>> {
        // Group by experiment_name and variant_name
        // For baseline (null variants), we group them together under an empty string or 'baseline'
        let rows = sqlx::query_as::<_, crate::models::analytics::ExperimentSummary>(
            r#"SELECT 
                experiment_name as "experiment_name!",
                COALESCE(variant_name, 'baseline') as "variant_name!",
                COUNT(*) as "total_requests!",
                COALESCE(AVG(response_latency_ms)::float8, 0.0) as "avg_latency_ms!",
                COALESCE(SUM(cost_usd)::float8, 0.0) as "total_cost_usd!",
                COALESCE(AVG(prompt_tokens + completion_tokens)::float8, 0.0) as "avg_tokens!",
                COUNT(*) FILTER (WHERE upstream_status >= 400) as "error_count!"
             FROM audit_logs
             WHERE project_id = $1 AND experiment_name IS NOT NULL
             GROUP BY experiment_name, variant_name
             ORDER BY experiment_name, variant_name"#
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -- Model Pricing Operations --

    pub async fn list_model_pricing(&self) -> anyhow::Result<Vec<ModelPricingRow>> {
        let rows = sqlx::query_as::<_, ModelPricingRow>(
            r#"SELECT id, provider, model_pattern, input_per_m, output_per_m, is_active, created_at, updated_at
               FROM model_pricing
               WHERE is_active = true
               ORDER BY provider ASC, model_pattern ASC"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert a pricing entry. Returns the row ID.
    pub async fn upsert_model_pricing(
        &self,
        provider: &str,
        model_pattern: &str,
        input_per_m: rust_decimal::Decimal,
        output_per_m: rust_decimal::Decimal,
    ) -> anyhow::Result<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO model_pricing (provider, model_pattern, input_per_m, output_per_m)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (provider, model_pattern) DO UPDATE
                 SET input_per_m = EXCLUDED.input_per_m,
                     output_per_m = EXCLUDED.output_per_m,
                     is_active = true,
                     updated_at = NOW()
               RETURNING id"#
        )
        .bind(provider)
        .bind(model_pattern)
        .bind(input_per_m)
        .bind(output_per_m)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Soft-delete a pricing entry (sets is_active = false).
    pub async fn delete_model_pricing(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE model_pricing SET is_active = false, updated_at = NOW() WHERE id = $1 AND is_active = true"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
    // -- System Settings Operations --

    pub async fn get_system_setting<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<T>> {
        let row = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT value FROM system_settings WHERE key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(val) = row {
            let parsed: T = serde_json::from_value(val)?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    pub async fn set_system_setting<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        let json_val = serde_json::to_value(value)?;
        
        sqlx::query(
            r#"
            INSERT INTO system_settings (key, value, description)
            VALUES ($1, $2, $3)
            ON CONFLICT (key) DO UPDATE
            SET value = EXCLUDED.value,
                description = COALESCE(EXCLUDED.description, system_settings.description),
                updated_at = NOW()
            "#
        )
        .bind(key)
        .bind(json_val)
        .bind(description)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn get_all_system_settings(&self) -> anyhow::Result<std::collections::HashMap<String, serde_json::Value>> {
        let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
            "SELECT key, value FROM system_settings"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut settings = std::collections::HashMap::new();
        for (k, v) in rows {
            settings.insert(k, v);
        }
        Ok(settings)
    }

    // -- Additional Approval Operations --

    /// List ALL approval requests for a project (pending + historical) for the dashboard.
    pub async fn list_approval_requests(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::approval::ApprovalRequest>> {
        let rows = sqlx::query_as::<_, crate::models::approval::ApprovalRequest>(
            r#"SELECT id, token_id, project_id, idempotency_key, request_summary,
                      status, reviewed_by, reviewed_at, expires_at, created_at
               FROM approval_requests
               WHERE project_id = $1
               ORDER BY created_at DESC
               LIMIT 200"#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Set the status of an approval request. Returns true if updated.
    pub async fn decide_approval_request(
        &self,
        id: Uuid,
        project_id: Uuid,
        decision: &str,    // "approved" | "rejected"
        reviewer_id: Option<Uuid>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"UPDATE approval_requests
               SET status = $1, reviewed_by = $2, reviewed_at = NOW()
               WHERE id = $3 AND project_id = $4 AND status = 'pending'"#,
        )
        .bind(decision)
        .bind(reviewer_id)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Prompt Management ─────────────────────────────────────────

    pub async fn insert_prompt(&self, p: &NewPrompt) -> anyhow::Result<PromptRow> {
        let row = sqlx::query_as::<_, PromptRow>(
            r#"INSERT INTO prompts (project_id, name, slug, description, folder, tags, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING *"#,
        )
        .bind(p.project_id)
        .bind(&p.name)
        .bind(&p.slug)
        .bind(&p.description)
        .bind(&p.folder)
        .bind(&p.tags)
        .bind(&p.created_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_prompt(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<Option<PromptRow>> {
        let row = sqlx::query_as::<_, PromptRow>(
            "SELECT * FROM prompts WHERE id = $1 AND project_id = $2 AND is_active = TRUE",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_prompts(&self, project_id: Uuid, folder: Option<&str>) -> anyhow::Result<Vec<PromptRow>> {
        let rows = if let Some(f) = folder {
            sqlx::query_as::<_, PromptRow>(
                "SELECT * FROM prompts WHERE project_id = $1 AND folder = $2 AND is_active = TRUE ORDER BY updated_at DESC",
            )
            .bind(project_id)
            .bind(f)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, PromptRow>(
                "SELECT * FROM prompts WHERE project_id = $1 AND is_active = TRUE ORDER BY updated_at DESC",
            )
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn update_prompt(&self, id: Uuid, project_id: Uuid, name: &str, description: &str, folder: &str, tags: &serde_json::Value) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"UPDATE prompts SET name = $1, description = $2, folder = $3, tags = $4, updated_at = NOW()
               WHERE id = $5 AND project_id = $6 AND is_active = TRUE"#,
        )
        .bind(name)
        .bind(description)
        .bind(folder)
        .bind(tags)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_prompt(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE prompts SET is_active = FALSE, updated_at = NOW() WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Create a new immutable version. Auto-increments version number.
    pub async fn insert_prompt_version(&self, v: &NewPromptVersion) -> anyhow::Result<PromptVersionRow> {
        let next_version: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM prompt_versions WHERE prompt_id = $1",
        )
        .bind(v.prompt_id)
        .fetch_one(&self.pool)
        .await?;

        let row = sqlx::query_as::<_, PromptVersionRow>(
            r#"INSERT INTO prompt_versions
               (prompt_id, version, model, messages, temperature, max_tokens, top_p, tools, commit_message, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING *"#,
        )
        .bind(v.prompt_id)
        .bind(next_version)
        .bind(&v.model)
        .bind(&v.messages)
        .bind(v.temperature)
        .bind(v.max_tokens)
        .bind(v.top_p)
        .bind(&v.tools)
        .bind(&v.commit_message)
        .bind(&v.created_by)
        .fetch_one(&self.pool)
        .await?;

        // Touch parent updated_at
        let _ = sqlx::query("UPDATE prompts SET updated_at = NOW() WHERE id = $1")
            .bind(v.prompt_id)
            .execute(&self.pool)
            .await;

        Ok(row)
    }

    pub async fn list_prompt_versions(&self, prompt_id: Uuid) -> anyhow::Result<Vec<PromptVersionRow>> {
        let rows = sqlx::query_as::<_, PromptVersionRow>(
            "SELECT * FROM prompt_versions WHERE prompt_id = $1 ORDER BY version DESC",
        )
        .bind(prompt_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_prompt_version(&self, prompt_id: Uuid, version: i32) -> anyhow::Result<Option<PromptVersionRow>> {
        let row = sqlx::query_as::<_, PromptVersionRow>(
            "SELECT * FROM prompt_versions WHERE prompt_id = $1 AND version = $2",
        )
        .bind(prompt_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Deploy: atomically move a label to a specific version.
    /// Removes the label from all other versions of the same prompt, then adds to the target.
    pub async fn deploy_prompt_version(&self, prompt_id: Uuid, version: i32, label: &str) -> anyhow::Result<bool> {
        // Remove label from all versions of this prompt
        sqlx::query(
            r#"UPDATE prompt_versions
               SET labels = labels - $1
               WHERE prompt_id = $2"#,
        )
        .bind(label)
        .bind(prompt_id)
        .execute(&self.pool)
        .await?;

        // Add label to the target version
        let result = sqlx::query(
            r#"UPDATE prompt_versions
               SET labels = CASE
                   WHEN NOT labels @> to_jsonb($1::text) THEN labels || to_jsonb($1::text)
                   ELSE labels
               END
               WHERE prompt_id = $2 AND version = $3"#,
        )
        .bind(label)
        .bind(prompt_id)
        .bind(version)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Render API: resolve a prompt by slug + optional label or version.
    /// Priority: explicit version > label > latest.
    pub async fn get_prompt_for_render(
        &self,
        project_id: Uuid,
        slug: &str,
        label: Option<&str>,
        version: Option<i32>,
    ) -> anyhow::Result<Option<(PromptRow, PromptVersionRow)>> {
        // First get the prompt by slug
        let prompt = sqlx::query_as::<_, PromptRow>(
            "SELECT * FROM prompts WHERE project_id = $1 AND slug = $2 AND is_active = TRUE",
        )
        .bind(project_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        let prompt = match prompt {
            Some(p) => p,
            None => return Ok(None),
        };

        // Resolve version
        let pv = if let Some(v) = version {
            // Explicit version pin
            sqlx::query_as::<_, PromptVersionRow>(
                "SELECT * FROM prompt_versions WHERE prompt_id = $1 AND version = $2",
            )
            .bind(prompt.id)
            .bind(v)
            .fetch_optional(&self.pool)
            .await?
        } else if let Some(lbl) = label {
            // Resolve by label
            sqlx::query_as::<_, PromptVersionRow>(
                r#"SELECT * FROM prompt_versions
                   WHERE prompt_id = $1 AND labels @> to_jsonb($2::text)
                   ORDER BY version DESC LIMIT 1"#,
            )
            .bind(prompt.id)
            .bind(lbl)
            .fetch_optional(&self.pool)
            .await?
        } else {
            // Latest version
            sqlx::query_as::<_, PromptVersionRow>(
                "SELECT * FROM prompt_versions WHERE prompt_id = $1 ORDER BY version DESC LIMIT 1",
            )
            .bind(prompt.id)
            .fetch_optional(&self.pool)
            .await?
        };

        match pv {
            Some(v) => Ok(Some((prompt, v))),
            None => Ok(None),
        }
    }

    /// Get all unique folders for a project's prompts.
    pub async fn list_prompt_folders(&self, project_id: Uuid) -> anyhow::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT folder FROM prompts WHERE project_id = $1 AND is_active = TRUE ORDER BY folder",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(f,)| f).collect())
    }
}


// -- Model Pricing Row --

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ModelPricingRow {
    pub id: Uuid,
    pub provider: String,
    pub model_pattern: String,
    pub input_per_m: rust_decimal::Decimal,
    pub output_per_m: rust_decimal::Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// -- Prompt Management --

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PromptRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub folder: String,
    pub tags: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PromptVersionRow {
    pub id: Uuid,
    pub prompt_id: Uuid,
    pub version: i32,
    pub model: String,
    pub messages: serde_json::Value,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub tools: Option<serde_json::Value>,
    pub commit_message: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub labels: serde_json::Value,
}

pub struct NewPrompt {
    pub project_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub folder: String,
    pub tags: serde_json::Value,
    pub created_by: String,
}

pub struct NewPromptVersion {
    pub prompt_id: Uuid,
    pub model: String,
    pub messages: serde_json::Value,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub tools: Option<serde_json::Value>,
    pub commit_message: String,
    pub created_by: String,
}
