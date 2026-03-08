use uuid::Uuid;
use super::PgStore;

impl PgStore {
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
}
