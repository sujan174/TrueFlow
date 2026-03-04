//! MCP client — Streamable HTTP transport.
//!
//! Implements the MCP protocol over HTTP(S):
//! - `initialize` handshake
//! - `tools/list` to discover available tools
//! - `tools/call` to execute a tool
//!
//! Uses JSON-RPC 2.0 over HTTP POST as specified by MCP Streamable HTTP transport.
//! Supports three auth modes: None, Bearer (static API key), OAuth 2.0 (dynamic token).

use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use super::oauth::OAuthTokenManager;
use super::types::*;

// ── Auth Modes ─────────────────────────────────────────────────

/// Authentication mode for an MCP server connection.
#[derive(Clone)]
pub enum McpAuth {
    /// No authentication.
    None,
    /// Static Bearer token (API key).
    Bearer(String),
    /// OAuth 2.0 with dynamic token refresh.
    OAuth {
        manager: Arc<OAuthTokenManager>,
        server_id: Uuid,
    },
}

impl std::fmt::Debug for McpAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "McpAuth::None"),
            Self::Bearer(_) => write!(f, "McpAuth::Bearer(****)"),
            Self::OAuth { server_id, .. } => write!(f, "McpAuth::OAuth({})", server_id),
        }
    }
}

// ── MCP Client ─────────────────────────────────────────────────

/// MCP client for a single MCP server (Streamable HTTP transport).
pub struct McpClient {
    endpoint: String,
    auth: McpAuth,
    http: Client,
    request_id: AtomicU64,
    /// Session ID returned by server during initialization (if any).
    session_id: std::sync::Mutex<Option<String>>,
}

