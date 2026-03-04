//! PII Tokenization Vault — reversible PII handling for enterprise compliance.
//!
//! Replaces detected PII with deterministic vault-backed tokens
//! (`tok_pii_{type}_{hash}`) that authorized callers can re-hydrate.
//!
//! Design:
//! - Tokens are deterministic per (project_id, pii_type, plaintext) triple
//!   so the same CC number always maps to the same token within a project.
//! - Original values are AES-256-GCM envelope-encrypted in PostgreSQL.
//! - Re-hydration requires `pii:rehydrate` API scope (PCI-DSS).
//! - Tokens auto-expire after 90 days (configurable via migration).

#![allow(dead_code)]
use crate::vault::builtin::VaultCrypto;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// A compiled PII pattern with its metadata.
pub struct PiiPattern {
    pub name: String,
    pub regex: Regex,
}

/// Result of a tokenization pass over a JSON value.
#[derive(Debug, Default)]
pub struct TokenizeResult {
    /// PII type names that were matched and tokenized.
    pub matched_types: Vec<String>,
    /// Number of individual values tokenized.
    pub tokens_created: usize,
}

/// Generate a deterministic PII token for a given value.
///
/// Format: `tok_pii_{type}_{first16_of_sha256}`
/// Deterministic: same (project_id, type, value) → same token.
pub fn generate_token(project_id: Uuid, pii_type: &str, plaintext: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_bytes());
    hasher.update(pii_type.as_bytes());
    hasher.update(plaintext.as_bytes());
    let hash = hex::encode(hasher.finalize());
    format!("tok_pii_{}_{}", pii_type, &hash[..16])
}

/// A PII match found during JSON tree walking.
struct PiiMatch {
    /// JSON Pointer path (e.g., "/messages/0/content")
    path: String,
    /// The matched PII substring
    matched_value: String,
    /// Pattern name (e.g., "credit_card")
    pattern_name: String,
    /// Generated token
    token: String,
}

/// Tokenize PII in a JSON value tree, replacing matches with vault-backed tokens.
///
/// Two-phase approach (avoids async recursion):
/// 1. Walk the JSON tree synchronously, collect all PII matches
/// 2. Store tokens in PG asynchronously, then replace values in the JSON
pub async fn tokenize_in_value(
    v: &mut Value,
    patterns: &[PiiPattern],
    project_id: Uuid,
    audit_log_id: Option<Uuid>,
    pool: &PgPool,
    vault: &VaultCrypto,
) -> TokenizeResult {
    let mut result = TokenizeResult::default();

    // Phase 1: Collect all PII matches (synchronous)
    let mut pii_matches: Vec<PiiMatch> = Vec::new();
    collect_pii_matches(v, patterns, project_id, "", &mut pii_matches);

    if pii_matches.is_empty() {
        return result;
    }

    // Phase 2: Store each token in the vault (async)
    let mut successful_tokens: Vec<(String, String, String)> = Vec::new(); // (matched_value, token, pattern_name)

    for m in &pii_matches {
        match store_token(pool, vault, &m.token, &m.pattern_name, &m.matched_value, project_id, audit_log_id).await {
            Ok(()) => {
                successful_tokens.push((m.matched_value.clone(), m.token.clone(), m.pattern_name.clone()));
            }
            Err(e) => {
                tracing::warn!(
                    pii_type = %m.pattern_name,
                    "PII vault store failed, skipping tokenization for this match: {}", e
                );
            }
        }
    }

    // Phase 3: Replace matched values with tokens in the JSON (synchronous)
    for (matched_value, token, pattern_name) in &successful_tokens {
        replace_in_value(v, matched_value, token);
        result.tokens_created += 1;
        if !result.matched_types.contains(pattern_name) {
            result.matched_types.push(pattern_name.clone());
        }
    }

    result
}

