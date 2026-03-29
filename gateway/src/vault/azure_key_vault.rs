//! Azure Key Vault backed secret store for customer-managed keys.
//!
//! This module implements the `SecretStore` trait using Azure Key Vault.
//! Supports both service principal and managed identity authentication.
//!
//! # Authentication Methods
//!
//! 1. **Service Principal**: Use tenant_id, client_id, and client_secret
//! 2. **Managed Identity**: Set use_managed_identity = true (no credentials needed)
//!
//! # Secret Reference Format
//!
//! The `external_vault_ref` column stores the secret name, optionally with version:
//! - `my-secret` - Latest version
//! - `my-secret/versions/abc123` - Specific version

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use super::{SecretStore, VaultBackend};

/// Azure Key Vault configuration.
#[derive(Clone, Deserialize, Serialize)]
pub struct AzureKeyVaultConfig {
    /// Azure Key Vault URL (e.g., https://my-vault.vault.azure.net/)
    pub vault_url: String,
    /// Azure AD tenant ID for service principal authentication
    pub tenant_id: Option<String>,
    /// Azure AD client (application) ID for service principal authentication
    pub client_id: Option<String>,
    /// Azure AD client secret for service principal authentication
    pub client_secret: Option<String>,
    /// Use Azure Managed Identity instead of service principal
    pub use_managed_identity: bool,
}

impl std::fmt::Debug for AzureKeyVaultConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureKeyVaultConfig")
            .field("vault_url", &self.vault_url)
            .field("tenant_id", &self.tenant_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("use_managed_identity", &self.use_managed_identity)
            .finish()
    }
}

impl AzureKeyVaultConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate vault URL
        let parsed = Url::parse(&self.vault_url).map_err(|e| {
            anyhow::anyhow!("Invalid vault URL '{}': {}", self.vault_url, e)
        })?;

        if parsed.scheme() != "https" {
            return Err(anyhow::anyhow!(
                "Azure Key Vault URL must use HTTPS: {}",
                self.vault_url
            ));
        }

        // Validate authentication configuration
        if self.use_managed_identity {
            // Managed identity: no credentials needed
            if self.tenant_id.is_some() || self.client_id.is_some() || self.client_secret.is_some() {
                tracing::warn!(
                    "Azure Key Vault: Using managed identity, ignoring service principal credentials"
                );
            }
        } else {
            // Service principal: require all three credentials
            if self.tenant_id.is_none() || self.client_id.is_none() || self.client_secret.is_none() {
                return Err(anyhow::anyhow!(
                    "Service principal authentication requires tenant_id, client_id, and client_secret"
                ));
            }
        }

        Ok(())
    }

    /// Get the vault name from the URL.
    /// E.g., "https://my-vault.vault.azure.net/" -> "my-vault"
    pub fn vault_name(&self) -> Option<&str> {
        self.vault_url
            .trim_start_matches("https://")
            .split('.')
            .next()
    }
}

/// Azure Key Vault backed secret store.
///
/// This store fetches secrets at runtime from Azure Key Vault.
/// The secret name is stored per-credential in `external_vault_ref`.
pub struct AzureKeyVaultStore {
    config: AzureKeyVaultConfig,
    pool: PgPool,
    http_client: reqwest::Client,
    /// Cached access token for Azure AD
    access_token: tokio::sync::RwLock<Option<(String, std::time::Instant)>>,
}

