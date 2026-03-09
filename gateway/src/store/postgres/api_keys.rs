use super::types::ApiKeyRow;
use super::PgStore;
use uuid::Uuid;

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
        let key = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT * FROM api_keys WHERE key_hash = $1 AND is_active = true",
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn list_api_keys(&self, org_id: Uuid) -> anyhow::Result<Vec<ApiKeyRow>> {
        let keys = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT * FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
        )
        .bind(org_id)
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
        sqlx::query!("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
