pub mod builtin;
pub mod mock;

#[cfg(feature = "aws-kms")]
pub mod aws_kms;

#[cfg(feature = "hashicorp-vault")]
pub mod hashicorp_common;

#[cfg(feature = "hashicorp-vault")]
pub mod hashicorp;

#[cfg(feature = "hashicorp-vault")]
pub mod hashicorp_kv;

#[cfg(feature = "aws-secrets-manager")]
pub mod aws_secrets_manager;

#[cfg(feature = "azure-key-vault")]
pub mod azure_key_vault;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Supported vault backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VaultBackend {
    /// Built-in AES-256-GCM envelope encryption in PostgreSQL.
    Builtin,
    /// AWS Key Management Service.
    AwsKms,
    /// AWS Secrets Manager - runtime fetch from customer's secret store.
    AwsSecretsManager,
    /// HashiCorp Vault Transit secrets engine.
    HashicorpVault,
    /// HashiCorp Vault KV secrets engine (future).
    HashicorpVaultKv,
    /// Azure Key Vault (future).
    AzureKeyVault,
}

impl Default for VaultBackend {
    fn default() -> Self {
        Self::Builtin
    }
}

/// Database row for looking up credential vault backend
#[derive(FromRow)]
struct CredentialBackendRow {
    vault_backend: VaultBackend,
}

impl std::fmt::Display for VaultBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::AwsKms => write!(f, "aws_kms"),
            Self::AwsSecretsManager => write!(f, "aws_secrets_manager"),
            Self::HashicorpVault => write!(f, "hashicorp_vault"),
            Self::HashicorpVaultKv => write!(f, "hashicorp_vault_kv"),
            Self::AzureKeyVault => write!(f, "azure_key_vault"),
        }
    }
}

impl std::str::FromStr for VaultBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "builtin" => Ok(Self::Builtin),
            "aws_kms" | "awskms" | "aws-kms" => Ok(Self::AwsKms),
            "aws_secrets_manager" | "awssecretsmanager" | "aws-secrets-manager" => {
                Ok(Self::AwsSecretsManager)
            }
            "hashicorp_vault" | "hashicorpvault" | "hashicorp-vault" | "hcp_vault" => {
                Ok(Self::HashicorpVault)
            }
            "hashicorp_vault_kv" | "hashicorpvaultkv" | "hashicorp-vault-kv" | "hcp_vault_kv" => {
                Ok(Self::HashicorpVaultKv)
            }
            "azure_key_vault" | "azurekeyvault" | "azure-key-vault" => {
                Ok(Self::AzureKeyVault)
            }
            _ => Err(format!("Unknown vault backend: {}", s)),
        }
    }
}

/// Status of a vault backend.
#[derive(Debug, Clone, Serialize)]
pub struct BackendStatus {
    pub backend: VaultBackend,
    pub healthy: bool,
    pub is_default: bool,
    pub error: Option<String>,
}

/// Decrypted secret with metadata.
#[derive(Debug, Clone)]
pub struct DecryptedSecret {
    pub plaintext: String,
    pub provider: String,
    pub injection_mode: String,
    pub injection_header: String,
}

/// Abstraction over secret storage backends.
/// Implementations: BuiltinStore, AwsKmsStore, AwsSecretsManagerStore, HashiCorpVaultStore.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Returns the backend type for this store.
    fn backend(&self) -> VaultBackend;

    /// Encrypt and store a secret. Returns the storage ID.
    #[allow(dead_code)]
    async fn store(&self, plaintext: &str) -> anyhow::Result<String>;

    /// Retrieve and decrypt a secret by its storage ID.
    /// Returns (plaintext_secret, provider, injection_mode, injection_header).
    async fn retrieve(&self, id: &str) -> anyhow::Result<(String, String, String, String)>;

    /// Delete a stored secret. Requires project_id for authorization.
    #[allow(dead_code)]
    async fn delete(&self, id: &str, project_id: Uuid) -> anyhow::Result<()>;

    /// Check if the backend is healthy and accessible.
    async fn health_check(&self) -> anyhow::Result<()>;

    /// Return self as Any for downcasting (needed for builtin store access).
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Registry for vault backends with factory pattern.
/// Supports per-credential vault_backend selection with default fallback.
pub struct VaultRegistry {
    backends: HashMap<VaultBackend, Arc<dyn SecretStore>>,
    default_backend: VaultBackend,
    /// Reference to builtin store for direct access to encrypt_string
    builtin_store: builtin::BuiltinStore,
}

impl VaultRegistry {
    /// Create a new registry with the given backends.
    pub fn new(
        backends: HashMap<VaultBackend, Arc<dyn SecretStore>>,
        default_backend: VaultBackend,
    ) -> anyhow::Result<Self> {
        if !backends.contains_key(&default_backend) {
            anyhow::bail!(
                "Default backend {:?} not found in backends map",
                default_backend
            );
        }

        // Extract builtin store for direct access
        let builtin_store = backends
            .get(&VaultBackend::Builtin)
            .and_then(|s| {
                // Try to downcast to BuiltinStore - this is safe because we know
                // the Builtin variant is always a BuiltinStore
                s.as_any().downcast_ref::<builtin::BuiltinStore>().cloned()
            })
            .ok_or_else(|| anyhow::anyhow!("Builtin vault backend not found"))?;

        Ok(Self {
            backends,
            default_backend,
            builtin_store,
        })
    }

