//! HashiCorp Vault KV v2 secrets engine backend.
//!
//! This module provides integration with HashiCorp Vault's KV v2 secrets engine
//! for customer-managed secrets stored at rest in Vault.
//!
//! # Security Model
//!
//! Unlike Transit (which stores pre-encrypted ciphertext), KV stores secrets at rest.
//! TrueFlow fetches plaintext secrets at runtime from Vault.
//!
//! # Secret Reference Format
//!
//! Secrets are referenced in `external_vault_ref` using the format:
//! `path/to/secret:key_name`
//!
//! For example: `prod/api-keys:openai_key`
//!
//! # Authentication Methods
//!
//! - **AppRole**: Recommended for machine-to-machine auth. Requires role_id and secret_id.
//! - **Kubernetes**: For pods running in Kubernetes. Uses JWT token from service account.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

use super::{SecretStore, VaultBackend};

/// Default timeout for Vault API calls.
const VAULT_TIMEOUT_SECS: u64 = 30;

/// Vault token refresh buffer (refresh 5 minutes before expiry).
const TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

/// HashiCorp Vault KV configuration for runtime secret fetch.
///
/// Note: Debug is intentionally implemented to redact sensitive fields.
#[derive(Clone, Deserialize, Serialize)]
pub struct HashiCorpVaultKvConfig {
    /// Vault server address (e.g., https://vault.example.com:8200)
    pub address: String,
    /// KV v2 secrets engine mount path (default: secret)
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

impl std::fmt::Debug for HashiCorpVaultKvConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashiCorpVaultKvConfig")
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

/// Cached Vault token with expiry information.
#[derive(Clone)]
struct VaultToken {
    /// The token string
    token: String,
    /// Token expiry time (Unix timestamp in seconds)
    expires_at: u64,
}

impl VaultToken {
    /// Check if the token needs refresh (within buffer period of expiry).
    fn needs_refresh(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now + TOKEN_REFRESH_BUFFER_SECS >= self.expires_at
    }
}

/// HashiCorp Vault KV v2 backed secret store.
///
/// This store fetches secrets at runtime from Vault's KV v2 secrets engine.
/// The secret reference is stored per-credential in `external_vault_ref`.
pub struct HashiCorpVaultKvStore {
    config: HashiCorpVaultKvConfig,
    pool: PgPool,
    /// HTTP client for Vault API calls
    client: Client,
    /// Cached authentication token
    token: Arc<RwLock<Option<VaultToken>>>,
}

// ============================================================================
// Vault API Types
// ============================================================================

/// AppRole login request.
#[derive(Serialize)]
struct AppRoleLoginRequest {
    role_id: String,
    secret_id: String,
}

/// AppRole login response.
#[derive(Deserialize)]
struct AppRoleLoginResponse {
    auth: VaultAuth,
}

/// Kubernetes login request.
#[derive(Serialize)]
struct KubernetesLoginRequest {
    role: String,
    jwt: String,
}

/// Kubernetes login response.
#[derive(Deserialize)]
struct KubernetesLoginResponse {
    auth: VaultAuth,
}

/// Common auth response fields.
#[derive(Deserialize)]
struct VaultAuth {
    client_token: String,
    lease_duration: u64,
}

/// KV v2 read response.
/// The response structure is: { data: { data: { key: value }, metadata: {...} } }
#[derive(Deserialize)]
struct KvV2ReadResponse {
    data: KvV2DataWrapper,
}

#[derive(Deserialize)]
struct KvV2DataWrapper {
    /// The actual secret data
    data: HashMap<String, serde_json::Value>,
    /// Metadata about the secret version
    #[allow(dead_code)]
    metadata: KvV2Metadata,
}

#[derive(Deserialize)]
struct KvV2Metadata {
    #[allow(dead_code)]
    created_time: String,
    #[allow(dead_code)]
    custom_metadata: Option<HashMap<String, String>>,
    #[allow(dead_code)]
    deleted: Option<bool>,
    #[allow(dead_code)]
    destroyed: Option<bool>,
    #[allow(dead_code)]
    version: i64,
}

/// Vault health response.
#[derive(Deserialize)]
struct HealthResponse {
    /// Whether Vault is initialized
    #[serde(default)]
    initialized: bool,
    /// Whether Vault is sealed
    #[serde(default)]
    sealed: bool,
    /// Whether Vault is in standby
    #[serde(default)]
    standby: bool,
}

/// Vault error response.
#[derive(Deserialize)]
struct VaultErrorResponse {
    errors: Vec<String>,
}

// ============================================================================
// Implementation
// ============================================================================

impl HashiCorpVaultKvStore {
    /// Create a new HashiCorp Vault KV store with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or if connection
    /// to Vault fails.
    pub async fn new(config: HashiCorpVaultKvConfig, pool: PgPool) -> anyhow::Result<Self> {
        // Validate configuration
        Self::validate_config(&config)?;

        // Build HTTP client with appropriate timeouts
        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(VAULT_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(10));

        if config.skip_tls_verify {
            client_builder = client_builder.danger_accept_invalid_certs(true);
            tracing::warn!(
                address = %config.address,
                "TLS verification disabled for Vault connection - not recommended for production"
            );
        }

        let client = client_builder.build()?;

        let store = Self {
            config,
            pool,
            client,
            token: Arc::new(RwLock::new(None)),
        };

        // Initial authentication to validate credentials
        store.authenticate().await?;

        tracing::info!(
            address = %store.config.address,
            mount_path = %store.config.mount_path,
            auth_method = %store.config.auth_method,
            "HashiCorp Vault KV store initialized successfully"
        );

        Ok(store)
    }

