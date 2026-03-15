# Vault & Encryption Security Review

## Summary
Reviewed 6 files for encryption correctness, memory safety, PII detection issues, and credential management.

---

## CRITICAL FINDINGS

### 1. KEK Not Zeroized on Drop
**File:** `gateway/src/vault/builtin.rs:29-31`
**Severity:** Critical

**Description:** The `VaultCrypto` struct holds the master key encryption key (KEK) in a fixed-size array `[u8; 32]`, but does not implement `Zeroize` or `Drop` to clear this sensitive key from memory when the struct is dropped. While DEKs are properly zeroized after use (lines 66, 100, 111), the KEK remains in memory indefinitely.

**Impact:** Long-lived `VaultCrypto` instances keep the master key in memory. If the process crashes and creates a core dump, or if memory is swapped to disk, the master key could be exposed.

**Suggested Fix:**
```rust
use zeroize::Zeroize;

pub struct VaultCrypto {
    kek: [u8; 32],
}

impl Drop for VaultCrypto {
    fn drop(&mut self) {
        self.kek.zeroize();
    }
}
```

**Related Code:** Lines 64-66, 98-100, 110-111 (DEK zeroization is correct)

---

### 2. Race Condition: Cache Invalidated After DB Update
**File:** `gateway/src/rotation.rs:202-235`
**Severity:** High

**Description:** During credential rotation, the cache is invalidated AFTER the database update succeeds. Between the DB update (line 222) and cache invalidation (line 235), concurrent requests may:
1. Miss the local cache (already stale)
2. Fetch from Redis (still has old value)
3. Store old credential back in local cache

The sequence is:
1. Update DB with new encrypted credential (line 202-223)
2. Invalidate local cache only (line 234-235)
3. Redis cache is NEVER invalidated

**Impact:** Requests may use stale cached credentials for up to the TTL duration (default 60s), defeating the purpose of key rotation.

**Suggested Fix:**
```rust
// Step 3: Invalidate cache FIRST (both local and Redis)
self.cache.invalidate(&cache_key).await; // Need new method

// Step 4: Then update DB
let result = sqlx::query(...)
```

Also add Redis invalidation to `TieredCache`:
```rust
pub async fn invalidate(&self, key: &str) -> anyhow::Result<()> {
    self.local.remove(key);
    let mut conn = self.redis.clone();
    redis::cmd("DEL").arg(key).query_async(&mut conn).await?;
    Ok(())
}
```

**Related Code:** `gateway/src/cache.rs:104-106` (only invalidates local cache)

---

## HIGH FINDINGS

### 3. Plaintext Secret Exposed During Re-encryption
**File:** `gateway/src/rotation.rs:181-191`
**Severity:** High

**Description:** In `rotate_credential`, the plaintext secret is decrypted (line 182-187) and then re-encrypted (line 190-191). While zeroization is added at line 195-197, the plaintext exists in memory for the duration of the re-encryption operation. If the re-encryption fails, the zeroization code may not be reached.

**Suggested Fix:** Use a `Zeroizing<String>` wrapper from the `zeroize` crate to ensure automatic zeroization:
```rust
use zeroize::Zeroizing;

let plaintext_secret = Zeroizing::new(
    self.vault.decrypt_string(...)
?);
let new_encrypted = self.vault.encrypt_string(&plaintext_secret)?;
// Automatically zeroized when plaintext_secret goes out of scope
```

**Related Code:** `gateway/src/vault/builtin.rs:113` (String returned from decrypt_string)

---

### 4. Redis Cache Not Invalidated on Credential Delete
**File:** `gateway/src/vault/builtin.rs:156-163`
**Severity:** High

**Description:** The `delete` method only soft-deletes the credential in the database (`is_active = false`). It does not invalidate any cached entries in Redis. Other instances with the credential cached will continue using it until TTL expires.

**Suggested Fix:** Add cache invalidation to the delete method (requires passing cache reference or restructuring).

---

## MEDIUM FINDINGS

### 5. SSN Pattern Allows 9-Digit False Positives
**File:** `gateway/src/middleware/redact.rs:26-34`
**Severity:** Medium

**Description:** The SSN regex `\b\d{3}-\d{2}-\d{4}\b|\b\d{9}\b` will match any 9 consecutive digits. This produces false positives on:
- Unix timestamps (e.g., "1234567890")
- Phone numbers without formatting
- Account numbers
- Order IDs

**Suggested Fix:** Use Luhn validation for the undashed format, or remove the undashed alternative and document that only standard SSN format (XXX-XX-XXXX) is detected:
```rust
// Option 1: Remove undashed format (fewer false positives)
Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap()

// Option 2: Add additional validation after match (requires code changes)
```

