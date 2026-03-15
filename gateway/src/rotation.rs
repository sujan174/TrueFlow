#![allow(dead_code)]
use crate::cache::TieredCache;
use crate::store::postgres::PgStore;
use crate::vault::builtin::VaultCrypto;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
// HIGH-12: Use Zeroizing wrapper for automatic zeroization on drop
use zeroize::Zeroizing;

/// Handles automatic rotation of upstream API keys.
/// Runs as a background task, checking credentials with rotation enabled.
///
/// The scheduler:
/// 1. Queries credentials where `rotation_enabled = true` and rotation is overdue
/// 2. Decrypts the current secret with the master key
/// 3. Re-encrypts with a fresh DEK (envelope rotation — same secret, new encryption layer)
/// 4. Updates the credential in PG with new encryption + bumped version
/// 5. Logs the rotation to `rotation_log` for audit
/// 6. Invalidates the credential cache so the next proxy request picks up the new version
///
/// NOTE: This performs *envelope key rotation* (re-encrypts with fresh DEK).
/// Full API key rotation (requesting a new key from the upstream provider) requires
/// provider-specific integration, which can be added as future work.
pub struct RotationScheduler {
    db: PgStore,
    vault: VaultCrypto,
    cache: TieredCache,
    interval_secs: u64,
}

/// A credential row that is due for rotation.
#[derive(Debug)]
struct RotationCandidate {
    id: Uuid,
    project_id: Uuid,
    name: String,
    provider: String,
    encrypted_dek: Vec<u8>,
    dek_nonce: Vec<u8>,
    encrypted_secret: Vec<u8>,
    secret_nonce: Vec<u8>,
    version: i32,
    rotation_interval: Option<String>,
}

impl RotationScheduler {
    /// Create a new scheduler.
    ///
    /// `check_interval_secs`: how often (seconds) the scheduler checks for due credentials.
    /// Default: 3600 (1 hour). Set via `TRUEFLOW_ROTATION_CHECK_INTERVAL` env var.
    pub fn new(db: PgStore, vault: VaultCrypto, cache: TieredCache, interval_secs: u64) -> Self {
        Self {
            db,
            vault,
            cache,
            interval_secs,
        }
    }

    /// Spawn the background rotation task.
    /// Runs forever, checking for due credentials every `interval_secs`.
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            tracing::info!(
                check_interval_secs = self.interval_secs,
                "Key rotation scheduler started"
            );