/// Synchronously walk a JSON tree and collect all PII matches.
fn collect_pii_matches(
    v: &Value,
    patterns: &[PiiPattern],
    project_id: Uuid,
    path: &str,
    matches: &mut Vec<PiiMatch>,
) {
    match v {
        Value::String(s) => {
            for pat in patterns {
                for m in pat.regex.find_iter(s) {
                    let matched = m.as_str().to_string();
                    let token = generate_token(project_id, &pat.name, &matched);
                    matches.push(PiiMatch {
                        path: path.to_string(),
                        matched_value: matched,
                        pattern_name: pat.name.clone(),
                        token,
                    });
                }
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{}/{}", path, i);
                collect_pii_matches(item, patterns, project_id, &child_path, matches);
            }
        }
        Value::Object(obj) => {
            for (key, val) in obj {
                let child_path = format!("{}/{}", path, key);
                collect_pii_matches(val, patterns, project_id, &child_path, matches);
            }
        }
        _ => {}
    }
}

/// Replace all occurrences of `target` with `replacement` in all string values.
fn replace_in_value(v: &mut Value, target: &str, replacement: &str) {
    match v {
        Value::String(s) => {
            if s.contains(target) {
                *s = s.replace(target, replacement);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                replace_in_value(item, target, replacement);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                replace_in_value(val, target, replacement);
            }
        }
        _ => {}
    }
}