impl McpClient {
    /// Create a new MCP client for the given endpoint.
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>) -> Self {
        let auth = match api_key {
            Some(key) => McpAuth::Bearer(key),
            None => McpAuth::None,
        };
        Self::with_auth(endpoint, auth)
    }

    /// Create a new MCP client with a specific auth mode.
    pub fn with_auth(endpoint: impl Into<String>, auth: McpAuth) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build HTTP client");

        Self {
            endpoint: endpoint.into(),
            auth,
            http,
            request_id: AtomicU64::new(1),
            session_id: std::sync::Mutex::new(None),
        }
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Resolve the current auth token to attach to requests.
    async fn resolve_auth_header(&self) -> Result<Option<String>, String> {
        match &self.auth {
            McpAuth::None => Ok(None),
            McpAuth::Bearer(key) => Ok(Some(format!("Bearer {}", key))),
            McpAuth::OAuth { manager, server_id } => {
                let token = manager.get_valid_token(server_id).await?;
                Ok(Some(format!("Bearer {}", token)))
            }
        }
    }

    /// Send a JSON-RPC request to the MCP server and return the parsed result.
    async fn rpc(&self, method: &str, params: Option<Value>) -> Result<Value, String> {
        self.rpc_inner(method, params, true).await
    }

    /// Inner RPC implementation with optional retry on 401.
    fn rpc_inner(
        &self,
        method: &str,
        params: Option<Value>,
        retry_on_401: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + '_>> {
        let method = method.to_string();
        Box::pin(async move {
        let req = JsonRpcRequest::new(self.next_id(), &method, params.clone());

        let mut http_req = self
            .http
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        // Attach auth header
        if let Some(auth_header) = self.resolve_auth_header().await? {
            http_req = http_req.header("Authorization", auth_header);
        }

        // Attach session ID if we have one
        if let Ok(guard) = self.session_id.lock() {
            if let Some(sid) = guard.as_ref() {
                http_req = http_req.header("Mcp-Session-Id", sid.clone());
            }
        }

        let resp = http_req
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("MCP request failed: {}", e))?;

        // Capture session ID from response headers
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            if let Ok(sid_str) = sid.to_str() {
                if let Ok(mut guard) = self.session_id.lock() {
                    *guard = Some(sid_str.to_string());
                }
            }
        }

        let status = resp.status();

        // Handle 401: refresh OAuth token and retry once
        if status == reqwest::StatusCode::UNAUTHORIZED && retry_on_401 {
            if let McpAuth::OAuth { manager, server_id } = &self.auth {
                tracing::info!(
                    server_id = %server_id,
                    "MCP server returned 401, attempting token refresh and retry"
                );
                // Force refresh by getting a new token
                let _ = manager.get_valid_token(server_id).await;
                return self.rpc_inner(&method, params, false).await;
            }
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("MCP server returned {}: {}", status, body));
        }

        // Parse JSON-RPC response
        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read MCP response: {}", e))?;
        let rpc_resp: JsonRpcResponse = serde_json::from_str(&body).map_err(|e| {
            format!(
                "Invalid JSON-RPC response: {} (body: {})",
                e,
                &body[..body.len().min(200)]
            )
        })?;

        if let Some(err) = rpc_resp.error {
            return Err(format!("{}", err));
        }

        rpc_resp
            .result
            .ok_or_else(|| "MCP response missing both result and error".to_string())
        })
    }

    /// Perform the MCP `initialize` handshake.
    pub async fn initialize(&self) -> Result<InitializeResult, String> {
        let params = serde_json::to_value(InitializeParams {
            protocol_version: "2025-06-18".to_string(),
            capabilities: ClientCapabilities {},
            client_info: Implementation {
                name: "trueflow-gateway".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
        .map_err(|e| format!("Failed to serialize initialize params: {}", e))?;

        let result = self.rpc("initialize", Some(params)).await?;
        let init: InitializeResult = serde_json::from_value(result)
            .map_err(|e| format!("Failed to parse initialize result: {}", e))?;

        // Send `initialized` notification (no response expected, but we send it per spec)
        let _ = self
            .rpc("notifications/initialized", Some(serde_json::json!({})))
            .await;

        tracing::info!(
            server = ?init.server_info,
            protocol = %init.protocol_version,
            "MCP server initialized"
        );

        Ok(init)
    }

    /// Fetch the list of tools from the MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDef>, String> {
        let mut all_tools = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let params = cursor.as_ref().map(|c| serde_json::json!({ "cursor": c }));

            let result = self.rpc("tools/list", params).await?;
            let page: ListToolsResult = serde_json::from_value(result)
                .map_err(|e| format!("Failed to parse tools/list result: {}", e))?;

            all_tools.extend(page.tools);

            match page.next_cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => break,
            }
        }

        Ok(all_tools)
    }

    /// Execute a tool on the MCP server.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<CallToolResult, String> {
        let params = serde_json::to_value(CallToolParams {
            name: name.to_string(),
            arguments,
        })
        .map_err(|e| format!("Failed to serialize call_tool params: {}", e))?;

        let result = self.rpc("tools/call", Some(params)).await?;
        let call_result: CallToolResult = serde_json::from_value(result)
            .map_err(|e| format!("Failed to parse tools/call result: {}", e))?;

        Ok(call_result)
    }

    /// Simple health check — attempts initialization.
    #[allow(dead_code)]
    pub async fn health_check(&self) -> Result<(), String> {
        self.initialize().await?;
        Ok(())
    }

    /// Get a reference to the endpoint URL.
    #[allow(dead_code)]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = McpClient::new("http://localhost:3000/mcp", None);
        assert_eq!(client.endpoint, "http://localhost:3000/mcp");
        assert!(matches!(client.auth, McpAuth::None));
    }

    #[test]
    fn test_client_with_api_key() {
        let client = McpClient::new("http://example.com/mcp", Some("sk-test".into()));
        assert!(matches!(client.auth, McpAuth::Bearer(_)));
    }

    #[test]
    fn test_client_with_oauth_auth() {
        let mgr = Arc::new(OAuthTokenManager::new());
        let server_id = Uuid::new_v4();
        let client = McpClient::with_auth(
            "http://example.com/mcp",
            McpAuth::OAuth {
                manager: mgr,
                server_id,
            },
        );
        assert!(matches!(client.auth, McpAuth::OAuth { .. }));
    }

    #[test]
    fn test_request_id_increments() {
        let client = McpClient::new("http://localhost/mcp", None);
        let id1 = client.next_id();
        let id2 = client.next_id();
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn test_auth_debug_format() {
        assert_eq!(format!("{:?}", McpAuth::None), "McpAuth::None");
        assert_eq!(format!("{:?}", McpAuth::Bearer("secret".into())), "McpAuth::Bearer(****)");

        let mgr = Arc::new(OAuthTokenManager::new());
        let id = Uuid::nil();
        let oauth = McpAuth::OAuth { manager: mgr, server_id: id };
        assert!(format!("{:?}", oauth).contains("McpAuth::OAuth"));
    }

    #[tokio::test]
    async fn test_resolve_auth_header_none() {
        let client = McpClient::new("http://localhost/mcp", None);
        let header = client.resolve_auth_header().await.unwrap();
        assert!(header.is_none());
    }

    #[tokio::test]
    async fn test_resolve_auth_header_bearer() {
        let client = McpClient::new("http://localhost/mcp", Some("my-key".into()));
        let header = client.resolve_auth_header().await.unwrap();
        assert_eq!(header.as_deref(), Some("Bearer my-key"));
    }

    #[tokio::test]
    async fn test_resolve_auth_header_oauth_no_token() {
        let mgr = Arc::new(OAuthTokenManager::new());
        let server_id = Uuid::new_v4();
        let client = McpClient::with_auth(
            "http://localhost/mcp",
            McpAuth::OAuth {
                manager: mgr,
                server_id,
            },
        );
        // No token stored → should error
        let result = client.resolve_auth_header().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_auth_header_oauth_with_token() {
        let mgr = Arc::new(OAuthTokenManager::new());
        let server_id = Uuid::new_v4();

        // Store a token
        let token_resp = super::super::oauth::TokenResponse {
            access_token: "oauth-access-1234".into(),
            token_type: "Bearer".into(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: None,
        };
        mgr.store_token(
            server_id,
            &token_resp,
            "https://auth.example.com/token".into(),
            "cid".into(),
            "csecret".into(),
            vec![],
        )
        .await;

        let client = McpClient::with_auth(
            "http://localhost/mcp",
            McpAuth::OAuth {
                manager: mgr,
                server_id,
            },
        );

        let header = client.resolve_auth_header().await.unwrap();
        assert_eq!(header.as_deref(), Some("Bearer oauth-access-1234"));
    }
}
