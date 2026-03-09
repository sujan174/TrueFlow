use super::PgStore;
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl PgStore {
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

    pub async fn get_approval_status(
        &self,
        request_id: Uuid,
        project_id: Uuid,
    ) -> anyhow::Result<String> {
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM approval_requests WHERE id = $1 AND project_id = $2",
        )
        .bind(request_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(status.unwrap_or_else(|| "expired".to_string()))
    }

    pub async fn list_pending_approvals(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<crate::models::approval::ApprovalRequest>> {
        let limit = limit.clamp(1, 1000); // Cap at 1000, minimum 1
        let rows = sqlx::query_as::<_, crate::models::approval::ApprovalRequest>(
            "SELECT * FROM approval_requests WHERE project_id = $1 AND status = 'pending' ORDER BY created_at ASC LIMIT $2 OFFSET $3"
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count pending approval requests for a specific token.
    /// Used to enforce HITL concurrency cap before creating new approvals.
    pub async fn count_pending_approvals_for_token(
        &self,
        token_id: &str,
        project_id: Uuid,
    ) -> anyhow::Result<i64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM approval_requests WHERE token_id = $1 AND project_id = $2 AND status = 'pending'"
        )
        .bind(token_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
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
        decision: &str, // "approved" | "rejected"
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
}