/// Encrypt and store a PII token → value mapping using full envelope encryption.
async fn store_token(
    pool: &PgPool,
    vault: &VaultCrypto,
    token: &str,
    pii_type: &str,
    plaintext: &str,
    project_id: Uuid,
    audit_log_id: Option<Uuid>,
) -> anyhow::Result<()> {
    // Envelope encryption: generates fresh DEK, encrypts plaintext, encrypts DEK with master key.
    // Returns (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce).
    let (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce) =
        vault.encrypt_string(plaintext)?;

    // Pack all 4 envelope parts into a single JSONB blob for storage.
    // This keeps the schema simple (single encrypted_value + nonce column)
    // while preserving full envelope encryption.
    let envelope = serde_json::json!({
        "dek": hex::encode(&encrypted_dek),
        "dek_nonce": hex::encode(&dek_nonce),
        "secret": hex::encode(&encrypted_secret),
        "secret_nonce": hex::encode(&secret_nonce),
    });
    let envelope_bytes = serde_json::to_vec(&envelope)?;

    // Upsert: if this exact token already exists (deterministic), skip
    sqlx::query(
        r#"
        INSERT INTO pii_token_vault (
            token, pii_type, encrypted_value, nonce,
            project_id, audit_log_id
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (token) DO NOTHING
        "#,
    )
    .bind(token)
    .bind(pii_type)
    .bind(&envelope_bytes)
    .bind([0u8; 12]) // nonce is embedded in envelope, placeholder for schema compat
    .bind(project_id)
    .bind(audit_log_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Look up a PII token and decrypt the original value.
///
/// Returns `Ok(Some(plaintext))` if found and decrypted.
/// Returns `Ok(None)` if the token doesn't exist or has expired.
/// Returns `Err` if decryption fails.
pub async fn rehydrate_token(
    pool: &PgPool,
    vault: &VaultCrypto,
    token: &str,
    project_id: Uuid,
) -> anyhow::Result<Option<String>> {
    let row = sqlx::query_as::<_, (Vec<u8>,)>(
        r#"
        SELECT encrypted_value
        FROM pii_token_vault
        WHERE token = $1
          AND project_id = $2
          AND expires_at > NOW()
        "#,
    )
    .bind(token)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let (envelope_bytes,) = match row {
        Some(r) => r,
        None => return Ok(None),
    };

    // Unpack the envelope
    let envelope: serde_json::Value = serde_json::from_slice(&envelope_bytes)?;

    let encrypted_dek = hex::decode(
        envelope["dek"].as_str().ok_or_else(|| anyhow::anyhow!("missing dek"))?
    )?;
    let dek_nonce = hex::decode(
        envelope["dek_nonce"].as_str().ok_or_else(|| anyhow::anyhow!("missing dek_nonce"))?
    )?;
    let encrypted_secret = hex::decode(
        envelope["secret"].as_str().ok_or_else(|| anyhow::anyhow!("missing secret"))?
    )?;
    let secret_nonce = hex::decode(
        envelope["secret_nonce"].as_str().ok_or_else(|| anyhow::anyhow!("missing secret_nonce"))?
    )?;

    let plaintext = vault.decrypt_string(
        &encrypted_dek,
        &dek_nonce,
        &encrypted_secret,
        &secret_nonce,
    )?;

    Ok(Some(plaintext))
}

/// Batch re-hydrate multiple tokens.
pub async fn rehydrate_tokens(
    pool: &PgPool,
    vault: &VaultCrypto,
    tokens: &[String],
    project_id: Uuid,
) -> anyhow::Result<std::collections::HashMap<String, String>> {
    let mut results = std::collections::HashMap::new();

    for token in tokens {
        match rehydrate_token(pool, vault, token, project_id).await {
            Ok(Some(value)) => {
                results.insert(token.clone(), value);
            }
            Ok(None) => {
                // Token not found or expired — skip
                tracing::debug!(token = %token, "PII token not found or expired");
            }
            Err(e) => {
                tracing::error!(token = %token, "PII token rehydration failed: {}", e);
            }
        }
    }

    Ok(results)
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_deterministic() {
        let project_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let t1 = generate_token(project_id, "credit_card", "4111111111111111");
        let t2 = generate_token(project_id, "credit_card", "4111111111111111");
        assert_eq!(t1, t2, "Same input must produce same token");
        assert!(t1.starts_with("tok_pii_credit_card_"));
    }

    #[test]
    fn test_generate_token_different_values() {
        let project_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let t1 = generate_token(project_id, "credit_card", "4111111111111111");
        let t2 = generate_token(project_id, "credit_card", "5500000000000004");
        assert_ne!(t1, t2, "Different values must produce different tokens");
    }

    #[test]
    fn test_generate_token_different_projects() {
        let p1 = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let p2 = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let t1 = generate_token(p1, "ssn", "123-45-6789");
        let t2 = generate_token(p2, "ssn", "123-45-6789");
        assert_ne!(t1, t2, "Same value in different projects must produce different tokens");
    }

    #[test]
    fn test_token_format() {
        let project_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let token = generate_token(project_id, "email", "test@example.com");
        assert!(token.starts_with("tok_pii_email_"));
        let suffix = token.strip_prefix("tok_pii_email_").unwrap();
        assert_eq!(suffix.len(), 16);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_collect_pii_matches_nested_json() {
        let v = serde_json::json!({
            "user": {
                "email": "alice@example.com",
                "name": "Alice"
            },
            "notes": ["Contact bob@test.org for details"]
        });

        let email_re = regex::Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap();
        let patterns = vec![PiiPattern { name: "email".to_string(), regex: email_re }];
        let project_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        let mut matches = Vec::new();
        collect_pii_matches(&v, &patterns, project_id, "", &mut matches);

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|m| m.matched_value == "alice@example.com"));
        assert!(matches.iter().any(|m| m.matched_value == "bob@test.org"));
        assert!(matches.iter().all(|m| m.token.starts_with("tok_pii_email_")));
    }

    #[test]
    fn test_replace_in_value() {
        let mut v = serde_json::json!({
            "msg": "Call me at alice@example.com",
            "list": ["alice@example.com is here"]
        });

        replace_in_value(&mut v, "alice@example.com", "tok_pii_email_abc123");

        assert_eq!(v["msg"], "Call me at tok_pii_email_abc123");
        assert_eq!(v["list"][0], "tok_pii_email_abc123 is here");
    }
}
