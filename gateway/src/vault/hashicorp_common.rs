//! Shared types and utilities for HashiCorp Vault integrations.
//!
//! This module provides common types used by both the Transit and KV v2
//! secrets engine backends.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default timeout for Vault API calls.
pub const VAULT_TIMEOUT_SECS: u64 = 30;

/// Vault token refresh buffer (refresh 5 minutes before expiry).
pub const TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

// ============================================================================
// Base Configuration
// ============================================================================

/// Base configuration fields shared by all HashiCorp Vault backends.
///
/// This struct contains the common authentication and connection settings
/// that are needed regardless of which Vault secrets engine is used.
#[derive(Clone, Deserialize, Serialize)]
pub struct HashiCorpVaultBaseConfig {
    /// Vault server address (e.g., https://vault.example.com:8200)
    pub address: String,
    /// Secrets engine mount path (e.g., "transit" or "secret")
    pub mount_path: String,
    /// Vault namespace (Enterprise only)
    pub namespace: Option<String>,
    /// Authentication method: "approle" or "kubernetes"
    pub auth_method: String,
    /// AppRole role ID (for approle auth)
    pub approle_role_id: Option<String>,
    /// AppRole secret ID (for approle auth)
    pub approle_secret_id: Option<String>,
    /// Kubernetes auth role (for k8s auth)
    pub k8s_role: Option<String>,
    /// Path to Kubernetes JWT token (default: /var/run/secrets/kubernetes.io/serviceaccount/token)
    pub k8s_jwt_path: Option<String>,
    /// Skip TLS verification (not recommended for production)
    #[serde(default)]
    pub skip_tls_verify: bool,
}

impl std::fmt::Debug for HashiCorpVaultBaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashiCorpVaultBaseConfig")
            .field("address", &self.address)
            .field("mount_path", &self.mount_path)
            .field("namespace", &self.namespace)
            .field("auth_method", &self.auth_method)
            .field("approle_role_id", &self.approle_role_id)
            .field("approle_secret_id", &"[REDACTED]")
            .field("k8s_role", &self.k8s_role)
            .field("k8s_jwt_path", &self.k8s_jwt_path)
            .field("skip_tls_verify", &self.skip_tls_verify)
            .finish()
    }
}

// ============================================================================
// Vault Token
// ============================================================================

/// Cached Vault token with expiry information.
#[derive(Clone)]
pub struct VaultToken {
    /// The token string
    pub token: String,
    /// Token expiry time (Unix timestamp in seconds)
    pub expires_at: u64,
}

impl VaultToken {
    /// Check if the token needs refresh (within buffer period of expiry).
    pub fn needs_refresh(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now + TOKEN_REFRESH_BUFFER_SECS >= self.expires_at
    }
}

// ============================================================================
// Vault API Types
// ============================================================================

/// AppRole login request.
#[derive(Serialize)]
pub struct AppRoleLoginRequest {
    pub role_id: String,
    pub secret_id: String,
}

/// AppRole login response.
#[derive(Deserialize)]
pub struct AppRoleLoginResponse {
    pub auth: VaultAuth,
}

/// Kubernetes login request.
#[derive(Serialize)]
pub struct KubernetesLoginRequest {
    pub role: String,
    pub jwt: String,
}

/// Kubernetes login response.
#[derive(Deserialize)]
pub struct KubernetesLoginResponse {
    pub auth: VaultAuth,
}

/// Common auth response fields.
#[derive(Deserialize)]
pub struct VaultAuth {
    pub client_token: String,
    pub lease_duration: u64,
}

/// Vault health response.
#[derive(Deserialize)]
pub struct HealthResponse {
    /// Whether Vault is initialized
    #[serde(default)]
    pub initialized: bool,
    /// Whether Vault is sealed
    #[serde(default)]
    pub sealed: bool,
    /// Whether Vault is in standby
    #[serde(default)]
    pub standby: bool,
}