impl AzureKeyVaultStore {
    /// Create a new Azure Key Vault store.
    pub fn new(config: AzureKeyVaultConfig, pool: PgPool) -> anyhow::Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            pool,
            http_client: reqwest::Client::new(),
            access_token: tokio::sync::RwLock::new(None),
        })
    }

    /// Get an Azure AD access token.
    async fn get_access_token(&self) -> anyhow::Result<String> {
        // Check if we have a cached token that's still valid (with 5 min buffer)
        {
            let token_guard = self.access_token.read().await;
            if let Some((token, expires_at)) = token_guard.as_ref() {
                let now = std::time::Instant::now();
                let buffer = std::time::Duration::from_secs(300); // 5 minutes
                if now + buffer < *expires_at {
                    return Ok(token.clone());
                }
            }
        }

        // Get a new token
        let token = if self.config.use_managed_identity {
            self.get_managed_identity_token().await?
        } else {
            self.get_service_principal_token().await?
        };

        Ok(token)
    }

    /// Get token using managed identity (IMDS endpoint).
    async fn get_managed_identity_token(&self) -> anyhow::Result<String> {
        // Azure IMDS endpoint for managed identity
        const IMDS_URL: &str = "http://169.254.169.254/metadata/identity/oauth2/token";

        let vault_name = self.config.vault_name().ok_or_else(|| {
            anyhow::anyhow!("Could not extract vault name from URL")
        })?;

        let resource = format!("https://{}.vault.azure.net", vault_name);

        let response = self
            .http_client
            .get(IMDS_URL)
            .query(&[
                ("api-version", "2018-02-01"),
                ("resource", &resource),
            ])
            .header("Metadata", "true")
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to call Azure IMDS endpoint");
                anyhow::anyhow!("Failed to get managed identity token: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Azure IMDS request failed with status {}: {}",
                status,
                body
            ));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            anyhow::anyhow!("Failed to parse Azure IMDS response: {}", e)
        })?;

        // Cache the token with expiry
        let expires_at = std::time::Instant::now()
            + std::time::Duration::from_secs(token_response.expires_in as u64);

        {
            let mut token_guard = self.access_token.write().await;
            *token_guard = Some((token_response.access_token.clone(), expires_at));
        }

        Ok(token_response.access_token)
    }

    /// Get token using service principal (client credentials flow).
    async fn get_service_principal_token(&self) -> anyhow::Result<String> {
        let tenant_id = self.config.tenant_id.as_ref().ok_or_else(|| {
            anyhow::anyhow!("tenant_id required for service principal authentication")
        })?;
        let client_id = self.config.client_id.as_ref().ok_or_else(|| {
            anyhow::anyhow!("client_id required for service principal authentication")
        })?;
        let client_secret = self.config.client_secret.as_ref().ok_or_else(|| {
            anyhow::anyhow!("client_secret required for service principal authentication")
        })?;

        let vault_name = self.config.vault_name().ok_or_else(|| {
            anyhow::anyhow!("Could not extract vault name from URL")
        })?;

        let resource = format!("https://{}.vault.azure.net/.default", vault_name);
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            tenant_id
        );

        let response = self
            .http_client
            .post(&token_url)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("scope", resource.as_str()),
                ("grant_type", "client_credentials"),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to get Azure AD token");
                anyhow::anyhow!("Failed to get Azure AD token: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Azure AD token request failed with status {}: {}",
                status,
                body
            ));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            anyhow::anyhow!("Failed to parse Azure AD token response: {}", e)
        })?;

        // Cache the token with expiry
        let expires_at = std::time::Instant::now()
            + std::time::Duration::from_secs(token_response.expires_in as u64);

        {
            let mut token_guard = self.access_token.write().await;
            *token_guard = Some((token_response.access_token.clone(), expires_at));
        }

        Ok(token_response.access_token)
    }

    /// Fetch a secret by name from Azure Key Vault.
    ///
    /// # Arguments
    /// * `secret_name` - The secret name, optionally with version (e.g., "my-secret" or "my-secret/versions/abc123")
    ///
    /// # Returns
    /// The secret value as a string.
    pub async fn fetch_secret(&self, secret_name: &str) -> anyhow::Result<String> {
        let access_token = self.get_access_token().await?;

        // Build the secret URL
        // Format: https://my-vault.vault.azure.net/secrets/{secret-name}?api-version=7.4
        // Or with version: https://my-vault.vault.azure.net/secrets/{secret-name}/versions/{version}?api-version=7.4
        let secret_url = format!(
            "{}secrets/{}?api-version=7.4",
            self.config.vault_url.trim_end_matches('/'),
            secret_name
        );

        let response = self
            .http_client
            .get(&secret_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| {
                tracing::error!(
                    secret_name = %secret_name,
                    error = %e,
                    "Failed to fetch secret from Azure Key Vault"
                );
                anyhow::anyhow!("Failed to fetch secret from Azure Key Vault: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 404 {
                return Err(anyhow::anyhow!(
                    "Secret '{}' not found in Azure Key Vault",
                    secret_name
                ));
            }

            return Err(anyhow::anyhow!(
                "Azure Key Vault request failed with status {}: {}",
                status,
                body
            ));
        }

        #[derive(Deserialize)]
        struct SecretResponse {
            value: String,
            #[allow(dead_code)]
            id: String,
            #[allow(dead_code)]
            attributes: Option<SecretAttributes>,
        }

        #[derive(Deserialize)]
        struct SecretAttributes {
            #[allow(dead_code)]
            enabled: Option<bool>,
            #[allow(dead_code)]
            expires: Option<i64>,
        }

        let secret_response: SecretResponse = response.json().await.map_err(|e| {
            anyhow::anyhow!("Failed to parse Azure Key Vault response: {}", e)
        })?;

        Ok(secret_response.value)
    }
}

