use super::types::{PolicyRow, PolicyVersionRow};
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    pub async fn get_policies_for_token(
        &self,
        project_id: Uuid,
        policy_ids: &[Uuid],
    ) -> anyhow::Result<Vec<crate::models::policy::Policy>> {
        if policy_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as::<_, PolicyRow>(
            "SELECT id, project_id, name, mode, phase, rules, retry, is_active, created_at FROM policies WHERE id = ANY($1) AND project_id = $2 AND is_active = true"
        )
        .bind(policy_ids)
        .bind(project_id)
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
                        tracing::error!(
                            "Failed to deserialize retry config for policy {}: {}",
                            row.id,
                            e
                        );
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

    pub async fn list_policies(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PolicyRow>> {
        let limit = limit.clamp(1, 1000); // Cap at 1000, minimum 1
        let rows = sqlx::query_as::<_, PolicyRow>(
            "SELECT id, project_id, name, mode, phase, rules, retry, is_active, created_at FROM policies WHERE project_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
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
        expected_version: Option<i32>,
    ) -> anyhow::Result<Result<bool, ()>> {
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

        // Build dynamic update with optional optimistic locking
        // If expected_version is provided, only update if the current version matches
        let result = if let Some(ver) = expected_version {
            sqlx::query(
                r#"UPDATE policies
                   SET mode = COALESCE($1, mode),
                       phase = COALESCE($2, phase),
                       rules = COALESCE($3, rules),
                       retry = COALESCE($4, retry),
                       name = COALESCE($5, name),
                       version = version + 1
                   WHERE id = $6 AND project_id = $7 AND is_active = true AND version = $8"#,
            )
            .bind(mode)
            .bind(phase)
            .bind(rules)
            .bind(retry)
            .bind(name)
            .bind(id)
            .bind(project_id)
            .bind(ver)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query(
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
            .await?
        };

        if result.rows_affected() == 0 && expected_version.is_some() {
            // Version mismatch - concurrent modification detected
            return Ok(Err(()));
        }

        Ok(Ok(result.rows_affected() > 0))
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
}
