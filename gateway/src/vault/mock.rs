//! Mock vault implementation for testing.
//!
//! Stores secrets in memory without encryption.
//! DO NOT USE IN PRODUCTION.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{SecretStore, VaultBackend};

/// In-memory mock vault for testing.
pub struct MockVault {
    store: RwLock<HashMap<String, StoredSecret>>,
}

#[derive(Clone)]
struct StoredSecret {
    plaintext: String,
    provider: String,
    injection_mode: String,
    injection_header: String,
}

impl MockVault {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }

    /// Create an Arc-wrapped mock vault.
    pub fn arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Pre-populate a secret for testing.
    pub async fn prepopulate(
        &self,
        id: &str,
        plaintext: &str,
        provider: &str,
        injection_mode: &str,
        injection_header: &str,
    ) {
        let mut store = self.store.write().await;
        store.insert(
            id.to_string(),
            StoredSecret {
                plaintext: plaintext.to_string(),
                provider: provider.to_string(),
                injection_mode: injection_mode.to_string(),
                injection_header: injection_header.to_string(),
            },
        );
    }
}

impl Default for MockVault {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretStore for MockVault {
    fn backend(&self) -> VaultBackend {
        VaultBackend::Builtin // Pretend to be builtin for tests
    }

    async fn store(&self, plaintext: &str) -> anyhow::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut store = self.store.write().await;
        store.insert(
            id.clone(),
            StoredSecret {
                plaintext: plaintext.to_string(),
                provider: "test".to_string(),
                injection_mode: "header".to_string(),
                injection_header: "Authorization".to_string(),
            },
        );
        Ok(id)
    }

    async fn retrieve(&self, id: &str) -> anyhow::Result<(String, String, String, String)> {
        let store = self.store.read().await;
        let secret = store
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Secret not found: {}", id))?;

        Ok((
            secret.plaintext.clone(),
            secret.provider.clone(),
            secret.injection_mode.clone(),
            secret.injection_header.clone(),
        ))
    }

    async fn delete(&self, id: &str, _project_id: Uuid) -> anyhow::Result<()> {
        let mut store = self.store.write().await;
        store.remove(id);
        Ok(())
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // Always healthy for mock
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_store_retrieve() {
        let vault = MockVault::new();

        let id = vault.store("my-secret-key").await.unwrap();
        let (plaintext, _, _, _) = vault.retrieve(&id).await.unwrap();

        assert_eq!(plaintext, "my-secret-key");
    }

    #[tokio::test]
    async fn test_mock_delete() {
        let vault = MockVault::new();

        let id = vault.store("secret").await.unwrap();
        vault.delete(&id, Uuid::nil()).await.unwrap();

        assert!(vault.retrieve(&id).await.is_err());
    }

    #[tokio::test]
    async fn test_prepopulate() {
        let vault = MockVault::new();
        vault
            .prepopulate("test-id", "test-key", "openai", "header", "Authorization")
            .await;

        let (plaintext, provider, _, _) = vault.retrieve("test-id").await.unwrap();
        assert_eq!(plaintext, "test-key");
        assert_eq!(provider, "openai");
    }
}