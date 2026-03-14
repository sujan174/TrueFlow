use super::types::{NewToken, TokenRow};
use super::PgStore;
use uuid::Uuid;

impl PgStore {
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
            "SELECT id, project_id, name, credential_id, upstream_url, scopes, policy_ids, is_active, expires_at, created_at, COALESCE(log_level, 1::SMALLINT) as log_level, upstreams, circuit_breaker, allowed_models, allowed_model_group_ids, team_id, tags, mcp_allowed_tools, mcp_blocked_tools, guardrail_header_mode FROM tokens WHERE id = $1"
        )
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_tokens(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<TokenRow>> {
        let limit = limit.clamp(1, 1000); // Cap at 1000, minimum 1
        let rows = sqlx::query_as::<_, TokenRow>(
            "SELECT id, project_id, name, credential_id, upstream_url, scopes, policy_ids, is_active, expires_at, created_at, COALESCE(log_level, 1::SMALLINT) as log_level, upstreams, circuit_breaker, allowed_models, allowed_model_group_ids, team_id, tags, mcp_allowed_tools, mcp_blocked_tools, guardrail_header_mode FROM tokens WHERE project_id = $1 AND is_active = true ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn revoke_token(&self, token_id: &str, project_id: Uuid) -> anyhow::Result<bool> {
        let result =
            sqlx::query("UPDATE tokens SET is_active = false, updated_at = NOW() WHERE id = $1 AND project_id = $2")
                .bind(token_id)
                .bind(project_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update the circuit breaker configuration for a token.
    /// Returns `true` if the token was found and updated, `false` if not found.
    pub async fn update_circuit_breaker(
        &self,
        token_id: &str,
        project_id: Uuid,
        config: serde_json::Value,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET circuit_breaker = $1 WHERE id = $2 AND project_id = $3 AND is_active = true"
        )
        .bind(&config)
        .bind(token_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Replace the `policy_ids` array on a token.
    /// Used by the guardrail presets API to attach auto-generated policies.
    pub async fn set_token_policy_ids(
        &self,
        token_id: &str,
        project_id: Uuid,
        policy_ids: &[Uuid],
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET policy_ids = $1 WHERE id = $2 AND project_id = $3 AND is_active = true"
        )
        .bind(policy_ids)
        .bind(token_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update a token's upstream URL, policy bindings, and log level.
    /// Used by config import to update an existing token without touching its credentials.
    pub async fn update_token_config(
        &self,
        token_id: &str,
        project_id: Uuid,
        policy_ids: Vec<Uuid>,
        log_level: i16,
        upstream_url: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET policy_ids = $1, log_level = $2, upstream_url = $3, updated_at = NOW() WHERE id = $4 AND project_id = $5 AND is_active = true"
        )
        .bind(&policy_ids)
        .bind(log_level)
        .bind(upstream_url)
        .bind(token_id)
        .bind(project_id)
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
            credential_id: None, // no credential — passthrough mode
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
}
