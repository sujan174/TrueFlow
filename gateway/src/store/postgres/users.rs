use super::types::{SyncUserRequest, SyncUserResponse, UserRow};
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    /// Sync a user from Supabase Auth to the gateway database.
    ///
    /// This is called by the dashboard after a successful Supabase login.
    /// It ensures the user exists in the gateway's `users` table and returns
    /// the user info for session establishment.
    ///
    /// Logic:
    /// 1. Look up user by `supabase_id` (primary) or `email` (fallback for migration)
    /// 2. If found: update `last_login_at` and return user info
    /// 3. If not found: create new user in default org with `member` role
    pub async fn sync_user_from_supabase(
        &self,
        request: SyncUserRequest,
    ) -> anyhow::Result<SyncUserResponse> {
        // Try to find existing user by supabase_id first
        let existing_by_supabase_id = self.get_user_by_supabase_id(request.supabase_id).await?;

        // If not found by supabase_id, try email (for users created before Supabase migration)
        let existing_user = if let Some(user) = existing_by_supabase_id {
            Some(user)
        } else {
            self.get_user_by_email(&request.email).await?
        };

        if let Some(user) = existing_user {
            // Update last_login_at and supabase_id if missing
            self.update_user_login(user.id, request.supabase_id, request.name.clone(), request.picture.clone())
                .await?;

            return Ok(SyncUserResponse {
                user_id: user.id,
                org_id: user.org_id,
                role: user.role,
                is_new_user: false,
                last_project_id: user.last_project_id,
            });
        }

        // Create a new organization for this user
        // Each user gets their own org for proper multi-tenancy
        let org_name = request.name.clone()
            .unwrap_or_else(|| request.email.split('@').next().unwrap_or("My Org").to_string());

        let org_id = self.create_organization_for_user(&org_name).await?;

        // Create a default project in the new org
        let project_name = "My First Project";
        let project_id = self.create_project_for_org(org_id, project_name).await?;

        // Create the user in their new org as admin (first user is always admin)
        let user = self
            .create_user_from_supabase_as_admin(
                org_id,
                request.supabase_id,
                &request.email,
                request.name.as_deref(),
                request.picture.as_deref(),
            )
            .await?;

        // Set this as their last project
        self.update_user_last_project(user.id, project_id).await?;

        Ok(SyncUserResponse {
            user_id: user.id,
            org_id: user.org_id,
            role: user.role,
            is_new_user: true,
            last_project_id: Some(project_id), // Pre-select their first project
        })
    }

    /// Get a user by their Supabase ID.
    pub async fn get_user_by_supabase_id(&self, supabase_id: Uuid) -> anyhow::Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id, org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id, created_at FROM users WHERE supabase_id = $1",
        )
        .bind(supabase_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get a user by their email address.
    pub async fn get_user_by_email(&self, email: &str) -> anyhow::Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id, org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id, created_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get a user by their gateway ID.
    pub async fn get_user_by_id(&self, id: Uuid) -> anyhow::Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id, org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id, created_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Update user's last login timestamp and optionally sync profile data.
    pub async fn update_user_login(
        &self,
        user_id: Uuid,
        supabase_id: Uuid,
        name: Option<String>,
        picture_url: Option<String>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET
                last_login_at = NOW(),
                supabase_id = COALESCE(supabase_id, $1),
                name = COALESCE(name, $2),
                picture_url = COALESCE(picture_url, $3)
            WHERE id = $4
            "#,
        )
        .bind(supabase_id)
        .bind(name)
        .bind(picture_url)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Create a new user from Supabase Auth.
    pub async fn create_user_from_supabase(
        &self,
        org_id: Uuid,
        supabase_id: Uuid,
        email: &str,
        name: Option<&str>,
        picture_url: Option<&str>,
    ) -> anyhow::Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id)
            VALUES ($1, $2, 'member', $3, $4, $5, NOW(), NULL)
            RETURNING id, org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id, created_at
            "#,
        )
        .bind(org_id)
        .bind(email)
        .bind(supabase_id)
        .bind(name)
        .bind(picture_url)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    /// List all users in an organization.
    pub async fn list_users_by_org(&self, org_id: Uuid) -> anyhow::Result<Vec<UserRow>> {
        let users = sqlx::query_as::<_, UserRow>(
            "SELECT id, org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id, created_at FROM users WHERE org_id = $1 ORDER BY created_at DESC",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Update a user's last used project.
    pub async fn update_user_last_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE users SET last_project_id = $1 WHERE id = $2",
        )
        .bind(project_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update a user's role.
    pub async fn update_user_role(&self, user_id: Uuid, role: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE users SET role = $1 WHERE id = $2"
        )
        .bind(role)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Create a new organization for a new user.
    pub async fn create_organization_for_user(&self, name: &str) -> anyhow::Result<Uuid> {
        let org_id: Uuid = sqlx::query_scalar(
            "INSERT INTO organizations (name, plan) VALUES ($1, 'free') RETURNING id"
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(org_id)
    }

    /// Create a default project for a new organization.
    pub async fn create_project_for_org(&self, org_id: Uuid, name: &str) -> anyhow::Result<Uuid> {
        let project_id: Uuid = sqlx::query_scalar(
            "INSERT INTO projects (org_id, name) VALUES ($1, $2) RETURNING id"
        )
        .bind(org_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(project_id)
    }

    /// Create a new user from Supabase Auth as an admin (first user in org).
    pub async fn create_user_from_supabase_as_admin(
        &self,
        org_id: Uuid,
        supabase_id: Uuid,
        email: &str,
        name: Option<&str>,
        picture_url: Option<&str>,
    ) -> anyhow::Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id)
            VALUES ($1, $2, 'admin', $3, $4, $5, NOW(), NULL)
            RETURNING id, org_id, email, role, supabase_id, name, picture_url, last_login_at, last_project_id, created_at
            "#,
        )
        .bind(org_id)
        .bind(email)
        .bind(supabase_id)
        .bind(name)
        .bind(picture_url)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }
}