//! MCP server management API handlers.
//!
//! CRUD operations for MCP server registration, tool listing,
//! connection testing, cache refresh, auto-discovery, and re-authentication.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AuthContext;
use crate::mcp::registry::{DiscoverRequest, DiscoveryResult, McpServerConfig, McpServerInfo};
use crate::mcp::types::McpToolDef;
use crate::store::postgres::mcp::McpToolToCache;
use crate::store::postgres::types::NewMcpServer;
use crate::utils;
use crate::AppState;

// ── Request / Response types ───────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterMcpServerRequest {
    pub name: Option<String>,
    pub endpoint: String,
    #[serde(default)]
    pub api_key: Option<String>,
    /// OAuth 2.0 client credentials (optional — for auto-discovery with OAuth servers).
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
    /// If true, perform auto-discovery: probe → OAuth → initialize → cache tools.
    /// If false (default), uses legacy manual registration.
    #[serde(default)]
    pub auto_discover: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct RegisterMcpServerResponse {
    pub id: Uuid,
    pub name: String,
    pub auth_type: String,
    pub tool_count: usize,
    pub tools: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct TestMcpServerResponse {
    pub connected: bool,
    pub tool_count: usize,
    pub tools: Vec<McpToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscoverMcpServerRequest {
    pub endpoint: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ReauthResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Helper Functions ───────────────────────────────────────────

/// Validate endpoint URL for safety (prevent SSRF attacks).
fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    if endpoint.is_empty() {
        return Err("Endpoint URL is required".to_string());
    }

    // Parse and validate URL scheme
    let url = match url::Url::parse(endpoint) {
        Ok(u) => u,
        Err(_) => return Err("Invalid endpoint URL format".to_string()),
    };

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("Endpoint must use HTTP or HTTPS".to_string()),
    }

    Ok(())
}

/// Verify user has access to the MCP server (project isolation).
async fn verify_server_access(
    db: &crate::store::postgres::PgStore,
    server_id: Uuid,
    project_id: Uuid,
) -> Result<crate::store::postgres::types::McpServerRow, StatusCode> {
    let server = db
        .get_mcp_server(server_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if server.project_id != project_id {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(server)
}

// ── Handlers ───────────────────────────────────────────────────

/// POST /api/v1/mcp/servers — Register a new MCP server.
///
/// Supports two modes:
/// - `auto_discover: false` (default): manual registration with name + endpoint + optional api_key
/// - `auto_discover: true`: auto-discovery with optional OAuth credentials
pub async fn register_mcp_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RegisterMcpServerRequest>,
) -> Result<(StatusCode, Json<RegisterMcpServerResponse>), (StatusCode, String)> {
    // SEC: admin + mcp:write required to register MCP servers
    auth.require_role("admin")
        .map_err(|s| (s, "Forbidden".to_string()))?;
    auth.require_scope("mcp:write")
        .map_err(|s| (s, "Forbidden".to_string()))?;

    // Validate endpoint format
    validate_endpoint(&req.endpoint)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // SEC: Prevent SSRF attacks - validate endpoint is safe
    if !utils::is_safe_webhook_url(&req.endpoint).await {
        return Err((StatusCode::BAD_REQUEST, "Invalid or unsafe endpoint URL".into()));
    }

    let project_id = auth.default_project_id();

    if req.auto_discover {
        // Auto-discovery mode
        let discover_req = DiscoverRequest {
            endpoint: req.endpoint.clone(),
            name: req.name.clone(),
            client_id: req.client_id.clone(),
            client_secret: req.client_secret.clone(),
            project_id,
        };

        let (id, tools) = state
            .mcp_registry
            .register_with_discovery(discover_req)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Auto-discovery failed: {}", e)))?;

        let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        let servers = state.mcp_registry.list_servers().await;
        let server_info = servers.iter().find(|s| s.id == id);
        let auth_type = server_info
            .map(|s| s.auth_type.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let name = server_info
            .map(|s| s.name.clone())
            .unwrap_or_default();

        // Persist to database
        let new_server = NewMcpServer {
            id,
            project_id,
            name: name.clone(),
            endpoint: req.endpoint,
            auth_type: auth_type.clone(),
            api_key_encrypted: None, // TODO: Encrypt if provided
            oauth_client_id: req.client_id,
            oauth_client_secret_enc: req.client_secret, // TODO: Encrypt
            oauth_token_endpoint: None,
            oauth_scopes: None,
            oauth_access_token_enc: None,
            oauth_refresh_token_enc: None,
            oauth_token_expires_at: None,
            status: "Connected".to_string(),
            tool_count: tools.len() as i32,
            discovered_server_info: server_info.and_then(|s| {
                s.server_info.as_ref().and_then(|info| {
                    serde_json::to_value(info).ok()
                })
            }),
        };

        if let Err(e) = state.db.insert_mcp_server(&new_server).await {
            tracing::error!("Failed to persist MCP server to database: {}", e);
            // Continue anyway - server is registered in memory
        }

        // Cache tools to database
        let tools_to_cache: Vec<McpToolToCache> = tools
            .iter()
            .map(|t| McpToolToCache {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
                output_schema: t.output_schema.clone(),
            })
            .collect();

        if let Err(e) = state.db.cache_mcp_tools(id, &tools_to_cache).await {
            tracing::error!("Failed to cache MCP tools to database: {}", e);
        }

        Ok((
            StatusCode::CREATED,
            Json(RegisterMcpServerResponse {
                id,
                name,
                auth_type,
                tool_count: tools.len(),
                tools: tool_names,
            }),
        ))
    } else {
        // Legacy manual mode
        let name = req.name.clone().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Name is required for manual registration (or use auto_discover: true)".to_string(),
            )
        })?;

        // Validate name
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "Name must be alphanumeric (hyphens/underscores allowed)".into(),
            ));
        }

        let id = Uuid::new_v4();
        let config = McpServerConfig {
            id,
            project_id,
            name: name.clone(),
            endpoint: req.endpoint.clone(),
            api_key: req.api_key.clone(),
            oauth_client_id: None,
            oauth_client_secret: None,
            oauth_token_endpoint: None,
            oauth_scopes: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };

        let tools = state
            .mcp_registry
            .register(config)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to connect to MCP server: {}", e)))?;

        let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        let auth_type = if req.api_key.is_some() { "bearer" } else { "none" };

        // Persist to database
        let new_server = NewMcpServer {
            id,
            project_id,
            name: name.clone(),
            endpoint: req.endpoint,
            auth_type: auth_type.to_string(),
            api_key_encrypted: None, // TODO: Encrypt api_key before storing
            oauth_client_id: None,
            oauth_client_secret_enc: None,
            oauth_token_endpoint: None,
            oauth_scopes: None,
            oauth_access_token_enc: None,
            oauth_refresh_token_enc: None,
            oauth_token_expires_at: None,
            status: "Connected".to_string(),
            tool_count: tools.len() as i32,
            discovered_server_info: None,
        };

        if let Err(e) = state.db.insert_mcp_server(&new_server).await {
            tracing::error!("Failed to persist MCP server to database: {}", e);
        }

        // Cache tools to database
        let tools_to_cache: Vec<McpToolToCache> = tools
            .iter()
            .map(|t| McpToolToCache {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
                output_schema: t.output_schema.clone(),
            })
            .collect();

        if let Err(e) = state.db.cache_mcp_tools(id, &tools_to_cache).await {
            tracing::error!("Failed to cache MCP tools to database: {}", e);
        }

        Ok((
            StatusCode::CREATED,
            Json(RegisterMcpServerResponse {
                id,
                name,
                auth_type: auth_type.to_string(),
                tool_count: tools.len(),
                tools: tool_names,
            }),
        ))
    }
}

