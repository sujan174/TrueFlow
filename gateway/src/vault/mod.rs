pub mod builtin;
pub mod mock;

#[cfg(feature = "aws-kms")]
pub mod aws_kms;

#[cfg(feature = "hashicorp-vault")]
pub mod hashicorp;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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
    /// HashiCorp Vault Transit secrets engine.
    HashicorpVault,
}

impl Default for VaultBackend {
    fn default() -> Self {
        Self::Builtin
    }
}

impl std::fmt::Display for VaultBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::AwsKms => write!(f, "aws_kms"),
            Self::HashicorpVault => write!(f, "hashicorp_vault"),
        }
    }
}

impl std::str::FromStr for VaultBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "builtin" => Ok(Self::Builtin),
            "aws_kms" | "awskms" | "aws-kms" => Ok(Self::AwsKms),
            "hashicorp_vault" | "hashicorpvault" | "hashicorp-vault" | "hcp_vault" => Ok(Self::HashicorpVault),
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
/// Implementations: BuiltinStore, AwsKmsStore, HashiCorpVaultStore.
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
}

/// Registry for vault backends with factory pattern.
/// Supports per-credential vault_backend selection with default fallback.
pub struct VaultRegistry {
    backends: HashMap<VaultBackend, Arc<dyn SecretStore>>,
    default_backend: VaultBackend,
}

impl VaultRegistry {
    /// Create a new registry with the given backends.
    pub fn new(
        backends: HashMap<VaultBackend, Arc<dyn SecretStore>>,
        default_backend: VaultBackend,
    ) -> anyhow::Result<Self> {
        if !backends.contains_key(&default_backend) {
            anyhow::bail!("Default backend {:?} not found in backends map", default_backend);
        }

        Ok(Self {
            backends,
            default_backend,
        })
    }

    /// Create a registry with only the builtin backend.
    pub fn builtin_only(vault: builtin::BuiltinStore) -> Self {
        let mut backends = HashMap::new();
        backends.insert(VaultBackend::Builtin, Arc::new(vault) as Arc<dyn SecretStore>);

        Self {
            backends,
            default_backend: VaultBackend::Builtin,
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
            let healthy = store.health_check().await.is_ok();
            let error = if !healthy {
                store.health_check().await.err().map(|e| e.to_string())
            } else {
                None
            };

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_backend_display() {
        assert_eq!(VaultBackend::Builtin.to_string(), "builtin");
        assert_eq!(VaultBackend::AwsKms.to_string(), "aws_kms");
        assert_eq!(VaultBackend::HashicorpVault.to_string(), "hashicorp_vault");
    }

    #[test]
    fn test_backend_from_str() {
        assert_eq!(VaultBackend::from_str("builtin").unwrap(), VaultBackend::Builtin);
        assert_eq!(VaultBackend::from_str("aws_kms").unwrap(), VaultBackend::AwsKms);
        assert_eq!(VaultBackend::from_str("AWS-KMS").unwrap(), VaultBackend::AwsKms);
        assert_eq!(VaultBackend::from_str("hashicorp_vault").unwrap(), VaultBackend::HashicorpVault);
        assert!(VaultBackend::from_str("unknown").is_err());
    }
}