/// Vault error response.
#[derive(Deserialize)]
pub struct VaultErrorResponse {
    pub errors: Vec<String>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Build an HTTP client for Vault API calls.
///
/// # Arguments
/// * `skip_tls_verify` - Whether to skip TLS certificate verification
///
/// # Returns
/// A configured reqwest::Client
pub fn build_vault_client(skip_tls_verify: bool) -> anyhow::Result<Client> {
    let mut client_builder = Client::builder()
        .timeout(Duration::from_secs(VAULT_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(10));

    if skip_tls_verify {
        client_builder = client_builder.danger_accept_invalid_certs(true);
        tracing::warn!(
            "TLS verification disabled for Vault connection - not recommended for production"
        );
    }

    client_builder.build().map_err(Into::into)
}

/// Validate the base configuration fields.
///
/// # Arguments
/// * `config` - The base configuration to validate
///
/// # Errors
/// Returns an error if:
/// - The address is not a valid URL
/// - The auth method is not supported
/// - Required fields for the auth method are missing
/// - The mount path is empty
pub fn validate_base_config(config: &HashiCorpVaultBaseConfig) -> anyhow::Result<()> {
    use url::Url;

    // Validate address is a valid URL
    Url::parse(&config.address)
        .map_err(|e| anyhow::anyhow!("Invalid Vault address '{}': {}", config.address, e))?;

    // Validate auth method and required fields
    match config.auth_method.as_str() {
        "approle" => {
            if config.approle_role_id.is_none()
                || config.approle_role_id.as_ref().map_or(true, |s| s.is_empty())
            {
                anyhow::bail!("approle_role_id is required for AppRole authentication");
            }
            if config.approle_secret_id.is_none()
                || config.approle_secret_id.as_ref().map_or(true, |s| s.is_empty())
            {
                anyhow::bail!("approle_secret_id is required for AppRole authentication");
            }
        }
        "kubernetes" => {
            if config.k8s_role.is_none() || config.k8s_role.as_ref().map_or(true, |s| s.is_empty())
            {
                anyhow::bail!("k8s_role is required for Kubernetes authentication");
            }
        }
        _ => {
            anyhow::bail!(
                "Unsupported auth method '{}'. Supported: approle, kubernetes",
                config.auth_method
            );
        }
    }

    // Validate mount path is not empty
    if config.mount_path.is_empty() {
        anyhow::bail!("mount_path cannot be empty");
    }

    Ok(())
}

/// Authenticate with Vault using AppRole method.
///
/// # Arguments
/// * `client` - HTTP client for Vault API calls
/// * `config` - Base configuration with auth credentials
///
/// # Returns
/// A VaultToken on success
pub async fn authenticate_approle(
    client: &Client,
    config: &HashiCorpVaultBaseConfig,
) -> anyhow::Result<VaultToken> {
    let role_id = config
        .approle_role_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("approle_role_id not configured"))?;
    let secret_id = config
        .approle_secret_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("approle_secret_id not configured"))?;

    let url = format!("{}/v1/auth/approle/login", config.address);

    let request = AppRoleLoginRequest {
        role_id: role_id.clone(),
        secret_id: secret_id.clone(),
    };

    let mut builder = client.post(&url).json(&request);

    if let Some(ref namespace) = config.namespace {
        builder = builder.header("X-Vault-Namespace", namespace);
    }

    let response = builder.send().await.map_err(|e| {
        tracing::error!(error = %e, url = %url, "Failed to connect to Vault for AppRole login");
        anyhow::anyhow!("Failed to connect to Vault: {}", e)
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = %error_body, "AppRole login failed");
        anyhow::bail!("AppRole login failed: HTTP {} - {}", status, error_body);
    }