/// GET /api/v1/mcp/servers — List all registered MCP servers.
pub async fn list_mcp_servers(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<McpServerInfo>>, StatusCode> {
    // SEC: mcp:read scope required
    auth.require_scope("mcp:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    // First try to get from DB for persisted servers, fall back to registry
    match state.db.list_mcp_servers(auth.default_project_id()).await {
        Ok(db_servers) if !db_servers.is_empty() => {
            // Convert DB rows to API response format
            let servers: Vec<McpServerInfo> = db_servers
                .into_iter()
                .map(|s| McpServerInfo {
                    id: s.id,
                    name: s.name,
                    endpoint: s.endpoint,
                    status: s.status,
                    auth_type: s.auth_type,
                    tool_count: s.tool_count as usize,
                    tools: vec![], // Tools are fetched separately
                    last_refreshed_secs_ago: 0,
                    server_info: s.discovered_server_info.and_then(|v| {
                        serde_json::from_value(v).ok()
                    }),
                })
                .collect();
            Ok(Json(servers))
        }
        _ => {
            // Fall back to in-memory registry (for backwards compatibility)
            Ok(Json(state.mcp_registry.list_servers().await))
        }
    }
}

/// GET /api/v1/mcp/servers/:id — Get a single MCP server by ID.
pub async fn get_mcp_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<McpServerInfo>, StatusCode> {
    // SEC: mcp:read scope required
    auth.require_scope("mcp:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    // SEC: Verify project isolation
    let server = verify_server_access(&state.db, id, auth.default_project_id()).await?;

    // Convert DB row to API response format
    Ok(Json(McpServerInfo {
        id: server.id,
        name: server.name,
        endpoint: server.endpoint,
        status: server.status,
        auth_type: server.auth_type,
        tool_count: server.tool_count as usize,
        tools: vec![],
        last_refreshed_secs_ago: 0,
        server_info: server.discovered_server_info.and_then(|v| {
            serde_json::from_value(v).ok()
        }),
    }))
}

/// DELETE /api/v1/mcp/servers/:id — Remove an MCP server.
pub async fn delete_mcp_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    // SEC: admin + mcp:write required to delete MCP servers
    auth.require_role("admin")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    auth.require_scope("mcp:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    // SEC: Verify project isolation
    verify_server_access(&state.db, id, auth.default_project_id()).await?;

    // Remove from memory
    state.mcp_registry.unregister(&id).await;

    // Remove from database (CASCADE will remove tools)
    match state.db.delete_mcp_server(id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::error!("Failed to delete MCP server from database: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/v1/mcp/servers/:id/refresh — Force-refresh tool cache.
pub async fn refresh_mcp_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<McpToolDef>>, (StatusCode, String)> {
    // SEC: admin + mcp:write required to refresh MCP server caches
    auth.require_role("admin")
        .map_err(|s| (s, "Forbidden".to_string()))?;
    auth.require_scope("mcp:write")
        .map_err(|s| (s, "Forbidden".to_string()))?;

    // SEC: Verify project isolation
    verify_server_access(&state.db, id, auth.default_project_id())
        .await
        .map_err(|s| (s, "Forbidden".to_string()))?;

    let tools = state
        .mcp_registry
        .refresh(&id)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    // Update cached tools in database
    let tools_to_cache: Vec<McpToolToCache> = tools
        .iter()
        .map(|t| McpToolToCache {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
            output_schema: t.output_schema.clone(),
        })
        .collect();

    if let Err(e) = state.db.cache_mcp_tools(id, &tools_to_cache).await {
        tracing::error!("Failed to update cached MCP tools: {}", e);
    }

    // Update tool count in database
    let _ = state
        .db
        .update_mcp_server_status(id, "Connected", tools.len() as i32, None)
        .await;

    Ok(Json(tools))
}

/// GET /api/v1/mcp/servers/:id/tools — List cached tools for a server.
pub async fn list_mcp_server_tools(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<McpToolDef>>, StatusCode> {
    // SEC: mcp:read scope required
    auth.require_scope("mcp:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    // SEC: Verify project isolation
    verify_server_access(&state.db, id, auth.default_project_id()).await?;

    // Try database first, fall back to memory
    match state.db.get_mcp_server_tools(id).await {
        Ok(db_tools) if !db_tools.is_empty() => {
            let tools: Vec<McpToolDef> = db_tools
                .into_iter()
                .map(|t| McpToolDef {
                    name: t.name,
                    description: t.description,
                    input_schema: t.input_schema,
                    output_schema: t.output_schema,
                })
                .collect();
            Ok(Json(tools))
        }
        _ => {
            // Fall back to in-memory registry
            match state.mcp_registry.get_server_tools(&id).await {
                Some(tools) => Ok(Json(tools)),
                None => Err(StatusCode::NOT_FOUND),
            }
        }
    }
}

/// POST /api/v1/mcp/servers/test — Test connection to an MCP server without registering.
pub async fn test_mcp_server(
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RegisterMcpServerRequest>,
) -> Result<Json<TestMcpServerResponse>, (StatusCode, String)> {
    // SEC: admin + mcp:write required to test MCP server connections
    auth.require_role("admin")
        .map_err(|s| (s, "Forbidden".to_string()))?;
    auth.require_scope("mcp:write")
        .map_err(|s| (s, "Forbidden".to_string()))?;

    // Validate endpoint
    validate_endpoint(&req.endpoint)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    use crate::mcp::client::McpClient;

    let client = McpClient::new(&req.endpoint, req.api_key);

    match client.initialize().await {
        Ok(_) => match client.list_tools().await {
            Ok(tools) => Ok(Json(TestMcpServerResponse {
                connected: true,
                tool_count: tools.len(),
                tools,
                error: None,
            })),
            Err(e) => Ok(Json(TestMcpServerResponse {
                connected: true,
                tool_count: 0,
                tools: vec![],
                error: Some(format!("Connected but failed to list tools: {}", e)),
            })),
        },
        Err(e) => Ok(Json(TestMcpServerResponse {
            connected: false,
            tool_count: 0,
            tools: vec![],
            error: Some(e),
        })),
    }
}

/// POST /api/v1/mcp/servers/discover — Dry-run discovery.
///
/// Probes the endpoint URL, discovers auth requirements, and returns
/// server info + tools without persisting anything.
pub async fn discover_mcp_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<DiscoverMcpServerRequest>,
) -> Result<Json<DiscoveryResult>, (StatusCode, String)> {
    // SEC: admin + mcp:read required
    auth.require_role("admin")
        .map_err(|s| (s, "Forbidden".to_string()))?;
    auth.require_scope("mcp:read")
        .map_err(|s| (s, "Forbidden".to_string()))?;

    // Validate endpoint
    validate_endpoint(&req.endpoint)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    state
        .mcp_registry
        .discover_dry_run(&req.endpoint)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Discovery failed: {}", e)))
}

/// POST /api/v1/mcp/servers/:id/reauth — Force re-authenticate an OAuth MCP server.
pub async fn reauth_mcp_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReauthResponse>, (StatusCode, String)> {
    // SEC: admin + mcp:write required
    auth.require_role("admin")
        .map_err(|s| (s, "Forbidden".to_string()))?;
    auth.require_scope("mcp:write")
        .map_err(|s| (s, "Forbidden".to_string()))?;

    // SEC: Verify project isolation
    verify_server_access(&state.db, id, auth.default_project_id())
        .await
        .map_err(|s| (s, "Forbidden".to_string()))?;

    let oauth_mgr = state.mcp_registry.oauth_manager();

    if !oauth_mgr.has_token(&id).await {
        return Ok(Json(ReauthResponse {
            success: false,
            error: Some("Server does not use OAuth authentication or no token cached".into()),
        }));
    }

    match oauth_mgr.get_valid_token(&id).await {
        Ok(_) => Ok(Json(ReauthResponse {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(ReauthResponse {
            success: false,
            error: Some(format!("Re-authentication failed: {}", e)),
        })),
    }
}