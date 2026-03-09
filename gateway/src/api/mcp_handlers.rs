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

#[allow(dead_code)]
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

    if req.endpoint.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Endpoint URL is required".into()));
    }

    if req.auto_discover {
        // Auto-discovery mode
        let discover_req = DiscoverRequest {
            endpoint: req.endpoint,
            name: req.name,
            client_id: req.client_id,
            client_secret: req.client_secret,
            project_id: auth.default_project_id(),
        };

        match state
            .mcp_registry
            .register_with_discovery(discover_req)
            .await
        {
            Ok((id, tools)) => {
                let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
                let servers = state.mcp_registry.list_servers().await;
                let auth_type = servers
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.auth_type.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let name = servers
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();

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
            }
            Err(e) => Err((
                StatusCode::BAD_GATEWAY,
                format!("Auto-discovery failed: {}", e),
            )),
        }
    } else {
        // Legacy manual mode
        let name = req.name.ok_or_else(|| {
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

        let config = McpServerConfig {
            id: Uuid::new_v4(),
            project_id: auth.default_project_id(),
            name: name.clone(),
            endpoint: req.endpoint,
            api_key: req.api_key,
        };

        let id = config.id;

        match state.mcp_registry.register(config).await {
            Ok(tools) => {
                let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
                Ok((
                    StatusCode::CREATED,
                    Json(RegisterMcpServerResponse {
                        id,
                        name,
                        auth_type: "bearer".to_string(),
                        tool_count: tools.len(),
                        tools: tool_names,
                    }),
                ))
            }
            Err(e) => Err((
                StatusCode::BAD_GATEWAY,
                format!("Failed to connect to MCP server: {}", e),
            )),
        }
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
    Ok(Json(state.mcp_registry.list_servers().await))
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
    if state.mcp_registry.unregister(&id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
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
    match state.mcp_registry.refresh(&id).await {
        Ok(tools) => Ok(Json(tools)),
        Err(e) => Err((StatusCode::BAD_GATEWAY, e)),
    }
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
    match state.mcp_registry.get_server_tools(&id).await {
        Some(tools) => Ok(Json(tools)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /api/v1/mcp/servers/test — Test connection to an MCP server without registering.
pub async fn test_mcp_server(
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RegisterMcpServerRequest>,
) -> Result<Json<TestMcpServerResponse>, StatusCode> {
    // SEC: admin + mcp:write required to test MCP server connections
    auth.require_role("admin")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    auth.require_scope("mcp:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;
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

    if req.endpoint.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Endpoint URL is required".into()));
    }

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
