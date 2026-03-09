use super::types::SessionEntity;
use super::PgStore;
use uuid::Uuid;

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
    /// Valid transitions:
    ///   - "active" → "paused", "completed", "expired"
    ///   - "paused" → "active", "expired"
    ///   - Any other transition returns None (invalid)
    pub async fn update_session_status(
        &self,
        session_id: &str,
        project_id: Uuid,
        new_status: &str,
    ) -> anyhow::Result<Option<SessionEntity>> {
        // Get current status first
        let current = sqlx::query_scalar::<_, Option<String>>(
            "SELECT status FROM sessions WHERE session_id = $1 AND project_id = $2",
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        let current_status = match current {
            Some(Some(s)) => s,
            Some(None) => return Ok(None), // Session not found
            None => return Ok(None),       // Query failed
        };

        // Validate transition
        let valid = matches!(
            (current_status.as_str(), new_status),
            ("active", "paused")
                | ("active", "completed")
                | ("active", "expired")
                | ("paused", "active")
                | ("paused", "expired")
        );

        if !valid {
            tracing::warn!(
                session_id = %session_id,
                current_status = %current_status,
                new_status = %new_status,
                "Invalid session state transition"
            );
            return Ok(None);
        }

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
