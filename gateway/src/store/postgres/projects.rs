use super::types::ProjectRow;
use super::PgStore;
use uuid::Uuid;

impl PgStore {
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
        let result = sqlx::query("UPDATE projects SET name = $1 WHERE id = $2 AND org_id = $3")
            .bind(name)
            .bind(id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_project(&self, id: Uuid, org_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM projects WHERE id = $1 AND org_id = $2")
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
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND org_id = $2)",
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
        let r = sqlx::query("DELETE FROM audit_logs WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        total_deleted += r.rows_affected();

        // 2. Purge agent sessions
        let r = sqlx::query("DELETE FROM sessions WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        total_deleted += r.rows_affected();

        // 3. Purge virtual key usage / billing records (keep keys themselves; owners may need invoicing data export first)
        let r = sqlx::query("DELETE FROM token_usage WHERE project_id = $1")
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
    pub async fn project_belongs_to_org(
        &self,
        project_id: Uuid,
        org_id: Uuid,
    ) -> anyhow::Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND org_id = $2)",
        )
        .bind(project_id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }
}
