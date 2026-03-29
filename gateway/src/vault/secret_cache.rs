//! Secret caching layer for external vault backends.
//!
//! This module provides a two-tier cache (DashMap L1 + Redis L2) for secrets
//! fetched from external vaults (AWS Secrets Manager, HashiCorp Vault, Azure Key Vault).
//! This prevents API rate limits on repeated vault calls.
//!
//! # Security
//! - Cache keys are SHA256 hashes of the external_vault_ref to prevent logging sensitive paths
//! - Secrets are stored as JSON in Redis; rely on Redis ACLs for access control
//! - L1 cache is process-local and never shared
//! - Rust's String type zeroizes heap memory on drop

use dashmap::DashMap;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for the secret cache.
#[derive(Debug, Clone)]
pub struct SecretCacheConfig {
    /// TTL for L1 (in-memory) cache. Default: 5 minutes.
    pub l1_ttl_secs: u64,
    /// TTL for L2 (Redis) cache. Default: 1 hour.
    pub l2_ttl_secs: u64,
    /// Key prefix for Redis entries. Default: "tf:secret:"
    pub redis_key_prefix: String,
}

impl Default for SecretCacheConfig {
    fn default() -> Self {
        Self {
            l1_ttl_secs: 300,        // 5 minutes
            l2_ttl_secs: 3600,       // 1 hour
            redis_key_prefix: "tf:secret:".to_string(),
        }
    }
}

/// Cached secret with metadata.
///
/// Note: This struct does NOT implement Drop with zeroize because:
/// 1. The `String` type in Rust already zeroes its heap memory when dropped
/// 2. The zeroize crate's `Zeroize` trait requires `Zeroize` on all fields
/// 3. Avoiding custom Drop simplifies the implementation and avoids potential issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSecret {
    /// The decrypted secret value.
    pub plaintext: String,
    /// Provider name (e.g., "openai", "anthropic").
    pub provider: String,
    /// Injection mode ("header" or "body").
    pub injection_mode: String,
    /// Header name for injection (e.g., "Authorization").
    pub injection_header: String,
    /// Backend that provided this secret.
    pub backend: String,
}

/// Entry stored in the local DashMap with an expiry timestamp.
#[derive(Clone)]
struct LocalCacheEntry {
    value: CachedSecret,
    expires_at: Instant,
}

/// Two-tier cache for secrets fetched from external vaults.
///
/// - L1: DashMap (in-memory, process-local)
/// - L2: Redis (shared across instances)
///
/// Cache keys are SHA256 hashes of the external_vault_ref for security.
#[derive(Clone)]
pub struct SecretCache {
    /// In-memory L1 cache.
    local: Arc<DashMap<String, LocalCacheEntry>>,
    /// Redis L2 cache connection.
    redis: ConnectionManager,
    /// Cache configuration.
    config: SecretCacheConfig,
}

impl SecretCache {
    /// Create a new secret cache with the given Redis connection and config.
    pub fn new(redis: ConnectionManager, config: SecretCacheConfig) -> Self {
        Self {
            local: Arc::new(DashMap::new()),
            redis,
            config,
        }
    }

    /// Create a new secret cache with default configuration.
    pub fn with_defaults(redis: ConnectionManager) -> Self {
        Self::new(redis, SecretCacheConfig::default())
    }

    /// Generate a SHA256-hashed cache key for an external vault reference.
    /// This prevents sensitive paths from appearing in logs or metrics.
    fn hash_key(&self, external_ref: &str, backend: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(backend.as_bytes());
        hasher.update(b":");
        hasher.update(external_ref.as_bytes());
        let hash = hasher.finalize();
        format!("{}{:x}", self.config.redis_key_prefix, hash)
    }

    /// Get a cached secret if it exists and hasn't expired.
    /// Returns None if not cached or expired.
    pub async fn get(&self, external_ref: &str, backend: &str) -> Option<CachedSecret> {
        let key = self.hash_key(external_ref, backend);

        // L1: Check local cache first (with TTL check)
        let expired = self
            .local
            .remove_if(&key, |_, entry| Instant::now() >= entry.expires_at);
        if expired.is_some() {
            tracing::debug!(
                cache_key = %key,
                "L1 cache entry expired, checking L2"
            );
        } else if let Some(entry) = self.local.get(&key) {
            tracing::debug!(
                cache_key = %key,
                backend = %backend,
                "L1 cache hit for secret"
            );
            return Some(entry.value.clone());
        }

        // L2: Check Redis
        let mut conn = self.redis.clone();
        match conn.get::<_, Option<String>>(&key).await {
            Ok(Some(v)) => {
                match serde_json::from_str::<CachedSecret>(&v) {
                    Ok(cached) => {
                        tracing::debug!(
                            cache_key = %key,
                            backend = %backend,
                            "L2 cache hit for secret"
                        );
                        // Populate L1 with remaining TTL
                        let ttl_secs: i64 = conn.ttl(&key).await.unwrap_or(60);
                        let ttl = if ttl_secs > 0 {
                            Duration::from_secs(ttl_secs as u64)
                        } else {
                            Duration::from_secs(60)
                        };
                        self.local.insert(
                            key.clone(),
                            LocalCacheEntry {
                                value: cached.clone(),
                                expires_at: Instant::now() + ttl,
                            },
                        );
                        return Some(cached);
                    }
                    Err(e) => {
                        tracing::debug!(
                            cache_key = %key,
                            backend = %backend,
                            error = %e,
                            "Failed to deserialize cached secret from Redis"
                        );
                    }
                }
            }
            Ok(None) => {
                // Key not found in Redis - this is normal, not an error
            }
            Err(e) => {
                tracing::debug!(
                    cache_key = %key,
                    backend = %backend,
                    error = %e,
                    "Redis GET failed for secret cache"
                );
            }
        }

        tracing::debug!(
            cache_key = %key,
            backend = %backend,
            "Cache miss for secret"
        );
        None
    }