    let login_response: AppRoleLoginResponse = response.json().await.map_err(|e| {
        anyhow::anyhow!("Failed to parse AppRole login response: {}", e)
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    tracing::info!(
        role_id = %role_id,
        lease_duration = login_response.auth.lease_duration,
        "Successfully authenticated with Vault using AppRole"
    );

    Ok(VaultToken {
        token: login_response.auth.client_token,
        expires_at: now + login_response.auth.lease_duration,
    })
}

/// Authenticate with Vault using Kubernetes method.
///
/// # Arguments
/// * `client` - HTTP client for Vault API calls
/// * `config` - Base configuration with auth credentials
///
/// # Returns
/// A VaultToken on success
pub async fn authenticate_kubernetes(
    client: &Client,
    config: &HashiCorpVaultBaseConfig,
) -> anyhow::Result<VaultToken> {
    let role = config
        .k8s_role
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("k8s_role not configured"))?;

    // Read JWT token from Kubernetes service account
    let jwt_path = config
        .k8s_jwt_path
        .as_deref()
        .unwrap_or("/var/run/secrets/kubernetes.io/serviceaccount/token");

    let jwt = tokio::fs::read_to_string(jwt_path).await.map_err(|e| {
        tracing::error!(path = %jwt_path, error = %e, "Failed to read Kubernetes JWT token");
        anyhow::anyhow!("Failed to read Kubernetes JWT from {}: {}", jwt_path, e)
    })?;

    let url = format!("{}/v1/auth/kubernetes/login", config.address);

    let request = KubernetesLoginRequest {
        role: role.clone(),
        jwt: jwt.trim().to_string(),
    };

    let mut builder = client.post(&url).json(&request);

    if let Some(ref namespace) = config.namespace {
        builder = builder.header("X-Vault-Namespace", namespace);
    }

    let response = builder.send().await.map_err(|e| {
        tracing::error!(error = %e, url = %url, "Failed to connect to Vault for Kubernetes login");
        anyhow::anyhow!("Failed to connect to Vault: {}", e)
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = %error_body, "Kubernetes login failed");
        anyhow::bail!("Kubernetes login failed: HTTP {} - {}", status, error_body);
    }

    let login_response: KubernetesLoginResponse = response.json().await.map_err(|e| {
        anyhow::anyhow!("Failed to parse Kubernetes login response: {}", e)
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    tracing::info!(
        role = %role,
        lease_duration = login_response.auth.lease_duration,
        "Successfully authenticated with Vault using Kubernetes auth"
    );

    Ok(VaultToken {
        token: login_response.auth.client_token,
        expires_at: now + login_response.auth.lease_duration,
    })
}

/// Authenticate with Vault using the configured method.
///
/// # Arguments
/// * `client` - HTTP client for Vault API calls
/// * `config` - Base configuration with auth credentials
///
/// # Returns
/// A VaultToken on success
pub async fn authenticate(
    client: &Client,
    config: &HashiCorpVaultBaseConfig,
) -> anyhow::Result<VaultToken> {
    match config.auth_method.as_str() {
        "approle" => authenticate_approle(client, config).await,
        "kubernetes" => authenticate_kubernetes(client, config).await,
        _ => anyhow::bail!("Unsupported auth method: {}", config.auth_method),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_needs_refresh() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Token that expires in 10 minutes - does not need refresh
        let token = VaultToken {
            token: "test-token".to_string(),
            expires_at: now + 600,
        };
        assert!(!token.needs_refresh());

        // Token that expires in 2 minutes - needs refresh
        let token = VaultToken {
            token: "test-token".to_string(),
            expires_at: now + 120,
        };
        assert!(token.needs_refresh());
    }

    #[test]
    fn test_validate_base_config_approle() {
        let valid_config = HashiCorpVaultBaseConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "transit".to_string(),
            namespace: None,
            auth_method: "approle".to_string(),
            approle_role_id: Some("role-id".to_string()),
            approle_secret_id: Some("secret-id".to_string()),
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        };

        assert!(validate_base_config(&valid_config).is_ok());

        // Missing secret_id
        let invalid_config = HashiCorpVaultBaseConfig {
            approle_secret_id: None,
            ..valid_config.clone()
        };
        assert!(validate_base_config(&invalid_config).is_err());
    }

    #[test]
    fn test_validate_base_config_kubernetes() {
        let valid_config = HashiCorpVaultBaseConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "transit".to_string(),
            namespace: None,
            auth_method: "kubernetes".to_string(),
            approle_role_id: None,
            approle_secret_id: None,
            k8s_role: Some("trueflow-role".to_string()),
            k8s_jwt_path: Some("/var/run/secrets/kubernetes.io/serviceaccount/token".to_string()),
            skip_tls_verify: false,
        };

        assert!(validate_base_config(&valid_config).is_ok());

        // Missing k8s_role
        let invalid_config = HashiCorpVaultBaseConfig {
            k8s_role: None,
            ..valid_config.clone()
        };
        assert!(validate_base_config(&invalid_config).is_err());
    }

    #[test]
    fn test_validate_base_config_invalid_auth_method() {
        let config = HashiCorpVaultBaseConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "transit".to_string(),
            namespace: None,
            auth_method: "invalid".to_string(),
            approle_role_id: None,
            approle_secret_id: None,
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        };

        assert!(validate_base_config(&config).is_err());
    }

    #[test]
    fn test_vault_api_types_serde() {
        // Test AppRole login request serialization
        let req = AppRoleLoginRequest {
            role_id: "role-123".to_string(),
            secret_id: "secret-456".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("role_id"));
        assert!(json.contains("secret_id"));

        // Health response deserialization
        let json = r#"{"initialized":true,"sealed":false,"standby":false}"#;
        let health: HealthResponse = serde_json::from_str(json).unwrap();
        assert!(health.initialized);
        assert!(!health.sealed);
        assert!(!health.standby);
    }
}