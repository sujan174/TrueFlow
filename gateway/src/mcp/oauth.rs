//! OAuth 2.0 token lifecycle manager for MCP servers.
//!
//! Implements the MCP-mandated authorization flow:
//! - RFC 9728: Protected Resource Metadata discovery
//! - RFC 8414: Authorization Server Metadata discovery
//! - OAuth 2.0 client_credentials grant + token refresh
//! - Pre-emptive token refresh (60s before expiry)

use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ── Discovery types ────────────────────────────────────────────

/// Result of RFC 9728 Protected Resource Metadata discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedResourceMetadata {
    /// The resource server URL
    pub resource: String,
    /// Authorization server(s) that protect this resource
    #[serde(default)]
    pub authorization_servers: Vec<String>,
    /// Scopes supported by this resource
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

/// Result of RFC 8414 Authorization Server Metadata discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthServerMetadata {
    pub issuer: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub authorization_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub grant_types_supported: Vec<String>,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
}

/// Combined discovery result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthDiscovery {
    pub resource_metadata: Option<ProtectedResourceMetadata>,
    pub auth_server: AuthServerMetadata,
    pub requires_auth: bool,
}

// ── Token types ────────────────────────────────────────────────

/// Token set returned from token endpoint.
/// SEC: Custom Debug impl redacts access_token and refresh_token to prevent logging secrets.
#[derive(Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("scope", &self.scope)
            .finish()
    }
}

/// Cached token with absolute expiry time.
/// SEC: Custom Debug impl redacts access_token, refresh_token, and client_secret.
#[derive(Clone)]
pub struct CachedToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub token_endpoint: String,
    #[allow(dead_code)]
    pub client_id: String,
    #[allow(dead_code)]
    pub client_secret: String,
    pub scopes: Vec<String>,
}

impl std::fmt::Debug for CachedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedToken")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_at", &self.expires_at)
            .field("token_endpoint", &self.token_endpoint)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .finish()
    }
}

impl CachedToken {
    /// Returns true if the token will expire within the given duration.
    pub fn expires_within(&self, duration: Duration) -> bool {
        Utc::now() + duration >= self.expires_at
    }

    /// Returns true if the token is already expired.
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }
}

// ── OAuth Token Manager ────────────────────────────────────────

/// Pre-emptive refresh window (refresh 60s before actual expiry).
const PREEMPTIVE_REFRESH_SECS: i64 = 60;

/// Maximum retries for token refresh.
const MAX_REFRESH_RETRIES: u32 = 3;

/// Manages OAuth 2.0 token lifecycle for MCP servers.
///
/// Handles:
/// - RFC 9728/8414 metadata discovery
/// - Token acquisition via client_credentials grant
/// - Pre-emptive token refresh
/// - In-memory token cache with DB persistence
pub struct OAuthTokenManager {
    http: Client,
    cache: Arc<RwLock<HashMap<Uuid, CachedToken>>>,
}

impl Default for OAuthTokenManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthTokenManager {
    pub fn new() -> Self {
        Self::try_new().expect(
            "failed to build OAuth HTTP client: check network configuration and system resources",
        )
    }