    /// Create a registry with only the builtin backend.
    pub fn builtin_only(vault: builtin::BuiltinStore) -> Self {
        let mut backends = HashMap::new();
        let vault_clone = vault.clone();
        backends.insert(
            VaultBackend::Builtin,
            Arc::new(vault_clone) as Arc<dyn SecretStore>,
        );

        Self {
            backends,
            default_backend: VaultBackend::Builtin,
            builtin_store: vault,
        }
    }

    /// Get a vault backend by type.
    pub fn get(&self, backend: VaultBackend) -> anyhow::Result<Arc<dyn SecretStore>> {
        self.backends
            .get(&backend)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Vault backend {:?} not configured", backend))
    }

    /// Get the default vault backend.
    pub fn default(&self) -> Arc<dyn SecretStore> {
        self.backends
            .get(&self.default_backend)
            .cloned()
            .expect("Default backend must exist")
    }

    /// Get the default backend type.
    pub fn default_backend(&self) -> VaultBackend {
        self.default_backend
    }

    /// Check if a backend is available.
    pub fn has(&self, backend: VaultBackend) -> bool {
        self.backends.contains_key(&backend)
    }

    /// List all available backends with their health status.
    pub async fn list_backends(&self) -> Vec<BackendStatus> {
        let mut statuses = Vec::new();

        for (backend, store) in &self.backends {
            // Call health_check once and use the result for both healthy and error
            let result = store.health_check().await;
            let healthy = result.is_ok();
            let error = result.err().map(|e| e.to_string());

            statuses.push(BackendStatus {
                backend: *backend,
                healthy,
                is_default: *backend == self.default_backend,
                error,
            });
        }

        statuses
    }

    /// Retrieve a secret using the appropriate backend.
    /// If backend_hint is provided, use that; otherwise use default.
    pub async fn retrieve(
        &self,
        id: &str,
        backend_hint: Option<VaultBackend>,
    ) -> anyhow::Result<(String, String, String, String)> {
        let backend = backend_hint.unwrap_or(self.default_backend);
        let store = self.get(backend)?;
        store.retrieve(id).await
    }

    /// Retrieve a credential by ID, automatically determining the vault backend.
    /// This queries the credentials table to get the vault_backend for the credential,
    /// then routes to the appropriate backend.
    ///
    /// This is the primary method for proxy handler credential resolution.
    pub async fn retrieve_credential(
        &self,
        pool: &sqlx::PgPool,
        id: &str,
    ) -> anyhow::Result<(String, String, String, String)> {
        // Query the credential to get its vault_backend
        let row = sqlx::query_as::<_, CredentialBackendRow>(
            "SELECT vault_backend FROM credentials WHERE id = $1 AND is_active = true",
        )
        .bind(uuid::Uuid::parse_str(id)?)
        .fetch_optional(pool)
        .await?;

        let backend = match row {
            Some(r) => r.vault_backend,
            None => anyhow::bail!("Credential not found: {}", id),
        };

        // Route to the appropriate store
        let store = self.get(backend)?;
        store.retrieve(id).await
    }

    /// Encrypt a plaintext string using the builtin vault.
    /// This is used for creating new credentials with the builtin backend.
    /// Returns (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce).
    pub fn encrypt_string(
        &self,
        plaintext: &str,
    ) -> anyhow::Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
        self.builtin_store.encrypt_string(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_backend_display() {
        assert_eq!(VaultBackend::Builtin.to_string(), "builtin");
        assert_eq!(VaultBackend::AwsKms.to_string(), "aws_kms");
        assert_eq!(
            VaultBackend::AwsSecretsManager.to_string(),
            "aws_secrets_manager"
        );
        assert_eq!(VaultBackend::HashicorpVault.to_string(), "hashicorp_vault");
        assert_eq!(VaultBackend::HashicorpVaultKv.to_string(), "hashicorp_vault_kv");
        assert_eq!(VaultBackend::AzureKeyVault.to_string(), "azure_key_vault");
    }

    #[test]
    fn test_backend_from_str() {
        assert_eq!(
            VaultBackend::from_str("builtin").unwrap(),
            VaultBackend::Builtin
        );
        assert_eq!(
            VaultBackend::from_str("aws_kms").unwrap(),
            VaultBackend::AwsKms
        );
        assert_eq!(
            VaultBackend::from_str("AWS-KMS").unwrap(),
            VaultBackend::AwsKms
        );
        assert_eq!(
            VaultBackend::from_str("aws_secrets_manager").unwrap(),
            VaultBackend::AwsSecretsManager
        );
        assert_eq!(
            VaultBackend::from_str("AWS-SECRETS-MANAGER").unwrap(),
            VaultBackend::AwsSecretsManager
        );
        assert_eq!(
            VaultBackend::from_str("hashicorp_vault").unwrap(),
            VaultBackend::HashicorpVault
        );
        assert_eq!(
            VaultBackend::from_str("hashicorp_vault_kv").unwrap(),
            VaultBackend::HashicorpVaultKv
        );
        assert_eq!(
            VaultBackend::from_str("azure_key_vault").unwrap(),
            VaultBackend::AzureKeyVault
        );
        assert!(VaultBackend::from_str("unknown").is_err());
    }
}