---

### 6. Credit Card Pattern Missing Luhn Validation
**File:** `gateway/src/middleware/redact.rs:36-37`
**Severity:** Medium

**Description:** The credit card regex `\b(?:\d{4}[ -]){3}\d{1,7}\b|\b\d{15,16}\b` matches 15-16 digit numbers without Luhn validation. This produces false positives on:
- Timestamps
- Order IDs
- Account numbers

**Suggested Fix:** Implement post-match Luhn validation:
```rust
fn luhn_valid(number: &str) -> bool {
    let digits: Vec<u32> = number.chars()
        .filter_map(|c| c.to_digit(10))
        .collect();
    // Luhn algorithm implementation
}
```

---

### 7. Passport Pattern Too Broad
**File:** `gateway/src/middleware/redact.rs:59-66`
**Severity:** Medium

**Description:** The passport regex `\b[A-Z]{2}\d{7,9}\b|\b[A-Z]\d{7,8}\b|\b[A-Z]\d{2}[A-Z]\d{2}[A-Z]\d{2}\b` will match:
- Model version strings like "GPT4O1234567"
- Product codes
- Any uppercase word followed by 7-9 digits

**Suggested Fix:** Add negative lookbehind for common prefixes, or require specific passport-related context.

---

## LOW FINDINGS

### 8. NLP Entity Position Mismatch After Regex Redaction
**File:** `gateway/src/middleware/pii/mod.rs:92-118`
**Severity:** Low

**Description:** The `redact_string_with_entities` function uses string replacement (`result.contains(&entity.text)`) rather than position-based slicing. This works correctly but means entity positions (start/end) are effectively ignored. If regex redaction runs first and modifies the string, NLP entity positions will be incorrect.

**Mitigation:** The current implementation handles this by using text search instead of positions, which is correct but O(n*m) for each entity.

**Related Code:** `gateway/src/middleware/pii/presidio.rs:139-154` (position to byte offset conversion)

---

### 9. UTF-8 Character Offset Conversion Performance
**File:** `gateway/src/middleware/pii/presidio.rs:139-154, 221-236`
**Severity:** Low

**Description:** Converting character offsets to byte offsets uses `text.char_indices().nth(e.start)` which is O(n) for each entity. For large texts with many entities, this could impact performance.

**Suggested Fix:** Pre-compute a character-to-byte index mapping for texts with many entities.

---

## POSITIVE OBSERVATIONS

1. **DEK Zeroization:** Properly implemented at lines 64-66, 98-100, 110-111 of `builtin.rs`
2. **Envelope Encryption:** Correct AES-256-GCM implementation with unique nonces per encryption
3. **Nonce Generation:** Uses `OsRng` (cryptographically secure) at lines 177-181
4. **Authentication Tag Validation:** AES-GCM decrypt returns error on tampering (verified by tests)
5. **Optimistic Concurrency:** Rotation uses version check to prevent concurrent modification
6. **SSRF Protection:** Presidio endpoint validated via `is_safe_webhook_url` at lines 98, 179
7. **ReDoS Protection:** Regex size limit of 1MB applied at line 219 of `redact.rs`
8. **Header Injection Protection:** Newlines/carriage returns blocked at line 389 of `redact.rs`
9. **Reserved Header Protection:** Authorization, Cookie, etc. blocked at line 410-416 of `redact.rs`

---

## FILES REVIEWED

- `gateway/src/vault/builtin.rs` (338 lines) - AES-256-GCM encryption
- `gateway/src/vault/mod.rs` (23 lines) - Vault trait definition
- `gateway/src/middleware/redact.rs` (1100+ lines) - PII redaction patterns
- `gateway/src/middleware/pii/mod.rs` (300 lines) - NLP-based PII detection
- `gateway/src/middleware/pii/presidio.rs` (239 lines) - Presidio integration
- `gateway/src/cache.rs` (207 lines) - Tiered cache implementation
- `gateway/src/rotation.rs` (280+ lines) - Credential rotation scheduler
- `gateway/src/store/postgres/types.rs` (416 lines) - Credential types

---

## RECOMMENDATIONS PRIORITY

1. **Critical:** Implement `Drop` for `VaultCrypto` to zeroize KEK
2. **High:** Fix cache invalidation ordering in rotation (invalidate before DB update)
3. **High:** Add Redis invalidation to cache methods
4. **Medium:** Consider Luhn validation for credit card detection
5. **Medium:** Tighten SSN pattern to reduce false positives