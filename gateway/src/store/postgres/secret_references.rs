//! Secret Reference persistence layer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{instrument, warn};
use uuid::Uuid;

use super::PgStore;
use crate::models::secret_reference::{
    CreateSecretReferenceRequest, SecretAccessResult, SecretReference, SecretReferenceFilter,
    UpdateSecretReferenceRequest,
};
use crate::vault::VaultBackend;

/// Database row representation for secret references.
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct SecretReferenceRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub vault_backend: String,
    pub external_ref: String,
    pub vault_config_id: Option<Uuid>,
    pub provider: Option<String>,
    pub injection_mode: String,
    pub injection_header: String,
    pub allowed_team_ids: Option<sqlx::types::Json<Vec<Uuid>>>,
    pub allowed_user_ids: Option<sqlx::types::Json<Vec<Uuid>>>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

impl From<SecretReferenceRow> for SecretReference {
    fn from(row: SecretReferenceRow) -> Self {
        let vault_backend = row.vault_backend.parse().unwrap_or_else(|e| {
            warn!(
                vault_backend = %row.vault_backend,
                error = %e,
                reference_id = %row.id,
                "Invalid vault_backend value, defaulting to 'builtin'"
            );
            VaultBackend::Builtin
        });
        Self {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            description: row.description,
            vault_backend,
            external_ref: row.external_ref,
            vault_config_id: row.vault_config_id,
            provider: row.provider,
            injection_mode: row.injection_mode,
            injection_header: row.injection_header,
            allowed_team_ids: row.allowed_team_ids.map(|j| j.0),
            allowed_user_ids: row.allowed_user_ids.map(|j| j.0),
            last_accessed_at: row.last_accessed_at,
            last_rotated_at: row.last_rotated_at,
            version: row.version,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
            created_by: row.created_by,
        }
    }
}

