use super::types::NewService;
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    pub async fn create_service(
        &self,
        svc: &NewService,
    ) -> anyhow::Result<crate::models::service::Service> {
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

    pub async fn list_services(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<crate::models::service::Service>> {
        let limit = limit.clamp(1, 1000); // Cap at 1000, minimum 1
        let rows = sqlx::query_as::<_, crate::models::service::Service>(
            "SELECT id, project_id, name, description, base_url, service_type, credential_id, is_active, created_at, updated_at FROM services WHERE project_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_service_by_name(
        &self,
        project_id: Uuid,
        name: &str,
    ) -> anyhow::Result<Option<crate::models::service::Service>> {
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
        let result = sqlx::query("DELETE FROM services WHERE id = $1 AND project_id = $2")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
