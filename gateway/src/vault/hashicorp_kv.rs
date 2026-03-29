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
use tokio::sync::RwLock;
use uuid::Uuid;

use super::hashicorp_common::{
    authenticate, build_vault_client, validate_base_config, HashiCorpVaultBaseConfig,
    HealthResponse, VaultErrorResponse, VaultToken,
};
use super::{SecretStore, VaultBackend};

/// HashiCorp Vault KV configuration for runtime secret fetch.
///
/// Note: Debug is intentionally implemented to redact sensitive fields.
#[derive(Clone, Deserialize, Serialize)]
pub struct HashiCorpVaultKvConfig {
    /// Base configuration fields (shared with other HashiCorp backends)
    #[serde(flatten)]
    pub base: HashiCorpVaultBaseConfig,
}

impl std::fmt::Debug for HashiCorpVaultKvConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashiCorpVaultKvConfig")
            .field("address", &self.base.address)
            .field("mount_path", &self.base.mount_path)
            .field("namespace", &self.base.namespace)
            .field("auth_method", &self.base.auth_method)
            .field("approle_role_id", &self.base.approle_role_id)
            .field("approle_secret_id", &"[REDACTED]")
            .field("k8s_role", &self.base.k8s_role)
            .field("k8s_jwt_path", &self.base.k8s_jwt_path)
            .field("skip_tls_verify", &self.base.skip_tls_verify)
            .finish()
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
// KV-specific API Types
// ============================================================================

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

        // Build HTTP client
        let client = build_vault_client(config.base.skip_tls_verify)?;

        let store = Self {
            config,
            pool,
            client,
            token: Arc::new(RwLock::new(None)),
        };

        // Initial authentication to validate credentials
        store.authenticate_and_cache().await?;

        tracing::info!(
            address = %store.config.base.address,
            mount_path = %store.config.base.mount_path,
            auth_method = %store.config.base.auth_method,
            "HashiCorp Vault KV store initialized successfully"
        );

        Ok(store)
    }

    /// Validate the configuration.
    fn validate_config(config: &HashiCorpVaultKvConfig) -> anyhow::Result<()> {
        validate_base_config(&config.base)
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
        self.authenticate_and_cache().await
    }

    /// Authenticate with Vault and cache the token.
    async fn authenticate_and_cache(&self) -> anyhow::Result<String> {
        let token = authenticate(&self.client, &self.config.base).await?;

        // Cache the token
        {
            let mut token_guard = self.token.write().await;
            *token_guard = Some(token.clone());
        }

        Ok(token.token)
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
            self.config.base.address, self.config.base.mount_path, secret_path
        );

        let mut builder = self.client.get(&url).header("X-Vault-Token", &token);

        if let Some(ref namespace) = self.config.base.namespace {
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
        let url = format!("{}/v1/sys/health", self.config.base.address);

        // Use standbyok=true and sealedcode=200 to get a 200 response even if Vault is in standby
        let url_with_params = format!(
            "{}?standbyok=true&sealedcode=200&uninitcode=200",
            url
        );

        let mut builder = self.client.get(&url_with_params);

        if let Some(ref namespace) = self.config.base.namespace {
            builder = builder.header("X-Vault-Namespace", namespace);
        }

        let response = builder.send().await.map_err(|e| {
            tracing::error!(
                address = %self.config.base.address,
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
            address = %self.config.base.address,
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

    fn create_test_base_config() -> HashiCorpVaultBaseConfig {
        HashiCorpVaultBaseConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            namespace: Some("admin/trueflow".to_string()),
            auth_method: "approle".to_string(),
            approle_role_id: Some("role-uuid".to_string()),
            approle_secret_id: Some("secret-uuid".to_string()),
            k8s_role: None,
            k8s_jwt_path: None,
            skip_tls_verify: false,
        }
    }

    #[test]
    fn test_config_serialization() {
        let config = HashiCorpVaultKvConfig {
            base: create_test_base_config(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: HashiCorpVaultKvConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.base.address, parsed.base.address);
        assert_eq!(config.base.mount_path, parsed.base.mount_path);
    }

    #[test]
    fn test_debug_redacts_secret() {
        let config = HashiCorpVaultKvConfig {
            base: HashiCorpVaultBaseConfig {
                approle_secret_id: Some("super-secret-id".to_string()),
                ..create_test_base_config()
            },
        };

        let debug_output = format!("{:?}", config);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super-secret-id"));
    }

    #[test]
    fn test_validate_config_approle() {
        let valid_config = HashiCorpVaultKvConfig {
            base: HashiCorpVaultBaseConfig {
                approle_role_id: Some("role-id".to_string()),
                approle_secret_id: Some("secret-id".to_string()),
                ..create_test_base_config()
            },
        };

        assert!(HashiCorpVaultKvStore::validate_config(&valid_config).is_ok());

        // Missing secret_id
        let invalid_config = HashiCorpVaultKvConfig {
            base: HashiCorpVaultBaseConfig {
                approle_secret_id: None,
                ..valid_config.base.clone()
            },
        };
        assert!(HashiCorpVaultKvStore::validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_validate_config_kubernetes() {
        let valid_config = HashiCorpVaultKvConfig {
            base: HashiCorpVaultBaseConfig {
                auth_method: "kubernetes".to_string(),
                approle_role_id: None,
                approle_secret_id: None,
                k8s_role: Some("trueflow-role".to_string()),
                k8s_jwt_path: Some(
                    "/var/run/secrets/kubernetes.io/serviceaccount/token".to_string(),
                ),
                ..create_test_base_config()
            },
        };

        assert!(HashiCorpVaultKvStore::validate_config(&valid_config).is_ok());

        // Missing k8s_role
        let invalid_config = HashiCorpVaultKvConfig {
            base: HashiCorpVaultBaseConfig {
                k8s_role: None,
                ..valid_config.base.clone()
            },
        };
        assert!(HashiCorpVaultKvStore::validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_validate_config_invalid_auth_method() {
        let config = HashiCorpVaultKvConfig {
            base: HashiCorpVaultBaseConfig {
                auth_method: "invalid".to_string(),
                approle_role_id: None,
                approle_secret_id: None,
                k8s_role: None,
                k8s_jwt_path: None,
                ..create_test_base_config()
            },
        };

        assert!(HashiCorpVaultKvStore::validate_config(&config).is_err());
    }

    #[test]
    fn test_parse_external_ref() {
        // Standard format
        let (path, key) =
            HashiCorpVaultKvStore::parse_external_ref("prod/api-keys:openai_key").unwrap();
        assert_eq!(path, "prod/api-keys");
        assert_eq!(key, "openai_key");

        // Nested path
        let (path, key) =
            HashiCorpVaultKvStore::parse_external_ref("team/secrets/prod:api_key").unwrap();
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
        assert_eq!(
            resp.data
                .data
                .get("api_key")
                .unwrap()
                .as_str()
                .unwrap(),
            "sk-test-123"
        );
        assert_eq!(resp.data.metadata.version, 1);
    }
}