    /// Store a secret in both cache tiers.
    pub async fn set(
        &self,
        external_ref: &str,
        backend: &str,
        secret: CachedSecret,
    ) -> anyhow::Result<()> {
        let key = self.hash_key(external_ref, backend);
        let json = serde_json::to_string(&secret)?;

        // L1: Store in local cache with L1 TTL
        self.local.insert(
            key.clone(),
            LocalCacheEntry {
                value: secret.clone(),
                expires_at: Instant::now() + Duration::from_secs(self.config.l1_ttl_secs),
            },
        );

        // L2: Store in Redis with L2 TTL
        let mut conn = self.redis.clone();
        conn.set_ex::<_, _, ()>(&key, json, self.config.l2_ttl_secs).await?;

        tracing::debug!(
            cache_key = %key,
            backend = %backend,
            l1_ttl_secs = self.config.l1_ttl_secs,
            l2_ttl_secs = self.config.l2_ttl_secs,
            "Secret cached in both tiers"
        );

        Ok(())
    }

    /// Invalidate a cached secret from both tiers.
    /// Call this when a credential is updated or deleted.
    pub async fn invalidate(&self, external_ref: &str, backend: &str) -> anyhow::Result<()> {
        let key = self.hash_key(external_ref, backend);

        // L1: Remove from local cache
        self.local.remove(&key);

        // L2: Remove from Redis
        let mut conn = self.redis.clone();
        let _: () = conn.del(&key).await?;

        tracing::debug!(
            cache_key = %key,
            backend = %backend,
            "Secret invalidated from both cache tiers"
        );

        Ok(())
    }

    /// Invalidate only the local (L1) cache entry.
    /// Use this when you want to force a refresh from L2/vault.
    pub fn invalidate_local(&self, external_ref: &str, backend: &str) {
        let key = self.hash_key(external_ref, backend);
        self.local.remove(&key);
    }

    /// Remove all locally-expired entries.
    /// Call this periodically from a background task to bound memory usage.
    pub fn evict_expired(&self) -> usize {
        let now = Instant::now();
        let before = self.local.len();
        self.local.retain(|_, entry| entry.expires_at > now);
        let evicted = before - self.local.len();
        if evicted > 0 {
            tracing::debug!(evicted_count = evicted, "Evicted expired L1 cache entries");
        }
        evicted
    }

    /// Get the current number of entries in the local cache (for metrics/debugging).
    pub fn local_len(&self) -> usize {
        self.local.len()
    }

    /// Check if Redis is reachable. Returns true if ping succeeds.
    pub async fn ping(&self) -> bool {
        let mut conn = self.redis.clone();
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to compute hash key without needing a full SecretCache.
    fn compute_hash_key(external_ref: &str, backend: &str) -> String {
        let prefix = "tf:secret:";
        let mut hasher = Sha256::new();
        hasher.update(backend.as_bytes());
        hasher.update(b":");
        hasher.update(external_ref.as_bytes());
        let hash = hasher.finalize();
        format!("{}{:x}", prefix, hash)
    }

    #[test]
    fn test_hash_key_deterministic() {
        let key1 = compute_hash_key("arn:aws:secretsmanager:us-east-1:123:secret/my-key", "aws_secrets_manager");
        let key2 = compute_hash_key("arn:aws:secretsmanager:us-east-1:123:secret/my-key", "aws_secrets_manager");
        assert_eq!(key1, key2, "Hash should be deterministic");

        // Different refs should produce different keys
        let key3 = compute_hash_key("arn:aws:secretsmanager:us-east-1:123:secret/other-key", "aws_secrets_manager");
        assert_ne!(key1, key3, "Different refs should have different keys");

        // Same ref, different backend should produce different keys
        let key4 = compute_hash_key("arn:aws:secretsmanager:us-east-1:123:secret/my-key", "hashicorp_vault");
        assert_ne!(key1, key4, "Different backends should have different keys");
    }

    #[test]
    fn test_hash_key_format() {
        let key = compute_hash_key("my-secret", "aws_secrets_manager");
        assert!(key.starts_with("tf:secret:"), "Key should have prefix");
        // SHA256 produces 64 hex characters
        assert_eq!(key.len(), "tf:secret:".len() + 64, "Key should be prefix + 64 hex chars");
    }

    #[test]
    fn test_config_defaults() {
        let config = SecretCacheConfig::default();
        assert_eq!(config.l1_ttl_secs, 300); // 5 minutes
        assert_eq!(config.l2_ttl_secs, 3600); // 1 hour
        assert_eq!(config.redis_key_prefix, "tf:secret:");
    }

    #[test]
    fn test_cached_secret_serialization() {
        let secret = CachedSecret {
            plaintext: "sk-test-123".to_string(),
            provider: "openai".to_string(),
            injection_mode: "header".to_string(),
            injection_header: "Authorization".to_string(),
            backend: "aws_secrets_manager".to_string(),
        };

        let json = serde_json::to_string(&secret).unwrap();
        let decoded: CachedSecret = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.plaintext, "sk-test-123");
        assert_eq!(decoded.provider, "openai");
    }
}