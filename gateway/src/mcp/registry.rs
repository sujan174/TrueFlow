//! MCP Server Registry — in-memory registry of active MCP connections
//! with cached tool schemas and OAuth 2.0 auto-discovery.
//!
//! The registry manages MCP server lifecycles:
//! - Registration (connect + initialize + cache tools)
//! - Auto-discovery (probe URL → OAuth discovery → token → connect)
//! - Tool schema caching and refresh
//! - Tool execution routing

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::client::{McpAuth, McpClient};
use super::oauth::OAuthTokenManager;
use super::types::*;

/// Configuration for registering an MCP server.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub api_key: Option<String>,
}

/// Request for auto-discovery registration.
#[derive(Debug, Clone)]
pub struct DiscoverRequest {
    pub endpoint: String,
    pub name: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub project_id: Uuid,
}

/// Result of a discovery probe (dry-run).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveryResult {
    pub endpoint: String,
    pub requires_auth: bool,
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes_supported: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<Implementation>,
    pub tools: Vec<McpToolDef>,
    pub tool_count: usize,
}

/// Runtime state for a connected MCP server.
pub struct McpServerState {
    pub config: McpServerConfig,
    pub client: McpClient,
    pub tools: Vec<McpToolDef>,
    pub last_refreshed: std::time::Instant,
    pub status: McpServerStatus,
    pub server_info: Option<Implementation>,
    pub auth_type: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum McpServerStatus {
    Connected,
    Disconnected,
    Error(String),
}

/// In-memory registry of active MCP server connections.
pub struct McpRegistry {
    servers: Arc<RwLock<HashMap<Uuid, McpServerState>>>,
    /// Name → ID index for fast lookup by server name.
    name_index: Arc<RwLock<HashMap<String, Uuid>>>,
    /// Shared OAuth token manager for all OAuth-authed servers.
    oauth_manager: Arc<OAuthTokenManager>,
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            oauth_manager: Arc::new(OAuthTokenManager::new()),
        }
    }

    /// Get a reference to the shared OAuth token manager.
    pub fn oauth_manager(&self) -> &Arc<OAuthTokenManager> {
        &self.oauth_manager
    }

    /// Register and connect to an MCP server (manual mode with optional API key).
    ///
    /// Performs the initialize handshake and caches the tool list.
    pub async fn register(&self, config: McpServerConfig) -> Result<Vec<McpToolDef>, String> {
        let client = McpClient::new(&config.endpoint, config.api_key.clone());
        let auth_type = if config.api_key.is_some() {
            "bearer"
        } else {
            "none"
        };

        self.register_with_client(config, client, auth_type.to_string())
            .await
    }

    /// Register via auto-discovery: probe → OAuth discovery → token → initialize → list tools.
    pub async fn register_with_discovery(
        &self,
        req: DiscoverRequest,
    ) -> Result<(Uuid, Vec<McpToolDef>), String> {
        let id = Uuid::new_v4();

        // Step 1: Attempt OAuth discovery
        let discovery = self.oauth_manager.discover(&req.endpoint).await;

        let (client, auth_type, name) = match discovery {
            Ok(disc) if disc.requires_auth => {
                // OAuth is required
                let client_id = req.client_id.ok_or_else(|| {
                    "Server requires OAuth 2.0 authentication. Please provide client_id and client_secret.".to_string()
                })?;
                let client_secret = req.client_secret.ok_or_else(|| {
                    "Server requires OAuth 2.0 authentication. Please provide client_secret."
                        .to_string()
                })?;

                // Acquire initial token
                let scopes = disc.auth_server.scopes_supported.clone();

                let token_resp = self
                    .oauth_manager
                    .acquire_token(
                        &disc.auth_server.token_endpoint,
                        &client_id,
                        &client_secret,
                        &scopes,
                    )
                    .await?;

                // Store in token cache
                self.oauth_manager
                    .store_token(
                        id,
                        &token_resp,
                        disc.auth_server.token_endpoint.clone(),
                        client_id,
                        client_secret,
                        scopes,
                    )
                    .await;

                let auth = McpAuth::OAuth {
                    manager: self.oauth_manager.clone(),
                    server_id: id,
                };

                let client = McpClient::with_auth(&req.endpoint, auth);
                (client, "oauth2".to_string(), req.name)
            }
            Ok(_) => {
                // No auth required
                let client = McpClient::new(&req.endpoint, None::<String>);
                (client, "none".to_string(), req.name)
            }
            Err(_) => {
                // Discovery failed — try connecting without auth
                // (server might not implement RFC 9728 but still work)
                let client = McpClient::new(&req.endpoint, None::<String>);
                (client, "none".to_string(), req.name)
            }
        };

        // Step 2: Initialize + list tools
        let init_result = client
            .initialize()
            .await
            .map_err(|e| format!("Failed to initialize MCP server: {}", e))?;

        if init_result.capabilities.tools.is_none() {
            return Err("MCP server does not advertise tools capability".to_string());
        }

        let tools = client
            .list_tools()
            .await
            .map_err(|e| format!("Failed to list tools: {}", e))?;

        // Step 3: Derive name from server info if not provided
        let server_name = name
            .or_else(|| {
                init_result
                    .server_info
                    .as_ref()
                    .map(|info| sanitize_server_name(&info.name))
            })
            .unwrap_or_else(|| format!("mcp-server-{}", &id.to_string()[..8]));

        tracing::info!(
            server = %server_name,
            tool_count = tools.len(),
            auth_type = %auth_type,
            tools = ?tools.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "MCP server auto-discovered and registered"
        );

        let config = McpServerConfig {
            id,
            project_id: req.project_id,
            name: server_name.clone(),
            endpoint: req.endpoint,
            api_key: None, // OAuth-managed servers don't use static keys
        };

        let tools_clone = tools.clone();
        let state = McpServerState {
            config,
            client,
            tools,
            last_refreshed: std::time::Instant::now(),
            status: McpServerStatus::Connected,
            server_info: init_result.server_info,
            auth_type,
        };

        {
            let mut servers = self.servers.write().await;
            servers.insert(id, state);
        }
        {
            let mut index = self.name_index.write().await;
            index.insert(server_name, id);
        }

        Ok((id, tools_clone))
    }

    /// Dry-run discovery: probe a URL, return auth requirements + server info without persisting.
    ///
    /// Returns `Err` if the server cannot be reached at all (not a network endpoint).
    /// If the server is OAuth-protected and no credentials are supplied, returns a partial
    /// DiscoveryResult with `requires_auth: true` and empty tool list instead of failing.
    pub async fn discover_dry_run(&self, endpoint: &str) -> Result<DiscoveryResult, String> {
        // Try OAuth discovery
        let discovery = self.oauth_manager.discover(endpoint).await;

        let (auth_type, token_endpoint, scopes) = match &discovery {
            Ok(disc) if disc.requires_auth => (
                "oauth2".to_string(),
                Some(disc.auth_server.token_endpoint.clone()),
                Some(disc.auth_server.scopes_supported.clone()),
            ),
            Ok(_) => ("none".to_string(), None, None),
            Err(_) => ("unknown".to_string(), None, None),
        };

        // If the server requires OAuth and we have no credentials, return a partial result.
        // Don't fail — the caller just can't see server_info/tools without authing first.
        if auth_type == "oauth2" {
            return Ok(DiscoveryResult {
                endpoint: endpoint.to_string(),
                requires_auth: true,
                auth_type,
                token_endpoint,
                scopes_supported: scopes,
                server_info: None,
                tools: vec![],
                tool_count: 0,
            });
        }

        // Try connecting without auth to get server info and tools.
        // If initialization fails, the endpoint is not an MCP server — propagate the error.
        let client = McpClient::new(endpoint, None::<String>);
        let init = client
            .initialize()
            .await
            .map_err(|e| format!("Endpoint is not an MCP-compatible server: {}", e))?;

        let tools = client.list_tools().await.unwrap_or_default();
        let tool_count = tools.len();

        Ok(DiscoveryResult {
            endpoint: endpoint.to_string(),
            requires_auth: false,
            auth_type,
            token_endpoint,
            scopes_supported: scopes,
            server_info: init.server_info,
            tools,
            tool_count,
        })
    }

    /// Internal: register a server with an already-constructed client.
    async fn register_with_client(
        &self,
        config: McpServerConfig,
        client: McpClient,
        auth_type: String,
    ) -> Result<Vec<McpToolDef>, String> {
        // Initialize
        let init_result = client
            .initialize()
            .await
            .map_err(|e| format!("Failed to initialize MCP server '{}': {}", config.name, e))?;

        // Verify server supports tools
        if init_result.capabilities.tools.is_none() {
            return Err(format!(
                "MCP server '{}' does not advertise tools capability",
                config.name
            ));
        }

        // Fetch tools
        let tools = client
            .list_tools()
            .await
            .map_err(|e| format!("Failed to list tools from '{}': {}", config.name, e))?;

        tracing::info!(
            server = %config.name,
            tool_count = tools.len(),
            auth_type = %auth_type,
            tools = ?tools.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "MCP server registered"
        );

        let id = config.id;
        let name = config.name.clone();
        let tools_clone = tools.clone();

        let state = McpServerState {
            config,
            client,
            tools,
            last_refreshed: std::time::Instant::now(),
            status: McpServerStatus::Connected,
            server_info: init_result.server_info,
            auth_type,
        };

        {
            let mut servers = self.servers.write().await;
            servers.insert(id, state);
        }
        {
            let mut index = self.name_index.write().await;
            index.insert(name, id);
        }

        Ok(tools_clone)
    }

    /// Remove an MCP server from the registry.
    pub async fn unregister(&self, id: &Uuid) -> bool {
        let mut servers = self.servers.write().await;
        if let Some(state) = servers.remove(id) {
            let mut index = self.name_index.write().await;
            index.remove(&state.config.name);
            // Clean up OAuth tokens
            self.oauth_manager.remove_token(id).await;
            true
        } else {
            false
        }
    }

    /// Refresh the tool cache for a specific server.
    pub async fn refresh(&self, id: &Uuid) -> Result<Vec<McpToolDef>, String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(id)
            .ok_or_else(|| format!("MCP server {} not found", id))?;

        match state.client.list_tools().await {
            Ok(tools) => {
                tracing::info!(
                    server = %state.config.name,
                    tool_count = tools.len(),
                    "MCP tools refreshed"
                );
                state.tools = tools.clone();
                state.last_refreshed = std::time::Instant::now();
                state.status = McpServerStatus::Connected;
                Ok(tools)
            }
            Err(e) => {
                state.status = McpServerStatus::Error(e.clone());
                Err(e)
            }
        }
    }

    /// Get merged OpenAI-format tool definitions for a set of server IDs.
    pub async fn get_openai_tools(&self, server_ids: &[Uuid]) -> Vec<Value> {
        let servers = self.servers.read().await;
        let mut tools = Vec::new();

        for id in server_ids {
            if let Some(state) = servers.get(id) {
                if state.status != McpServerStatus::Connected {
                    continue;
                }
                for tool in &state.tools {
                    tools.push(to_openai_function(&state.config.name, tool));
                }
            }
        }

        tools
    }

    /// Get merged OpenAI-format tool definitions by server names.
    pub async fn get_openai_tools_by_name(&self, server_names: &[String]) -> Vec<Value> {
        let index = self.name_index.read().await;
        let ids: Vec<Uuid> = server_names
            .iter()
            .filter_map(|name| index.get(name).copied())
            .collect();
        self.get_openai_tools(&ids).await
    }

    /// Execute a tool call routed by server name.
    ///
    /// # Security
    /// The caller MUST provide the `project_id` to enforce project isolation.
    /// Returns an error if the server belongs to a different project.
    pub async fn execute_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Option<Value>,
        project_id: Uuid,
    ) -> Result<CallToolResult, String> {
        let server_id = {
            let index = self.name_index.read().await;
            index
                .get(server_name)
                .copied()
                .ok_or_else(|| format!("MCP server '{}' not found", server_name))?
        };

        let servers = self.servers.read().await;
        let state = servers
            .get(&server_id)
            .ok_or_else(|| format!("MCP server '{}' not found in registry", server_name))?;

        // Project isolation: prevent cross-project access
        if state.config.project_id != project_id {
            tracing::warn!(
                server_name = %server_name,
                server_project_id = %state.config.project_id,
                request_project_id = %project_id,
                "MCP server project isolation violation blocked"
            );
            return Err(format!("MCP server '{}' not found", server_name));
        }

        if state.status != McpServerStatus::Connected {
            return Err(format!(
                "MCP server '{}' is not connected: {:?}",
                server_name, state.status
            ));
        }

        state.client.call_tool(tool_name, arguments).await
    }

    /// List all registered servers with their status and tool counts.
    pub async fn list_servers(&self) -> Vec<McpServerInfo> {
        let servers = self.servers.read().await;
        servers
            .values()
            .map(|s| McpServerInfo {
                id: s.config.id,
                name: s.config.name.clone(),
                endpoint: s.config.endpoint.clone(),
                status: format!("{:?}", s.status),
                auth_type: s.auth_type.clone(),
                tool_count: s.tools.len(),
                tools: s.tools.iter().map(|t| t.name.clone()).collect(),
                last_refreshed_secs_ago: s.last_refreshed.elapsed().as_secs(),
                server_info: s.server_info.clone(),
            })
            .collect()
    }

    /// Get tools for a specific server.
    pub async fn get_server_tools(&self, id: &Uuid) -> Option<Vec<McpToolDef>> {
        let servers = self.servers.read().await;
        servers.get(id).map(|s| s.tools.clone())
    }

    /// Check if any MCP servers are registered.
    pub async fn has_servers(&self) -> bool {
        let servers = self.servers.read().await;
        !servers.is_empty()
    }

    /// Refresh all connected servers. Called by background task.
    pub async fn refresh_all(&self) {
        let ids: Vec<Uuid> = {
            let servers = self.servers.read().await;
            servers.keys().copied().collect()
        };

        for id in ids {
            if let Err(e) = self.refresh(&id).await {
                tracing::warn!(server_id = %id, error = %e, "Failed to refresh MCP server");
            }
        }
    }
}

