use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use async_trait::async_trait;
use rand::RngCore;
use sqlx::PgPool;

pub type EncryptedBlob = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

/// Built-in vault using AES-256-GCM envelope encryption in PostgreSQL.
pub struct BuiltinStore {
    crypto: VaultCrypto,
    pool: PgPool,
}

impl BuiltinStore {
    pub fn new(master_key_hex: &str, pool: PgPool) -> anyhow::Result<Self> {
        let crypto = VaultCrypto::new(master_key_hex)?;
        Ok(Self { crypto, pool })
    }

    /// Delegate to VaultCrypto for API handler use.
    pub fn encrypt_string(&self, plaintext: &str) -> anyhow::Result<EncryptedBlob> {
        self.crypto.encrypt_string(plaintext)
    }
}

pub struct VaultCrypto {
    kek: [u8; 32],
}

impl VaultCrypto {
    pub fn new(master_key_hex: &str) -> anyhow::Result<Self> {
        let kek = parse_master_key(master_key_hex)?;
        Ok(Self { kek })
    }

    /// Encrypts a plaintext string using envelope encryption.
    /// Returns (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce).
    pub fn encrypt_string(&self, plaintext: &str) -> anyhow::Result<EncryptedBlob> {
        // 1. Generate a random DEK
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);

        // 2. Encrypt the secret with the DEK
        let secret_cipher = Aes256Gcm::new_from_slice(&dek)
            .map_err(|e| anyhow::anyhow!("invalid key length: {:?}", e))?;
        let secret_nonce_bytes = generate_nonce();
        let secret_nonce = Nonce::from_slice(&secret_nonce_bytes);
        let encrypted_secret = secret_cipher
            .encrypt(secret_nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("secret encryption failed: {}", e))?;

        // 3. Encrypt the DEK with the master KEK
        let kek_cipher = Aes256Gcm::new_from_slice(&self.kek)
            .map_err(|e| anyhow::anyhow!("invalid key length: {:?}", e))?;
        let dek_nonce_bytes = generate_nonce();
        let dek_nonce = Nonce::from_slice(&dek_nonce_bytes);
        let encrypted_dek = kek_cipher
            .encrypt(dek_nonce, dek.as_ref())
            .map_err(|e| anyhow::anyhow!("DEK encryption failed: {}", e))?;

        // 4. Zero the plaintext DEK
        dek.fill(0);

        Ok((
            encrypted_dek,
            dek_nonce_bytes.to_vec(),
            encrypted_secret,
            secret_nonce_bytes.to_vec(),
        ))
    }

    /// Decrypts a secret using envelope decryption.
    pub fn decrypt_string(
        &self,
        encrypted_dek: &[u8],
        dek_nonce: &[u8],
        encrypted_secret: &[u8],
        secret_nonce: &[u8],
    ) -> anyhow::Result<String> {
        // 1. Decrypt DEK with master KEK
        let kek_cipher = Aes256Gcm::new_from_slice(&self.kek)
            .map_err(|e| anyhow::anyhow!("invalid key length: {:?}", e))?;
        let d_nonce = Nonce::from_slice(dek_nonce);
        let dek_bytes = kek_cipher
            .decrypt(d_nonce, encrypted_dek)
            .map_err(|e| anyhow::anyhow!("DEK decryption failed: {}", e))?;

        let mut dek = [0u8; 32];
        dek.copy_from_slice(&dek_bytes);

        // 2. Decrypt secret with DEK
        let secret_cipher = Aes256Gcm::new_from_slice(&dek)
            .map_err(|e| anyhow::anyhow!("invalid key length: {:?}", e))?;
        let s_nonce = Nonce::from_slice(secret_nonce);
        let plaintext_bytes = secret_cipher
            .decrypt(s_nonce, encrypted_secret)
            .map_err(|e| anyhow::anyhow!("secret decryption failed: {}", e))?;

        // Zero the DEK
        dek.fill(0);

        Ok(String::from_utf8(plaintext_bytes)?)
    }
}