    pub fn try_new() -> Result<Self, String> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| format!("failed to build OAuth HTTP client: {}", e))?;

        Ok(Self {
            http,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Discover OAuth metadata for a given MCP server endpoint.
    ///
    /// Flow:
    /// 1. Probe endpoint — check for 401
    /// 2. Fetch `/.well-known/oauth-protected-resource` (RFC 9728)
    /// 3. Extract authorization_server URL
    /// 4. Fetch `{as}/.well-known/oauth-authorization-server` (RFC 8414)
    pub async fn discover(&self, endpoint: &str) -> Result<OAuthDiscovery, String> {
        let base_url = extract_base_url(endpoint);

        // Step 1: Try fetching protected resource metadata (RFC 9728)
        let resource_url = format!("{}/.well-known/oauth-protected-resource", base_url);
        let resource_meta = self
            .fetch_json::<ProtectedResourceMetadata>(&resource_url)
            .await
            .ok();

        // Step 2: Determine authorization server URL
        let as_url = if let Some(ref meta) = resource_meta {
            meta.authorization_servers
                .first()
                .cloned()
                .unwrap_or_else(|| base_url.clone())
        } else {
            base_url.clone()
        };

        // Step 3: Fetch authorization server metadata (RFC 8414)
        let as_meta_url = format!("{}/.well-known/oauth-authorization-server", as_url);
        let auth_server = self
            .fetch_json::<AuthServerMetadata>(&as_meta_url)
            .await
            .map_err(|e| {
                format!(
                    "Failed to discover OAuth authorization server at {}: {}",
                    as_meta_url, e
                )
            })?;

        // Determine if auth is required by probing the endpoint
        let requires_auth = self.probe_requires_auth(endpoint).await;

        Ok(OAuthDiscovery {
            resource_metadata: resource_meta,
            auth_server,
            requires_auth,
        })
    }

    /// Acquire an access token using client_credentials grant.
    pub async fn acquire_token(
        &self,
        token_endpoint: &str,
        client_id: &str,
        client_secret: &str,
        scopes: &[String],
    ) -> Result<TokenResponse, String> {
        let mut form = vec![
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];

        let scope_str = scopes.join(" ");
        if !scope_str.is_empty() {
            form.push(("scope", &scope_str));
        }

        let resp = self
            .http
            .post(token_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("Token request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Token endpoint returned {}: {}", status, body));
        }

        let token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse token response: {}", e))?;

        Ok(token_resp)
    }

    /// Refresh an access token using a refresh_token grant.
    pub async fn refresh_token(
        &self,
        token_endpoint: &str,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Result<TokenResponse, String> {
        let resp = self
            .http
            .post(token_endpoint)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await
            .map_err(|e| format!("Refresh token request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Refresh token endpoint returned {}: {}",
                status, body
            ));
        }

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse refresh token response: {}", e))
    }

    /// Get a valid access token for a server, refreshing if needed.
    ///
    /// Uses pre-emptive refresh: refreshes 60s before actual expiry.
    pub async fn get_valid_token(&self, server_id: &Uuid) -> Result<String, String> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(server_id) {
                if !cached.expires_within(Duration::seconds(PREEMPTIVE_REFRESH_SECS)) {
                    return Ok(cached.access_token.clone());
                }
            }
        }

        // Need to refresh or re-acquire
        let cached = {
            let cache = self.cache.read().await;
            cache.get(server_id).cloned()
        };

        let cached = cached.ok_or_else(|| format!("No cached token for server {}", server_id))?;

        // Try refresh first if we have a refresh token
        if let Some(ref rt) = cached.refresh_token {
            for attempt in 0..MAX_REFRESH_RETRIES {
                match self
                    .refresh_token(
                        &cached.token_endpoint,
                        &cached.client_id,
                        &cached.client_secret,
                        rt,
                    )
                    .await
                {
                    Ok(resp) => {
                        let new_cached = self.token_response_to_cached(&resp, &cached);
                        let token = new_cached.access_token.clone();
                        self.cache.write().await.insert(*server_id, new_cached);
                        return Ok(token);
                    }
                    Err(e) => {
                        if attempt < MAX_REFRESH_RETRIES - 1 {
                            let delay = std::time::Duration::from_millis(1000 * 2u64.pow(attempt));
                            tracing::warn!(
                                server_id = %server_id,
                                attempt = attempt + 1,
                                error = %e,
                                "Token refresh failed, retrying in {:?}",
                                delay
                            );
                            tokio::time::sleep(delay).await;
                        }
                    }
                }
            }
        }

        // Fallback: re-acquire via client_credentials
        let resp = self
            .acquire_token(
                &cached.token_endpoint,
                &cached.client_id,
                &cached.client_secret,
                &cached.scopes,
            )
            .await?;

        let new_cached = self.token_response_to_cached(&resp, &cached);
        let token = new_cached.access_token.clone();
        self.cache.write().await.insert(*server_id, new_cached);

        Ok(token)
    }

    /// Store initial token in cache after acquisition.
    pub async fn store_token(
        &self,
        server_id: Uuid,
        token_resp: &TokenResponse,
        token_endpoint: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    ) {
        let expires_at = Utc::now() + Duration::seconds(token_resp.expires_in.unwrap_or(3600));

        let cached = CachedToken {
            access_token: token_resp.access_token.clone(),
            refresh_token: token_resp.refresh_token.clone(),
            expires_at,
            token_endpoint,
            client_id,
            client_secret,
            scopes,
        };

        self.cache.write().await.insert(server_id, cached);
    }

    /// Remove a server's cached token.
    pub async fn remove_token(&self, server_id: &Uuid) {
        self.cache.write().await.remove(server_id);
    }

    /// Check if a server has a cached token.
    pub async fn has_token(&self, server_id: &Uuid) -> bool {
        self.cache.read().await.contains_key(server_id)
    }

    // ── Private helpers ────────────────────────────────────────

    fn token_response_to_cached(&self, resp: &TokenResponse, prev: &CachedToken) -> CachedToken {
        let expires_at = Utc::now() + Duration::seconds(resp.expires_in.unwrap_or(3600));

        CachedToken {
            access_token: resp.access_token.clone(),
            refresh_token: resp
                .refresh_token
                .clone()
                .or_else(|| prev.refresh_token.clone()),
            expires_at,
            token_endpoint: prev.token_endpoint.clone(),
            client_id: prev.client_id.clone(),
            client_secret: prev.client_secret.clone(),
            scopes: prev.scopes.clone(),
        }
    }

    async fn fetch_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, String> {
        let resp = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("GET {} failed: {}", url, e))?;

        if !resp.status().is_success() {
            return Err(format!("GET {} returned {}", url, resp.status()));
        }

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse JSON from {}: {}", url, e))
    }

    async fn probe_requires_auth(&self, endpoint: &str) -> bool {
        match self.http.get(endpoint).send().await {
            Ok(resp) => resp.status() == reqwest::StatusCode::UNAUTHORIZED,
            Err(_) => false,
        }
    }
}

