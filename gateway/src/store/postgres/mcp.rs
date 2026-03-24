//! MCP Server Persistence Layer
//!
//! Provides database storage for MCP server configurations and tool schemas.
//! Servers are persisted to survive gateway restarts and are loaded into the
//! in-memory registry on startup.

use super::types::{McpServerRow, McpServerToolRow, NewMcpServer};
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    /// Insert a new MCP server configuration.
    pub async fn insert_mcp_server(&self, server: &NewMcpServer) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO mcp_servers (
                id, project_id, name, endpoint, auth_type,
                api_key_encrypted, oauth_client_id, oauth_client_secret_enc,
                oauth_token_endpoint, oauth_scopes, oauth_access_token_enc,
                oauth_refresh_token_enc, oauth_token_expires_at,
                status, tool_count, discovered_server_info
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#
        )
        .bind(&server.id)
        .bind(server.project_id)
        .bind(&server.name)
        .bind(&server.endpoint)
        .bind(&server.auth_type)
        .bind(&server.api_key_encrypted)
        .bind(&server.oauth_client_id)
        .bind(&server.oauth_client_secret_enc)
        .bind(&server.oauth_token_endpoint)
        .bind(&server.oauth_scopes)
        .bind(&server.oauth_access_token_enc)
        .bind(&server.oauth_refresh_token_enc)
        .bind(server.oauth_token_expires_at)
        .bind(&server.status)
        .bind(server.tool_count)
        .bind(&server.discovered_server_info)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all MCP servers for a project.
    pub async fn list_mcp_servers(&self, project_id: Uuid) -> anyhow::Result<Vec<McpServerRow>> {
        let rows = sqlx::query_as::<_, McpServerRow>(
            "SELECT * FROM mcp_servers WHERE project_id = $1 ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// List all MCP servers across all projects (for startup restoration).
    pub async fn list_all_mcp_servers(&self) -> anyhow::Result<Vec<McpServerRow>> {
        let rows = sqlx::query_as::<_, McpServerRow>(
            "SELECT * FROM mcp_servers ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get a specific MCP server by ID.
    pub async fn get_mcp_server(&self, id: Uuid) -> anyhow::Result<Option<McpServerRow>> {
        let row = sqlx::query_as::<_, McpServerRow>(
            "SELECT * FROM mcp_servers WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Update MCP server status and tool count.
    pub async fn update_mcp_server_status(
        &self,
        id: Uuid,
        status: &str,
        tool_count: i32,
        last_error: Option<&str>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE mcp_servers
            SET status = $2, tool_count = $3, last_error = $4, updated_at = NOW()
            WHERE id = $1
            "#
        )
        .bind(id)
        .bind(status)
        .bind(tool_count)
        .bind(last_error)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update OAuth tokens for an MCP server.
    pub async fn update_mcp_oauth_tokens(
        &self,
        id: Uuid,
        access_token_enc: Option<&str>,
        refresh_token_enc: Option<&str>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE mcp_servers
            SET oauth_access_token_enc = $2,
                oauth_refresh_token_enc = $3,
                oauth_token_expires_at = $4,
                updated_at = NOW()
            WHERE id = $1
            "#
        )
        .bind(id)
        .bind(access_token_enc)
        .bind(refresh_token_enc)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete an MCP server and its cached tools.
    pub async fn delete_mcp_server(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Cache tool schemas for an MCP server (replaces existing tools).
    pub async fn cache_mcp_tools(
        &self,
        server_id: Uuid,
        tools: &[McpToolToCache],
    ) -> anyhow::Result<()> {
        // Delete existing tools for this server
        sqlx::query("DELETE FROM mcp_server_tools WHERE server_id = $1")
            .bind(server_id)
            .execute(&self.pool)
            .await?;

        // Insert new tools
        for tool in tools {
            sqlx::query(
                r#"
                INSERT INTO mcp_server_tools (id, server_id, name, description, input_schema, output_schema)
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (server_id, name) DO UPDATE SET
                    description = EXCLUDED.description,
                    input_schema = EXCLUDED.input_schema,
                    output_schema = EXCLUDED.output_schema
                "#
            )
            .bind(Uuid::new_v4())
            .bind(server_id)
            .bind(&tool.name)
            .bind(&tool.description)
            .bind(&tool.input_schema)
            .bind(&tool.output_schema)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get all cached tools for an MCP server.
    pub async fn get_mcp_server_tools(&self, server_id: Uuid) -> anyhow::Result<Vec<McpServerToolRow>> {
        let rows = sqlx::query_as::<_, McpServerToolRow>(
            "SELECT * FROM mcp_server_tools WHERE server_id = $1 ORDER BY name"
        )
        .bind(server_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

/// Tool schema to cache in the database.
#[derive(Debug, Clone)]
pub struct McpToolToCache {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
}