    /// Validate the configuration.
    fn validate_config(config: &HashiCorpVaultKvConfig) -> anyhow::Result<()> {
        // Validate address is a valid URL
        Url::parse(&config.address)
            .map_err(|e| anyhow::anyhow!("Invalid Vault address '{}': {}", config.address, e))?;

        // Validate auth method and required fields
        match config.auth_method.as_str() {
            "approle" => {
                if config.approle_role_id.is_none()
                    || config
                        .approle_role_id
                        .as_ref()
                        .map_or(true, |s| s.is_empty())
                {
                    anyhow::bail!("approle_role_id is required for AppRole authentication");
                }
                if config.approle_secret_id.is_none()
                    || config
                        .approle_secret_id
                        .as_ref()
                        .map_or(true, |s| s.is_empty())
                {
                    anyhow::bail!("approle_secret_id is required for AppRole authentication");
                }
            }
            "kubernetes" => {
                if config.k8s_role.is_none()
                    || config.k8s_role.as_ref().map_or(true, |s| s.is_empty())
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

    /// Get or refresh the authentication token.
    async fn get_token(&self) -> anyhow::Result<String> {
        // Check if we have a valid cached token
        {
            let token_guard = self.token.read().await;
            if let Some(ref token) = *token_guard {
                if !token.needs_refresh() {
                    return Ok(token.token.clone());
                }
            }
        }

        // Need to authenticate or refresh
        self.authenticate().await
    }

    /// Authenticate with Vault and cache the token.
    async fn authenticate(&self) -> anyhow::Result<String> {
        let token = match self.config.auth_method.as_str() {
            "approle" => self.authenticate_approle().await?,
            "kubernetes" => self.authenticate_kubernetes().await?,
            _ => anyhow::bail!("Unsupported auth method: {}", self.config.auth_method),
        };

        // Cache the token
        {
            let mut token_guard = self.token.write().await;
            *token_guard = Some(token.clone());
        }

        Ok(token.token)
    }

    /// Authenticate using AppRole method.
    async fn authenticate_approle(&self) -> anyhow::Result<VaultToken> {
        let role_id = self
            .config
            .approle_role_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("approle_role_id not configured"))?;
        let secret_id = self
            .config
            .approle_secret_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("approle_secret_id not configured"))?;

        let url = format!("{}/v1/auth/approle/login", self.config.address);

        let request = AppRoleLoginRequest {
            role_id: role_id.clone(),
            secret_id: secret_id.clone(),
        };

        let mut builder = self.client.post(&url).json(&request);

        if let Some(ref namespace) = self.config.namespace {
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

    /// Authenticate using Kubernetes method.
    async fn authenticate_kubernetes(&self) -> anyhow::Result<VaultToken> {
        let role = self
            .config
            .k8s_role
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("k8s_role not configured"))?;

        // Read JWT token from Kubernetes service account
        let jwt_path = self
            .config
            .k8s_jwt_path
            .as_deref()
            .unwrap_or("/var/run/secrets/kubernetes.io/serviceaccount/token");

        let jwt = tokio::fs::read_to_string(jwt_path).await.map_err(|e| {
            tracing::error!(path = %jwt_path, error = %e, "Failed to read Kubernetes JWT token");
            anyhow::anyhow!("Failed to read Kubernetes JWT from {}: {}", jwt_path, e)
        })?;

        let url = format!("{}/v1/auth/kubernetes/login", self.config.address);

        let request = KubernetesLoginRequest {
            role: role.clone(),
            jwt: jwt.trim().to_string(),
        };

        let mut builder = self.client.post(&url).json(&request);

        if let Some(ref namespace) = self.config.namespace {
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

    /// Fetch a secret from Vault KV v2.
    ///
    /// The secret_path is the path to the secret (without mount path).
    /// Returns the entire secret data as a HashMap.
    pub async fn read_secret(
        &self,
        secret_path: &str,
    ) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        let token = self.get_token().await?;

        // KV v2 API endpoint: /v1/{mount}/data/{path}
        let url = format!(
            "{}/v1/{}/data/{}",
            self.config.address, self.config.mount_path, secret_path
        );

        let mut builder = self.client.get(&url).header("X-Vault-Token", &token);

        if let Some(ref namespace) = self.config.namespace {
            builder = builder.header("X-Vault-Namespace", namespace);
        }

        let response = builder.send().await.map_err(|e| {
            tracing::error!(
                secret_path = %secret_path,
                error = %e,
                "Failed to call Vault KV read"
            );
            anyhow::anyhow!("Failed to call Vault KV read: {}", e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();

            // Try to parse Vault error response
            if let Ok(vault_error) = serde_json::from_str::<VaultErrorResponse>(&error_body) {
                tracing::error!(
                    secret_path = %secret_path,
                    status = %status,
                    errors = ?vault_error.errors,
                    "Vault KV read failed"
                );
                anyhow::bail!("Vault KV read failed: {}", vault_error.errors.join(", "));
            }

            anyhow::bail!("Vault KV read failed: HTTP {} - {}", status, error_body);
        }

        let read_response: KvV2ReadResponse = response.json().await.map_err(|e| {
            anyhow::anyhow!("Failed to parse Vault KV read response: {}", e)
        })?;

        Ok(read_response.data.data)
    }

    /// Parse the external_vault_ref to extract secret path and key name.
    ///
    /// The external_vault_ref format is: `path/to/secret:key_name`
    fn parse_external_ref(external_ref: &str) -> anyhow::Result<(String, String)> {
        // Find the last colon that separates path from key name
        // This allows paths like "prod/secrets:api_key"
        if let Some(colon_pos) = external_ref.rfind(':') {
            let secret_path = external_ref[..colon_pos].to_string();
            let key_name = external_ref[colon_pos + 1..].to_string();

            if secret_path.is_empty() {
                anyhow::bail!("Secret path cannot be empty in external_vault_ref");
            }
            if key_name.is_empty() {
                anyhow::bail!("Key name cannot be empty in external_vault_ref");
            }

            Ok((secret_path, key_name))
        } else {
            anyhow::bail!(
                "Invalid external_vault_ref format for KV. Expected 'path/to/secret:key_name', got '{}'",
                external_ref
            );
        }
    }
}

#[async_trait]
impl SecretStore for HashiCorpVaultKvStore {
    fn backend(&self) -> VaultBackend {
        VaultBackend::HashicorpVaultKv
    }

    /// Store is not supported for KV backend.
    ///
    /// Secrets should be stored directly in Vault using the Vault CLI or API.
    async fn store(&self, _plaintext: &str) -> anyhow::Result<String> {
        anyhow::bail!(
            "HashiCorp Vault KV does not support storing via TrueFlow. \
             Store secrets directly in Vault and reference with 'path/to/secret:key_name' format."
        )
    }

    /// Retrieve a secret from Vault KV v2.
    ///
    /// The ID is the credential UUID. We look up the external_vault_ref
    /// from the database and fetch the secret from Vault.
    async fn retrieve(&self, id: &str) -> anyhow::Result<(String, String, String, String)> {
        let cred_id = uuid::Uuid::parse_str(id)?;

        // Fetch the external_vault_ref and metadata from database
        let row = sqlx::query_as::<_, CredentialRow>(
            r#"SELECT
                external_vault_ref,
                provider,
                injection_mode,
                injection_header
               FROM credentials
               WHERE id = $1 AND is_active = true"#,
        )
        .bind(cred_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(
                credential_id = %cred_id,
                error = %e,
                "Failed to fetch credential for Vault KV retrieval"
            );
            anyhow::anyhow!("Credential not found: {}", e)
        })?;

        let external_ref = row
            .external_vault_ref
            .ok_or_else(|| anyhow::anyhow!("Credential {} has no external_vault_ref", id))?;

        // Parse secret path and key name
        let (secret_path, key_name) = Self::parse_external_ref(&external_ref)?;

        // Fetch secret from Vault KV v2
        let secret_data = self.read_secret(&secret_path).await?;

        // Extract the specific key from the secret
        let secret_value = secret_data
            .get(&key_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Key '{}' not found in secret at path '{}'",
                    key_name,
                    secret_path
                )
            })?
            .clone();

        // Convert value to string (handle both string and JSON values)
        let plaintext = match secret_value {
            serde_json::Value::String(s) => s,
            _ => serde_json::to_string(&secret_value)
                .map_err(|e| anyhow::anyhow!("Failed to serialize secret value: {}", e))?,
        };

        tracing::info!(
            credential_id = %cred_id,
            provider = %row.provider,
            secret_path = %secret_path,
            key_name = %key_name,
            "Successfully retrieved credential from HashiCorp Vault KV"
        );

        Ok((
            plaintext,
            row.provider,
            row.injection_mode,
            row.injection_header,
        ))
    }

    /// Delete is a no-op for KV vault (customer manages their secret).
    async fn delete(&self, id: &str, project_id: Uuid) -> anyhow::Result<()> {
        // Just mark as inactive in database - external vault data is managed by customer
        let result = sqlx::query(
            "UPDATE credentials SET is_active = false WHERE id = $1 AND project_id = $2",
        )
        .bind(uuid::Uuid::parse_str(id)?)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            tracing::warn!(
                credential_id = %id,
                project_id = %project_id,
                "Credential not found or already deleted"
            );
        }

        Ok(())
    }

    /// Check if Vault is healthy and accessible.
    async fn health_check(&self) -> anyhow::Result<()> {
        let url = format!("{}/v1/sys/health", self.config.address);

        // Use standbyok=true and sealedcode=200 to get a 200 response even if Vault is in standby
        let url_with_params = format!(
            "{}?standbyok=true&sealedcode=200&uninitcode=200",
            url
        );

        let mut builder = self.client.get(&url_with_params);

        if let Some(ref namespace) = self.config.namespace {
            builder = builder.header("X-Vault-Namespace", namespace);
        }

        let response = builder.send().await.map_err(|e| {
            tracing::error!(
                address = %self.config.address,
                error = %e,
                "Failed to connect to Vault for health check"
            );
            anyhow::anyhow!("Vault health check failed: cannot connect - {}", e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Vault health check failed: HTTP {}", status);
        }

        let health: HealthResponse = response.json().await.map_err(|e| {
            anyhow::anyhow!("Failed to parse Vault health response: {}", e)
        })?;

        if health.sealed {
            anyhow::bail!("Vault is sealed and cannot process requests");
        }

        if !health.initialized {
            anyhow::bail!("Vault is not initialized");
        }

        // Also verify authentication works
        self.get_token().await?;

        tracing::debug!(
            address = %self.config.address,
            initialized = health.initialized,
            sealed = health.sealed,
            standby = health.standby,
            "Vault health check passed"
        );

        Ok(())
    }

    /// Return self as Any for downcasting.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Database row for external vault credential lookup.
#[derive(sqlx::FromRow)]
struct CredentialRow {
    external_vault_ref: Option<String>,
    provider: String,
    injection_mode: String,
    injection_header: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = HashiCorpVaultKvConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            namespace: Some("admin/trueflow".to_string()),
            auth_method: "approle".to_string(),
            approle_role_id: Some("role-uuid".to_string()),
            approle_secret_id: Some("secret-uuid".to_string()),
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: HashiCorpVaultKvConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.address, parsed.address);
        assert_eq!(config.mount_path, parsed.mount_path);
    }

    #[test]
    fn test_debug_redacts_secret() {
        let config = HashiCorpVaultKvConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            namespace: None,
            auth_method: "approle".to_string(),
            approle_role_id: Some("role-uuid".to_string()),
            approle_secret_id: Some("super-secret-id".to_string()),
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        };

        let debug_output = format!("{:?}", config);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super-secret-id"));
    }

    #[test]
    fn test_validate_config_approle() {
        let valid_config = HashiCorpVaultKvConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            namespace: None,
            auth_method: "approle".to_string(),
            approle_role_id: Some("role-id".to_string()),
            approle_secret_id: Some("secret-id".to_string()),
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        };

        assert!(HashiCorpVaultKvStore::validate_config(&valid_config).is_ok());

        // Missing secret_id
        let invalid_config = HashiCorpVaultKvConfig {
            approle_secret_id: None,
            ..valid_config.clone()
        };
        assert!(HashiCorpVaultKvStore::validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_validate_config_kubernetes() {
        let valid_config = HashiCorpVaultKvConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            namespace: None,
            auth_method: "kubernetes".to_string(),
            approle_role_id: None,
            approle_secret_id: None,
            k8s_role: Some("trueflow-role".to_string()),
            k8s_jwt_path: Some(
                "/var/run/secrets/kubernetes.io/serviceaccount/token".to_string(),
            ),
            skip_tls_verify: false,
        };

        assert!(HashiCorpVaultKvStore::validate_config(&valid_config).is_ok());

        // Missing k8s_role
        let invalid_config = HashiCorpVaultKvConfig {
            k8s_role: None,
            ..valid_config.clone()
        };
        assert!(HashiCorpVaultKvStore::validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_validate_config_invalid_auth_method() {
        let config = HashiCorpVaultKvConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            namespace: None,
            auth_method: "invalid".to_string(),
            approle_role_id: None,
            approle_secret_id: None,
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        };

        assert!(HashiCorpVaultKvStore::validate_config(&config).is_err());
    }

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
    fn test_parse_external_ref() {
        // Standard format
        let (path, key) = HashiCorpVaultKvStore::parse_external_ref("prod/api-keys:openai_key").unwrap();
        assert_eq!(path, "prod/api-keys");
        assert_eq!(key, "openai_key");

        // Nested path
        let (path, key) = HashiCorpVaultKvStore::parse_external_ref("team/secrets/prod:api_key").unwrap();
        assert_eq!(path, "team/secrets/prod");
        assert_eq!(key, "api_key");

        // Simple path
        let (path, key) = HashiCorpVaultKvStore::parse_external_ref("my-secret:key").unwrap();
        assert_eq!(path, "my-secret");
        assert_eq!(key, "key");

        // Invalid - no colon
        assert!(HashiCorpVaultKvStore::parse_external_ref("no-colon-here").is_err());

        // Invalid - empty key
        assert!(HashiCorpVaultKvStore::parse_external_ref("path:").is_err());

        // Invalid - empty path
        assert!(HashiCorpVaultKvStore::parse_external_ref(":key").is_err());
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

        // Test KV v2 read response deserialization
        let json = r#"{
            "data": {
                "data": {
                    "api_key": "sk-test-123",
                    "other_value": "hello"
                },
                "metadata": {
                    "created_time": "2024-01-01T00:00:00Z",
                    "custom_metadata": null,
                    "version": 1
                }
            }
        }"#;
        let resp: KvV2ReadResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.data.get("api_key").unwrap().as_str().unwrap(), "sk-test-123");
        assert_eq!(resp.data.metadata.version, 1);

        // Health response deserialization
        let json = r#"{"initialized":true,"sealed":false,"standby":false}"#;
        let health: HealthResponse = serde_json::from_str(json).unwrap();
        assert!(health.initialized);
        assert!(!health.sealed);
        assert!(!health.standby);
    }
}