            loop {
                match self.run_rotation_cycle().await {
                    Ok(count) => {
                        if count > 0 {
                            tracing::info!(rotated = count, "Rotation cycle complete");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Rotation cycle failed: {}", e);
                    }
                }

                tokio::time::sleep(std::time::Duration::from_secs(self.interval_secs)).await;
            }
        });
    }

    /// Run one rotation cycle: find due credentials and rotate each.
    async fn run_rotation_cycle(&self) -> anyhow::Result<usize> {
        let candidates = self.find_due_credentials().await?;
        let mut rotated = 0;

        for cred in candidates {
            match self.rotate_credential(&cred).await {
                Ok(()) => {
                    tracing::info!(
                        credential_id = %cred.id,
                        name = %cred.name,
                        provider = %cred.provider,
                        old_version = cred.version,
                        new_version = cred.version + 1,
                        "Credential rotated successfully"
                    );
                    rotated += 1;
                }
                Err(e) => {
                    tracing::error!(
                        credential_id = %cred.id,
                        name = %cred.name,
                        "Credential rotation failed: {}",
                        e
                    );
                    // Log the failure
                    let _ = self
                        .log_rotation(
                            cred.id,
                            cred.version,
                            cred.version,
                            &cred.provider,
                            "failed",
                            Some(&e.to_string()),
                        )
                        .await;
                }
            }
        }

        Ok(rotated)
    }

    /// Query credentials that are due for rotation.
    ///
    /// A credential is due when:
    /// - `rotation_enabled = true`
    /// - `is_active = true`
    /// - `last_rotated_at` is NULL (never rotated) OR
    ///   `last_rotated_at + rotation_interval < NOW()`
    async fn find_due_credentials(&self) -> anyhow::Result<Vec<RotationCandidate>> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, name, provider,
                   encrypted_dek, dek_nonce, encrypted_secret, secret_nonce,
                   version, rotation_interval
            FROM credentials
            WHERE rotation_enabled = true
              AND is_active = true
              AND (
                  last_rotated_at IS NULL
                  OR last_rotated_at + (rotation_interval || ' seconds')::INTERVAL < NOW()
              )
            ORDER BY last_rotated_at ASC NULLS FIRST
            LIMIT 50
            "#,
        )
        .fetch_all(self.db.pool())
        .await?;

        let mut candidates = Vec::with_capacity(rows.len());
        for row in rows {
            candidates.push(RotationCandidate {
                id: row.get("id"),
                project_id: row.get("project_id"),
                name: row.get("name"),
                provider: row.get("provider"),
                encrypted_dek: row.get("encrypted_dek"),
                dek_nonce: row.get("dek_nonce"),
                encrypted_secret: row.get("encrypted_secret"),
                secret_nonce: row.get("secret_nonce"),
                version: row.get("version"),
                rotation_interval: row.get("rotation_interval"),
            });
        }

        Ok(candidates)
    }

    /// Rotate a single credential:
    /// 1. Decrypt the current secret
    /// 2. Re-encrypt with a fresh DEK (envelope rotation)
    /// 3. Update the DB atomically
    /// 4. Invalidate cache
    /// 5. Log the rotation
    async fn rotate_credential(&self, cred: &RotationCandidate) -> anyhow::Result<()> {
        // Step 1: Decrypt current secret
        // HIGH-12: Wrap in Zeroizing for automatic zeroization on drop
        let plaintext_secret = Zeroizing::new(self.vault.decrypt_string(
            &cred.encrypted_dek,
            &cred.dek_nonce,
            &cred.encrypted_secret,
            &cred.secret_nonce,
        )?);

        // Step 2: Re-encrypt with fresh DEK
        let (new_encrypted_dek, new_dek_nonce, new_encrypted_secret, new_secret_nonce) =
            self.vault.encrypt_string(&plaintext_secret)?;

        // HIGH-12: plaintext_secret is automatically zeroized when it goes out of scope
        // No need for manual zeroize() call - Zeroizing<String> handles it on drop

        let new_version = cred.version + 1;

        // HIGH-10: Invalidate cache BEFORE DB update to avoid race where
        // concurrent requests read stale cached data after the update.
        // This ensures the cache miss happens before the new data is written.
        let cache_key = format!("credential:{}", cred.id);
        if let Err(e) = self.cache.invalidate(&cache_key).await {
            tracing::warn!(
                credential_id = %cred.id,
                error = %e,
                "HIGH-10: Failed to invalidate Redis cache before rotation - continuing with local invalidation only"
            );
            self.cache.invalidate_local(&cache_key);
        }

        // Step 3: Atomic DB update — version check prevents concurrent rotation
        let result = sqlx::query(
            r#"
            UPDATE credentials
            SET encrypted_dek = $1,
                dek_nonce = $2,
                encrypted_secret = $3,
                secret_nonce = $4,
                version = $5,
                last_rotated_at = NOW(),
                updated_at = NOW()
            WHERE id = $6 AND version = $7
            "#,
        )
        .bind(&new_encrypted_dek)
        .bind(&new_dek_nonce)
        .bind(&new_encrypted_secret)
        .bind(&new_secret_nonce)
        .bind(new_version)
        .bind(cred.id)
        .bind(cred.version) // Optimistic concurrency: only update if version matches
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!(
                "Version conflict: credential {} was modified concurrently (expected v{})",
                cred.id,
                cred.version
            );
        }

        // Step 4: Log the rotation
        self.log_rotation(
            cred.id,
            cred.version,
            new_version,
            &cred.provider,
            "success",
            None,
        )
        .await?;

        Ok(())
    }

    /// Insert a row into the rotation_log table.
    async fn log_rotation(
        &self,
        credential_id: Uuid,
        old_version: i32,
        new_version: i32,
        provider: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO rotation_log (
                credential_id, old_version, new_version, provider, status, error_message
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(credential_id)
        .bind(old_version)
        .bind(new_version)
        .bind(provider)
        .bind(status)
        .bind(error_message)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }
}