#[async_trait]
impl SecretStore for AzureKeyVaultStore {
    fn backend(&self) -> VaultBackend {
        VaultBackend::AzureKeyVault
    }

    async fn store(&self, _plaintext: &str) -> anyhow::Result<String> {
        anyhow::bail!(
            "Azure Key Vault does not support storing via TrueFlow. \
             Store secrets directly in Azure Key Vault and reference by name."
        )
    }

    async fn retrieve(&self, id: &str) -> anyhow::Result<(String, String, String, String)> {
        let cred_id = uuid::Uuid::parse_str(id)?;

        // Fetch the secret name from database
        let row = sqlx::query_as::<_, (Option<String>, String, String, String)>(
            r#"SELECT external_vault_ref, provider, injection_mode, injection_header
               FROM credentials WHERE id = $1 AND is_active = true"#,
        )
        .bind(cred_id)
        .fetch_one(&self.pool)
        .await?;

        let secret_name = row.0.ok_or_else(|| {
            anyhow::anyhow!("Credential {} has no external_vault_ref (secret name)", id)
        })?;

        // Fetch secret from Azure Key Vault
        let plaintext = self.fetch_secret(&secret_name).await?;

        tracing::info!(
            credential_id = %cred_id,
            secret_name = %secret_name,
            vault_url = %self.config.vault_url,
            "Successfully fetched secret from Azure Key Vault"
        );

        Ok((plaintext, row.1, row.2, row.3))
    }

    async fn delete(&self, id: &str, project_id: Uuid) -> anyhow::Result<()> {
        // Just mark as inactive - secret remains in customer's Key Vault
        sqlx::query("UPDATE credentials SET is_active = false WHERE id = $1 AND project_id = $2")
            .bind(uuid::Uuid::parse_str(id)?)
            .bind(project_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // Try to get an access token as a health check
        self.get_access_token().await?;

        // Optionally, we could try to list secrets, but that requires additional permissions
        // Getting a token is sufficient to verify authentication is working

        tracing::info!(
            vault_url = %self.config.vault_url,
            use_managed_identity = self.config.use_managed_identity,
            "Azure Key Vault health check passed"
        );

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        // Valid managed identity config
        let config = AzureKeyVaultConfig {
            vault_url: "https://my-vault.vault.azure.net/".to_string(),
            tenant_id: None,
            client_id: None,
            client_secret: None,
            use_managed_identity: true,
        };
        assert!(config.validate().is_ok());

        // Valid service principal config
        let config = AzureKeyVaultConfig {
            vault_url: "https://my-vault.vault.azure.net/".to_string(),
            tenant_id: Some("tenant-123".to_string()),
            client_id: Some("client-456".to_string()),
            client_secret: Some("secret-789".to_string()),
            use_managed_identity: false,
        };
        assert!(config.validate().is_ok());

        // Invalid: non-HTTPS URL
        let config = AzureKeyVaultConfig {
            vault_url: "http://my-vault.vault.azure.net/".to_string(),
            tenant_id: None,
            client_id: None,
            client_secret: None,
            use_managed_identity: true,
        };
        assert!(config.validate().is_err());

        // Invalid: missing service principal credentials
        let config = AzureKeyVaultConfig {
            vault_url: "https://my-vault.vault.azure.net/".to_string(),
            tenant_id: Some("tenant-123".to_string()),
            client_id: None,
            client_secret: None,
            use_managed_identity: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_vault_name_extraction() {
        let config = AzureKeyVaultConfig {
            vault_url: "https://my-vault.vault.azure.net/".to_string(),
            tenant_id: None,
            client_id: None,
            client_secret: None,
            use_managed_identity: true,
        };
        assert_eq!(config.vault_name(), Some("my-vault"));

        let config = AzureKeyVaultConfig {
            vault_url: "https://prod-keys.vault.azure.net/".to_string(),
            tenant_id: None,
            client_id: None,
            client_secret: None,
            use_managed_identity: true,
        };
        assert_eq!(config.vault_name(), Some("prod-keys"));
    }

    #[test]
    fn test_debug_redacts_secret() {
        let config = AzureKeyVaultConfig {
            vault_url: "https://my-vault.vault.azure.net/".to_string(),
            tenant_id: Some("tenant-123".to_string()),
            client_id: Some("client-456".to_string()),
            client_secret: Some("super-secret-value".to_string()),
            use_managed_identity: false,
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("super-secret-value"));
    }
}