impl PgStore {
    /// Create a new secret reference.
    #[instrument(skip(self))]
    pub async fn create_secret_reference(
        &self,
        project_id: Uuid,
        req: CreateSecretReferenceRequest,
        created_by: Option<Uuid>,
    ) -> anyhow::Result<SecretReference> {
        let vault_backend: VaultBackend = req
            .vault_backend
            .parse()
            .map_err(|e: String| anyhow::anyhow!("Invalid vault backend: {}", e))?;
        let injection_mode = req.injection_mode.unwrap_or_else(|| "bearer".to_string());
        let injection_header = req
            .injection_header
            .unwrap_or_else(|| "Authorization".to_string());

        let row = sqlx::query_as::<_, SecretReferenceRow>(
            r#"INSERT INTO secret_references (
                project_id, name, description, vault_backend, external_ref,
                vault_config_id, provider, injection_mode, injection_header,
                allowed_team_ids, allowed_user_ids, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *"#,
        )
        .bind(project_id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(vault_backend.to_string())
        .bind(&req.external_ref)
        .bind(req.vault_config_id)
        .bind(&req.provider)
        .bind(&injection_mode)
        .bind(&injection_header)
        .bind(req.allowed_team_ids.map(sqlx::types::Json))
        .bind(req.allowed_user_ids.map(sqlx::types::Json))
        .bind(created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    /// List secret references with optional filtering.
    #[instrument(skip(self))]
    pub async fn list_secret_references(
        &self,
        project_id: Uuid,
        filter: SecretReferenceFilter,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<SecretReference>> {
        // Clamp limit to prevent DoS via excessive result sets
        let limit = limit.clamp(1, 1000);
        let mut query = String::from(
            "SELECT * FROM secret_references WHERE project_id = $1",
        );
        let mut param_count = 2;

        // Add filter conditions
        if filter.vault_backend.is_some() {
            query.push_str(&format!(" AND vault_backend = ${}", param_count));
            param_count += 1;
        }
        if filter.provider.is_some() {
            query.push_str(&format!(" AND provider = ${}", param_count));
            param_count += 1;
        }
        if filter.is_active.is_some() {
            query.push_str(&format!(" AND is_active = ${}", param_count));
            param_count += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_count, param_count + 1
        ));

        let mut q = sqlx::query_as::<_, SecretReferenceRow>(&query);

        q = q.bind(project_id);

        if let Some(ref vb) = filter.vault_backend {
            q = q.bind(vb);
        }
        if let Some(ref p) = filter.provider {
            q = q.bind(p);
        }
        if let Some(is_active) = filter.is_active {
            q = q.bind(is_active);
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get a single secret reference by ID.
    #[instrument(skip(self))]
    pub async fn get_secret_reference(
        &self,
        id: Uuid,
        project_id: Uuid,
    ) -> anyhow::Result<Option<SecretReference>> {
        let row = sqlx::query_as::<_, SecretReferenceRow>(
            "SELECT * FROM secret_references WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Update a secret reference.
    #[instrument(skip(self))]
    pub async fn update_secret_reference(
        &self,
        id: Uuid,
        project_id: Uuid,
        req: UpdateSecretReferenceRequest,
    ) -> anyhow::Result<Option<SecretReference>> {
        // Build dynamic UPDATE query
        let mut updates = Vec::new();
        let mut param_count = 3; // Start after id and project_id

        if req.name.is_some() {
            updates.push(format!("name = ${}", param_count));
            param_count += 1;
        }
        if req.description.is_some() {
            updates.push(format!("description = ${}", param_count));
            param_count += 1;
        }
        if req.external_ref.is_some() {
            updates.push(format!("external_ref = ${}", param_count));
            param_count += 1;
        }
        if req.vault_config_id.is_some() {
            updates.push(format!("vault_config_id = ${}", param_count));
            param_count += 1;
        }
        if req.provider.is_some() {
            updates.push(format!("provider = ${}", param_count));
            param_count += 1;
        }
        if req.injection_mode.is_some() {
            updates.push(format!("injection_mode = ${}", param_count));
            param_count += 1;
        }
        if req.injection_header.is_some() {
            updates.push(format!("injection_header = ${}", param_count));
            param_count += 1;
        }
        if req.allowed_team_ids.is_some() {
            updates.push(format!("allowed_team_ids = ${}", param_count));
            param_count += 1;
        }
        if req.allowed_user_ids.is_some() {
            updates.push(format!("allowed_user_ids = ${}", param_count));
            param_count += 1;
        }
        if req.version.is_some() {
            updates.push(format!("version = ${}", param_count));
            param_count += 1;
        }
        if req.is_active.is_some() {
            updates.push(format!("is_active = ${}", param_count));
        }

        if updates.is_empty() {
            // No updates, just return the current record
            return self.get_secret_reference(id, project_id).await;
        }

        updates.push("updated_at = NOW()".to_string());

        let query = format!(
            "UPDATE secret_references SET {} WHERE id = $1 AND project_id = $2 RETURNING *",
            updates.join(", ")
        );

        let mut q = sqlx::query_as::<_, SecretReferenceRow>(&query);

        q = q.bind(id).bind(project_id);

        if let Some(ref name) = req.name {
            q = q.bind(name);
        }
        if let Some(ref description) = req.description {
            q = q.bind(description);
        }
        if let Some(ref external_ref) = req.external_ref {
            q = q.bind(external_ref);
        }
        if let Some(vault_config_id) = req.vault_config_id {
            q = q.bind(vault_config_id);
        }
        if let Some(ref provider) = req.provider {
            q = q.bind(provider);
        }
        if let Some(ref injection_mode) = req.injection_mode {
            q = q.bind(injection_mode);
        }
        if let Some(ref injection_header) = req.injection_header {
            q = q.bind(injection_header);
        }
        if let Some(ref allowed_team_ids) = req.allowed_team_ids {
            q = q.bind(sqlx::types::Json(allowed_team_ids));
        }
        if let Some(ref allowed_user_ids) = req.allowed_user_ids {
            q = q.bind(sqlx::types::Json(allowed_user_ids));
        }
        if let Some(ref version) = req.version {
            q = q.bind(version);
        }
        if let Some(is_active) = req.is_active {
            q = q.bind(is_active);
        }

        let row = q.fetch_optional(&self.pool).await?;

        Ok(row.map(Into::into))
    }

    /// Soft-delete a secret reference by setting is_active = false.
    #[instrument(skip(self))]
    pub async fn delete_secret_reference(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE secret_references SET is_active = false, updated_at = NOW() \
             WHERE id = $1 AND project_id = $2 AND is_active = true",
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if a user has access to a secret reference.
    ///
    /// Returns Granted if access is allowed, Denied if not, NotFound if the reference doesn't exist.
    #[instrument(skip(self))]
    pub async fn check_secret_access(
        &self,
        secret_reference_id: Uuid,
        user_id: Uuid,
        team_id: Option<Uuid>,
    ) -> anyhow::Result<SecretAccessResult> {
        // Get the secret reference and check access lists
        let row = sqlx::query_as::<_, SecretReferenceRow>(
            "SELECT * FROM secret_references WHERE id = $1 AND is_active = true",
        )
        .bind(secret_reference_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(ref_row) = row else {
            return Ok(SecretAccessResult::NotFound);
        };

        // If no restrictions, access is granted
        if ref_row.allowed_user_ids.is_none() && ref_row.allowed_team_ids.is_none() {
            return Ok(SecretAccessResult::Granted);
        }

        // Check user allowlist
        if let Some(ref allowed_users) = ref_row.allowed_user_ids {
            if allowed_users.0.contains(&user_id) {
                return Ok(SecretAccessResult::Granted);
            }
        }

        // Check team allowlist
        if let Some(team_id) = team_id {
            if let Some(ref allowed_teams) = ref_row.allowed_team_ids {
                if allowed_teams.0.contains(&team_id) {
                    return Ok(SecretAccessResult::Granted);
                }
            }
        }

        Ok(SecretAccessResult::Denied)
    }

    /// Update last_accessed_at timestamp for a secret reference.
    #[instrument(skip(self))]
    pub async fn touch_secret_access(&self, secret_reference_id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE secret_references SET last_accessed_at = NOW() WHERE id = $1",
        )
        .bind(secret_reference_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get secret reference by name within a project.
    #[instrument(skip(self))]
    pub async fn get_secret_reference_by_name(
        &self,
        project_id: Uuid,
        name: &str,
    ) -> anyhow::Result<Option<SecretReference>> {
        let row = sqlx::query_as::<_, SecretReferenceRow>(
            "SELECT * FROM secret_references WHERE project_id = $1 AND name = $2 AND is_active = true",
        )
        .bind(project_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Hard delete a secret reference (admin only, use with caution).
    #[instrument(skip(self))]
    pub async fn hard_delete_secret_reference(&self, id: Uuid, project_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM secret_references WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}