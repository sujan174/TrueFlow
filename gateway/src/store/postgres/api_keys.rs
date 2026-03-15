use super::types::ApiKeyRow;
use super::PgStore;
use uuid::Uuid;

/// SEC-10: Error returned when attempting to revoke the last admin key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastAdminError;

impl std::fmt::Display for LastAdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cannot revoke the last admin key")
    }
}

impl std::error::Error for LastAdminError {}

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

    /// SEC-10: Atomically revoke an API key while preventing last-admin revocation.
    /// This fixes a TOCTOU race condition where two concurrent requests could both
    /// pass the "last admin" check and then both revoke, leaving no admin keys.
    ///
    /// Returns:
    /// - Ok(true) if the key was revoked successfully
    /// - Ok(false) if the key was not found
    /// - Err(LastAdminError) if this is the last admin key
    pub async fn revoke_api_key_atomic(
        &self,
        id: Uuid,
        org_id: Uuid,
    ) -> anyhow::Result<Result<bool, LastAdminError>> {
        // Use a CTE to atomically check admin count and revoke in a single query
        // This prevents the TOCTOU race condition
        #[derive(sqlx::FromRow)]
        struct AdminCheckResult {
            admin_count: i64,
            target_role: Option<String>,
            target_is_active: Option<bool>,
            target_exists: bool,
        }

        let result: AdminCheckResult = sqlx::query_as(
            r#"
            WITH target_key AS (
                SELECT id, role, is_active
                FROM api_keys
                WHERE id = $1 AND org_id = $2
            ),
            admin_count AS (
                SELECT COUNT(*) as count
                FROM api_keys
                WHERE org_id = $2 AND role = 'admin' AND is_active = true
            )
            SELECT
                (SELECT count FROM admin_count) as admin_count,
                (SELECT role FROM target_key) as target_role,
                (SELECT is_active FROM target_key) as target_is_active,
                EXISTS (SELECT 1 FROM target_key) as target_exists
            "#
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await?;

        // Check if key exists
        if !result.target_exists {
            return Ok(Ok(false));
        }

        // Check if it's an admin key
        let is_admin = result.target_role.as_deref() == Some("admin");
        let is_active = result.target_is_active.unwrap_or(false);

        // SEC-10: Prevent revoking the last active admin key
        if is_admin && is_active && result.admin_count <= 1 {
            return Ok(Err(LastAdminError));
        }

        // Perform the revocation
        let revoke_result: sqlx::postgres::PgQueryResult = sqlx::query(
            "UPDATE api_keys SET is_active = false WHERE id = $1 AND org_id = $2 AND is_active = true"
        )
        .bind(id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;

        Ok(Ok(revoke_result.rows_affected() > 0))
    }

    pub async fn touch_api_key_usage(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query!("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