#[async_trait]
impl super::SecretStore for BuiltinStore {
    async fn store(&self, plaintext: &str) -> anyhow::Result<String> {
        let (enc_dek, dek_nonce, enc_secret, secret_nonce) =
            self.crypto.encrypt_string(plaintext)?;

        let payload = serde_json::json!({
            "encrypted_dek": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &enc_dek),
            "dek_nonce": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &dek_nonce),
            "encrypted_secret": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &enc_secret),
            "secret_nonce": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &secret_nonce),
        });

        Ok(payload.to_string())
    }

    async fn retrieve(&self, id: &str) -> anyhow::Result<(String, String, String, String)> {
        let row = sqlx::query_as::<_, CredentialRow>(
            "SELECT encrypted_dek, dek_nonce, encrypted_secret, secret_nonce, provider, injection_mode, injection_header FROM credentials WHERE id = $1 AND is_active = true"
        )
        .bind(uuid::Uuid::parse_str(id)?)
        .fetch_one(&self.pool)
        .await?;

        let secret = self.crypto.decrypt_string(
            &row.encrypted_dek,
            &row.dek_nonce,
            &row.encrypted_secret,
            &row.secret_nonce,
        )?;

        Ok((
            secret,
            row.provider,
            row.injection_mode,
            row.injection_header,
        ))
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE credentials SET is_active = false WHERE id = $1")
            .bind(uuid::Uuid::parse_str(id)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct CredentialRow {
    encrypted_dek: Vec<u8>,
    dek_nonce: Vec<u8>,
    encrypted_secret: Vec<u8>,
    secret_nonce: Vec<u8>,
    provider: String,
    injection_mode: String,
    injection_header: String,
}

fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

pub fn parse_master_key(hex: &str) -> anyhow::Result<[u8; 32]> {
    if hex.len() != 64 {
        anyhow::bail!(
            "TRUEFLOW_MASTER_KEY must be 64 hex chars (32 bytes), got {} chars",
            hex.len()
        );
    }
    let bytes = hex::decode(hex)?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[test]
    fn test_encryption_roundtrip() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();

        let secret = "sk_live_123456789";
        let (enc_dek, dek_nonce, enc_secret, secret_nonce) = crypto.encrypt_string(secret).unwrap();

        let decrypted = crypto
            .decrypt_string(&enc_dek, &dek_nonce, &enc_secret, &secret_nonce)
            .unwrap();
        assert_eq!(decrypted, secret);
    }

    // ── Chaos: Tampered Ciphertext ──────────────────────────────

    /// Flipping one bit in the encrypted DEK must cause authenticated decryption to fail.
    #[test]
    fn test_tampered_encrypted_dek_rejected() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();
        let (mut enc_dek, dek_nonce, enc_secret, secret_nonce) =
            crypto.encrypt_string("sk-secret").unwrap();
        enc_dek[0] ^= 0x01;
        assert!(
            crypto.decrypt_string(&enc_dek, &dek_nonce, &enc_secret, &secret_nonce).is_err(),
            "Tampered DEK should fail AES-GCM authentication"
        );
    }

    /// Flipping one bit in the encrypted secret must cause decryption to fail.
    #[test]
    fn test_tampered_encrypted_secret_rejected() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();
        let (enc_dek, dek_nonce, mut enc_secret, secret_nonce) =
            crypto.encrypt_string("sk-secret").unwrap();
        enc_secret[0] ^= 0x01;
        assert!(
            crypto.decrypt_string(&enc_dek, &dek_nonce, &enc_secret, &secret_nonce).is_err(),
            "Tampered encrypted secret should fail"
        );
    }

    /// Flipping one bit in the DEK nonce must cause decryption to fail.
    #[test]
    fn test_tampered_dek_nonce_rejected() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();
        let (enc_dek, mut dek_nonce, enc_secret, secret_nonce) =
            crypto.encrypt_string("sk-secret").unwrap();
        dek_nonce[0] ^= 0x01;
        assert!(
            crypto.decrypt_string(&enc_dek, &dek_nonce, &enc_secret, &secret_nonce).is_err(),
            "Tampered DEK nonce should fail"
        );
    }

    /// Flipping one bit in the secret nonce must cause decryption to fail.
    #[test]
    fn test_tampered_secret_nonce_rejected() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();
        let (enc_dek, dek_nonce, enc_secret, mut secret_nonce) =
            crypto.encrypt_string("sk-secret").unwrap();
        secret_nonce[0] ^= 0x01;
        assert!(
            crypto.decrypt_string(&enc_dek, &dek_nonce, &enc_secret, &secret_nonce).is_err(),
            "Tampered secret nonce should fail"
        );
    }

    /// Wrong master key must fail — prevents cross-environment key confusion.
    #[test]
    fn test_wrong_master_key_rejected() {
        let crypto_a = VaultCrypto::new(TEST_KEY).unwrap();
        let crypto_b = VaultCrypto::new("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();
        let (enc_dek, dek_nonce, enc_secret, secret_nonce) =
            crypto_a.encrypt_string("sk-secret").unwrap();
        assert!(
            crypto_b.decrypt_string(&enc_dek, &dek_nonce, &enc_secret, &secret_nonce).is_err(),
            "Decrypting with wrong master key should fail"
        );
    }

    /// Truncated ciphertext should fail, not panic.
    #[test]
    fn test_truncated_ciphertext_rejected() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();
        let (enc_dek, dek_nonce, enc_secret, secret_nonce) =
            crypto.encrypt_string("sk-secret").unwrap();
        let truncated = &enc_secret[..2];
        assert!(
            crypto.decrypt_string(&enc_dek, &dek_nonce, truncated, &secret_nonce).is_err(),
            "Truncated ciphertext should fail"
        );
    }

    /// Empty ciphertext should fail cleanly.
    #[test]
    fn test_empty_ciphertext_rejected() {
        let crypto = VaultCrypto::new(TEST_KEY).unwrap();
        let (enc_dek, dek_nonce, _enc_secret, secret_nonce) =
            crypto.encrypt_string("sk-secret").unwrap();
        assert!(
            crypto.decrypt_string(&enc_dek, &dek_nonce, &[], &secret_nonce).is_err(),
            "Empty ciphertext should fail"
        );
    }

    /// Short master key (<64 hex chars) must be rejected at construction.
    #[test]
    fn test_short_master_key_rejected() {
        assert!(VaultCrypto::new("deadbeef").is_err());
    }

    /// Invalid hex in master key must be rejected at construction.
    #[test]
    fn test_invalid_hex_master_key_rejected() {
        assert!(VaultCrypto::new(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
        ).is_err());
    }
}
