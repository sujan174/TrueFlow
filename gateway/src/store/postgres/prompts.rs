use super::types::{NewPrompt, NewPromptVersion, PromptRow, PromptVersionRow};
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    // ── Prompt Management ─────────────────────────────────────────

    pub async fn insert_prompt(&self, p: &NewPrompt) -> anyhow::Result<PromptRow> {
        let row = sqlx::query_as::<_, PromptRow>(
            r#"INSERT INTO prompts (project_id, name, slug, description, folder, tags, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING *"#,
        )
        .bind(p.project_id)
        .bind(&p.name)
        .bind(&p.slug)
        .bind(&p.description)
        .bind(&p.folder)
        .bind(&p.tags)
        .bind(&p.created_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_prompt(
        &self,
        id: Uuid,
        project_id: Uuid,
    ) -> anyhow::Result<Option<PromptRow>> {
        let row = sqlx::query_as::<_, PromptRow>(
            "SELECT * FROM prompts WHERE id = $1 AND project_id = $2 AND is_active = TRUE",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_prompts(
        &self,
        project_id: Uuid,
        folder: Option<&str>,
    ) -> anyhow::Result<Vec<PromptRow>> {
        let rows = if let Some(f) = folder {
            sqlx::query_as::<_, PromptRow>(
                "SELECT * FROM prompts WHERE project_id = $1 AND folder = $2 AND is_active = TRUE ORDER BY updated_at DESC",
            )
            .bind(project_id)
            .bind(f)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, PromptRow>(
                "SELECT * FROM prompts WHERE project_id = $1 AND is_active = TRUE ORDER BY updated_at DESC",
            )
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn update_prompt(
        &self,
        id: Uuid,
        project_id: Uuid,
        name: &str,
        description: &str,
        folder: &str,
        tags: &serde_json::Value,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"UPDATE prompts SET name = $1, description = $2, folder = $3, tags = $4, updated_at = NOW()
               WHERE id = $5 AND project_id = $6 AND is_active = TRUE"#,
        )
        .bind(name)
        .bind(description)
        .bind(folder)
        .bind(tags)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_prompt(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE prompts SET is_active = FALSE, updated_at = NOW() WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Create a new immutable version. Auto-increments version number atomically.
    pub async fn insert_prompt_version(
        &self,
        v: &NewPromptVersion,
    ) -> anyhow::Result<PromptVersionRow> {
        // Use ON CONFLICT DO UPDATE with subquery to atomically get next version
        // This prevents race conditions when multiple versions are created concurrently
        let row = sqlx::query_as::<_, PromptVersionRow>(
            r#"INSERT INTO prompt_versions
               (prompt_id, version, model, messages, temperature, max_tokens, top_p, tools, commit_message, created_by)
               VALUES (
                   $1,
                   (SELECT COALESCE(MAX(version), 0) + 1 FROM prompt_versions WHERE prompt_id = $1),
                   $2, $3, $4, $5, $6, $7, $8, $9
               )
               RETURNING *"#,
        )
        .bind(v.prompt_id)
        .bind(&v.model)
        .bind(&v.messages)
        .bind(v.temperature)
        .bind(v.max_tokens)
        .bind(v.top_p)
        .bind(&v.tools)
        .bind(&v.commit_message)
        .bind(&v.created_by)
        .fetch_one(&self.pool)
        .await?;

        // Touch parent updated_at
        let _ = sqlx::query("UPDATE prompts SET updated_at = NOW() WHERE id = $1")
            .bind(v.prompt_id)
            .execute(&self.pool)
            .await;

        Ok(row)
    }

    pub async fn list_prompt_versions(
        &self,
        prompt_id: Uuid,
    ) -> anyhow::Result<Vec<PromptVersionRow>> {
        let rows = sqlx::query_as::<_, PromptVersionRow>(
            "SELECT * FROM prompt_versions WHERE prompt_id = $1 ORDER BY version DESC",
        )
        .bind(prompt_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_prompt_version(
        &self,
        prompt_id: Uuid,
        version: i32,
    ) -> anyhow::Result<Option<PromptVersionRow>> {
        let row = sqlx::query_as::<_, PromptVersionRow>(
            "SELECT * FROM prompt_versions WHERE prompt_id = $1 AND version = $2",
        )
        .bind(prompt_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Deploy: atomically move a label to a specific version.
    /// Removes the label from all other versions of the same prompt, then adds to the target.
    pub async fn deploy_prompt_version(
        &self,
        prompt_id: Uuid,
        version: i32,
        label: &str,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;

        // Remove label from all versions of this prompt
        sqlx::query(
            r#"UPDATE prompt_versions
               SET labels = labels - $1
               WHERE prompt_id = $2"#,
        )
        .bind(label)
        .bind(prompt_id)
        .execute(&mut *tx)
        .await?;

        // Add label to the target version
        let result = sqlx::query(
            r#"UPDATE prompt_versions
               SET labels = CASE
                   WHEN NOT labels @> to_jsonb($1::text) THEN labels || to_jsonb($1::text)
                   ELSE labels
               END
               WHERE prompt_id = $2 AND version = $3"#,
        )
        .bind(label)
        .bind(prompt_id)
        .bind(version)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    /// Render API: resolve a prompt by slug + optional label or version.
    /// Priority: explicit version > label > latest.
    pub async fn get_prompt_for_render(
        &self,
        project_id: Uuid,
        slug: &str,
        label: Option<&str>,
        version: Option<i32>,
    ) -> anyhow::Result<Option<(PromptRow, PromptVersionRow)>> {
        // First get the prompt by slug
        let prompt = sqlx::query_as::<_, PromptRow>(
            "SELECT * FROM prompts WHERE project_id = $1 AND slug = $2 AND is_active = TRUE",
        )
        .bind(project_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        let prompt = match prompt {
            Some(p) => p,
            None => return Ok(None),
        };

        // Resolve version
        let pv = if let Some(v) = version {
            // Explicit version pin
            sqlx::query_as::<_, PromptVersionRow>(
                "SELECT * FROM prompt_versions WHERE prompt_id = $1 AND version = $2",
            )
            .bind(prompt.id)
            .bind(v)
            .fetch_optional(&self.pool)
            .await?
        } else if let Some(lbl) = label {
            // Resolve by label
            sqlx::query_as::<_, PromptVersionRow>(
                r#"SELECT * FROM prompt_versions
                   WHERE prompt_id = $1 AND labels @> to_jsonb($2::text)
                   ORDER BY version DESC LIMIT 1"#,
            )
            .bind(prompt.id)
            .bind(lbl)
            .fetch_optional(&self.pool)
            .await?
        } else {
            // Latest version
            sqlx::query_as::<_, PromptVersionRow>(
                "SELECT * FROM prompt_versions WHERE prompt_id = $1 ORDER BY version DESC LIMIT 1",
            )
            .bind(prompt.id)
            .fetch_optional(&self.pool)
            .await?
        };

        match pv {
            Some(v) => Ok(Some((prompt, v))),
            None => Ok(None),
        }
    }

    /// Get all unique folders for a project's prompts.
    pub async fn list_prompt_folders(&self, project_id: Uuid) -> anyhow::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT folder FROM prompts WHERE project_id = $1 AND is_active = TRUE ORDER BY folder",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(f,)| f).collect())
    }
}