/// Extract the base URL (scheme + authority) from an endpoint URL.
fn extract_base_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        format!("{}://{}", parsed.scheme(), parsed.authority())
    } else {
        url.to_string()
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_base_url() {
        assert_eq!(
            extract_base_url("https://mcp.example.com/v1/mcp"),
            "https://mcp.example.com"
        );
        assert_eq!(
            extract_base_url("http://localhost:3001/mcp"),
            "http://localhost:3001"
        );
        // Note: url crate strips default ports (443 for https, 80 for http)
        assert_eq!(
            extract_base_url("https://api.slack.com:443/mcp/v2"),
            "https://api.slack.com"
        );
    }

    #[test]
    fn test_cached_token_expiry() {
        let token = CachedToken {
            access_token: "test-token".into(),
            refresh_token: None,
            expires_at: Utc::now() + Duration::seconds(30),
            token_endpoint: "https://auth.example.com/token".into(),
            client_id: "client".into(),
            client_secret: "secret".into(),
            scopes: vec![],
        };

        assert!(!token.is_expired());
        // 30s remaining, so within 60s window → should refresh
        assert!(token.expires_within(Duration::seconds(60)));
        // But not within 10s window
        assert!(!token.expires_within(Duration::seconds(10)));
    }

    #[test]
    fn test_cached_token_already_expired() {
        let token = CachedToken {
            access_token: "expired-token".into(),
            refresh_token: Some("refresh-me".into()),
            expires_at: Utc::now() - Duration::seconds(10),
            token_endpoint: "https://auth.example.com/token".into(),
            client_id: "client".into(),
            client_secret: "secret".into(),
            scopes: vec!["tools:read".into()],
        };

        assert!(token.is_expired());
        assert!(token.expires_within(Duration::seconds(0)));
    }

    #[test]
    fn test_token_response_deserialization() {
        let json = serde_json::json!({
            "access_token": "eyJ...",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2ggdG9rZW4",
            "scope": "tools:read tools:call"
        });

        let resp: TokenResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.access_token, "eyJ...");
        assert_eq!(resp.token_type, "Bearer");
        assert_eq!(resp.expires_in, Some(3600));
        assert_eq!(
            resp.refresh_token.as_deref(),
            Some("dGhpcyBpcyBhIHJlZnJlc2ggdG9rZW4")
        );
        assert_eq!(resp.scope.as_deref(), Some("tools:read tools:call"));
    }

    #[test]
    fn test_token_response_minimal() {
        let json = serde_json::json!({
            "access_token": "abc123",
            "token_type": "bearer"
        });

        let resp: TokenResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.access_token, "abc123");
        assert!(resp.expires_in.is_none());
        assert!(resp.refresh_token.is_none());
        assert!(resp.scope.is_none());
    }

    #[test]
    fn test_protected_resource_metadata_deserialization() {
        let json = serde_json::json!({
            "resource": "https://mcp.example.com",
            "authorization_servers": ["https://auth.example.com"],
            "scopes_supported": ["tools:read", "tools:call"]
        });

        let meta: ProtectedResourceMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(meta.resource, "https://mcp.example.com");
        assert_eq!(meta.authorization_servers.len(), 1);
        assert_eq!(meta.scopes_supported.len(), 2);
    }

    #[test]
    fn test_auth_server_metadata_deserialization() {
        let json = serde_json::json!({
            "issuer": "https://auth.example.com",
            "token_endpoint": "https://auth.example.com/oauth/token",
            "authorization_endpoint": "https://auth.example.com/oauth/authorize",
            "scopes_supported": ["tools:read"],
            "grant_types_supported": ["client_credentials", "refresh_token"]
        });

        let meta: AuthServerMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(meta.issuer, "https://auth.example.com");
        assert_eq!(meta.token_endpoint, "https://auth.example.com/oauth/token");
        assert!(meta.authorization_endpoint.is_some());
        assert!(meta
            .grant_types_supported
            .contains(&"client_credentials".into()));
    }

    #[test]
    fn test_auth_server_metadata_minimal() {
        let json = serde_json::json!({
            "issuer": "https://auth.example.com",
            "token_endpoint": "https://auth.example.com/token"
        });

        let meta: AuthServerMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(meta.token_endpoint, "https://auth.example.com/token");
        assert!(meta.authorization_endpoint.is_none());
        assert!(meta.scopes_supported.is_empty());
        assert!(meta.grant_types_supported.is_empty());
    }

    #[test]
    fn test_oauth_discovery_serialization() {
        let discovery = OAuthDiscovery {
            resource_metadata: None,
            auth_server: AuthServerMetadata {
                issuer: "https://auth.example.com".into(),
                token_endpoint: "https://auth.example.com/token".into(),
                authorization_endpoint: None,
                scopes_supported: vec![],
                grant_types_supported: vec!["client_credentials".into()],
                response_types_supported: vec![],
            },
            requires_auth: true,
        };

        let json = serde_json::to_value(&discovery).unwrap();
        assert_eq!(json["requires_auth"], true);
        assert_eq!(
            json["auth_server"]["token_endpoint"],
            "https://auth.example.com/token"
        );
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let mgr = OAuthTokenManager::new();
        let id = Uuid::new_v4();
        assert!(!mgr.has_token(&id).await);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_token() {
        let mgr = OAuthTokenManager::new();
        let server_id = Uuid::new_v4();

        let token_resp = TokenResponse {
            access_token: "test-access-token".into(),
            token_type: "Bearer".into(),
            expires_in: Some(7200),
            refresh_token: Some("test-refresh".into()),
            scope: None,
        };

        mgr.store_token(
            server_id,
            &token_resp,
            "https://auth.example.com/token".into(),
            "client-id".into(),
            "client-secret".into(),
            vec!["tools:read".into()],
        )
        .await;

        assert!(mgr.has_token(&server_id).await);

        // Token should be valid since we set expires_in = 7200
        let token = mgr.get_valid_token(&server_id).await.unwrap();
        assert_eq!(token, "test-access-token");
    }

    #[tokio::test]
    async fn test_remove_token() {
        let mgr = OAuthTokenManager::new();
        let server_id = Uuid::new_v4();

        let token_resp = TokenResponse {
            access_token: "to-be-removed".into(),
            token_type: "Bearer".into(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: None,
        };

        mgr.store_token(
            server_id,
            &token_resp,
            "https://auth.example.com/token".into(),
            "c".into(),
            "s".into(),
            vec![],
        )
        .await;

        assert!(mgr.has_token(&server_id).await);
        mgr.remove_token(&server_id).await;
        assert!(!mgr.has_token(&server_id).await);
    }

    #[tokio::test]
    async fn test_get_valid_token_not_cached() {
        let mgr = OAuthTokenManager::new();
        let id = Uuid::new_v4();

        let result = mgr.get_valid_token(&id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No cached token"));
    }
}