/// Sanitize a server name from serverInfo to a safe identifier.
///
/// Rules:
/// - Lowercase alphanumeric, hyphens, and single underscores only
/// - Non-alphanumeric chars become hyphens
/// - Consecutive hyphens/underscores are collapsed to a single hyphen
///   (prevents `__` in names which would break `mcp__{server}__{tool}` parsing)
/// - Leading/trailing hyphens and underscores are trimmed
fn sanitize_server_name(name: &str) -> String {
    // Step 1: map each char to alphanumeric-or-hyphen
    let raw: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Step 2: collapse consecutive separators (`-`, `_`, or any mix) to a single `-`
    let mut collapsed = String::with_capacity(raw.len());
    let mut prev_was_sep = false;
    for c in raw.chars() {
        if c == '-' || c == '_' {
            if !prev_was_sep {
                collapsed.push('-');
            }
            prev_was_sep = true;
        } else {
            collapsed.push(c);
            prev_was_sep = false;
        }
    }

    // Step 3: trim leading/trailing hyphens
    collapsed.trim_matches('-').to_string()
}

/// Serializable server info for API responses.
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpServerInfo {
    pub id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub status: String,
    pub auth_type: String,
    pub tool_count: usize,
    pub tools: Vec<String>,
    pub last_refreshed_secs_ago: u64,
    pub server_info: Option<Implementation>,
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        let registry = McpRegistry::new();
        assert!(!registry.has_servers().await);
    }

    #[tokio::test]
    async fn test_empty_tools_by_name() {
        let registry = McpRegistry::new();
        let tools = registry
            .get_openai_tools_by_name(&["nonexistent".to_string()])
            .await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_execute_tool_unknown_server() {
        let registry = McpRegistry::new();
        let result = registry
            .execute_tool("nope", "tool", None, Uuid::nil())
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_unregister_nonexistent() {
        let registry = McpRegistry::new();
        let removed = registry.unregister(&Uuid::new_v4()).await;
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_list_servers_empty() {
        let registry = McpRegistry::new();
        let servers = registry.list_servers().await;
        assert!(servers.is_empty());
    }

    #[test]
    fn test_sanitize_server_name() {
        assert_eq!(sanitize_server_name("Brave Search"), "brave-search");
        assert_eq!(sanitize_server_name("slack-mcp"), "slack-mcp");
        assert_eq!(sanitize_server_name("My Server!@#"), "my-server");
        // Consecutive underscores collapse to a single hyphen (prevents __ in mcp__ prefix)
        assert_eq!(sanitize_server_name("GITHUB_Tools"), "github-tools");
        assert_eq!(sanitize_server_name("my__api"), "my-api");
        assert_eq!(sanitize_server_name("---test---"), "test");
        assert_eq!(sanitize_server_name("a__b__c"), "a-b-c");
    }

    #[test]
    fn test_discovery_result_serialization() {
        let result = DiscoveryResult {
            endpoint: "https://mcp.example.com".into(),
            requires_auth: true,
            auth_type: "oauth2".into(),
            token_endpoint: Some("https://auth.example.com/token".into()),
            scopes_supported: Some(vec!["tools:read".into()]),
            server_info: Some(Implementation {
                name: "test-server".into(),
                version: "1.0.0".into(),
            }),
            tools: vec![],
            tool_count: 0,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["requires_auth"], true);
        assert_eq!(json["auth_type"], "oauth2");
        assert!(json["token_endpoint"].is_string());
    }

    #[test]
    fn test_discovery_result_omits_none() {
        let result = DiscoveryResult {
            endpoint: "http://localhost/mcp".into(),
            requires_auth: false,
            auth_type: "none".into(),
            token_endpoint: None,
            scopes_supported: None,
            server_info: None,
            tools: vec![],
            tool_count: 0,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("token_endpoint").is_none());
        assert!(json.get("scopes_supported").is_none());
        assert!(json.get("server_info").is_none());
    }
}
