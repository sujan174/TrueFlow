use uuid::Uuid;
use super::PgStore;
use super::types::{NewCredential, CredentialMeta};

impl PgStore {
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
}
