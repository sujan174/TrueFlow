//! SSO / OIDC Integration — JWT validation and claim → RBAC mapping.
//!
//! Supports any OIDC-compliant IdP (Okta, Azure AD, Google Workspace, Auth0).
//! Workflow:
//! 1. Fetch `.well-known/openid-configuration` from provider
//! 2. Cache JWKS keys (public keys for JWT signature verification)
//! 3. On each request with `Authorization: Bearer <jwt>`:
//!    a. Decode header → find matching `kid` in JWKS
//!    b. Verify signature, expiry, audience, issuer
//!    c. Map OIDC claims → AuthContext (role, scopes, org_id, user_id)
//!
//! Keys are cached in-memory with a 1-hour TTL and refreshed on cache miss.

use chrono::{Duration, Utc};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Types ────────────────────────────────────────────────────

/// OIDC provider configuration (loaded from DB).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcProvider {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub jwks_uri: Option<String>,
    pub audience: Option<String>,
    pub claim_mapping: serde_json::Value,
    pub default_role: String,
    pub default_scopes: String,
    pub enabled: bool,
}

/// OpenID Connect Discovery document (subset of fields we need).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub jwks_uri: String,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
}

/// JSON Web Key Set.
#[derive(Debug, Clone, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// A single JSON Web Key.
#[derive(Debug, Clone, Deserialize)]
pub struct Jwk {
    pub kty: String,
    pub kid: Option<String>,
    #[serde(rename = "use")]
    pub key_use: Option<String>,
    pub alg: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
    // For EC keys
    #[allow(dead_code)]
    pub crv: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
}

/// Validated OIDC claims extracted from a JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcClaims {
    /// Subject (user identifier from the IdP)
    pub sub: String,
    /// Email (if present in the token)
    pub email: Option<String>,
    /// Name (if present)
    pub name: Option<String>,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: Option<String>,
    /// Expiration (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: Option<i64>,
    /// All raw claims (for custom claim mapping)
    pub raw: serde_json::Value,
}

/// Result of OIDC authentication — maps to AuthContext fields.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OidcAuthResult {
    pub user_id: String,
    pub email: Option<String>,
    pub role: String,
    pub scopes: Vec<String>,
    pub org_id: Uuid,
    pub provider_name: String,
}

// ── JWKS Cache ───────────────────────────────────────────────

struct CachedJwks {
    jwks: Jwks,
    fetched_at: chrono::DateTime<Utc>,
}

static JWKS_CACHE: Lazy<DashMap<String, CachedJwks>> = Lazy::new(DashMap::new);

const JWKS_CACHE_TTL_SECS: i64 = 3600; // 1 hour

/// SEC-02: Validate OIDC URL to prevent SSRF attacks.
/// Blocks private IPs, loopback, link-local, and cloud metadata endpoints.
fn validate_oidc_url(url_str: &str) -> anyhow::Result<()> {
    let parsed = url::Url::parse(url_str).map_err(|e| {
        anyhow::anyhow!("SEC-02: Invalid OIDC URL '{}': {}", url_str, e)
    })?;

    // Must be HTTPS (allow HTTP only for localhost in development)
    match parsed.scheme() {
        "https" => {}
        "http" => {
            let host = parsed.host_str().unwrap_or("");
            if host != "localhost" && host != "127.0.0.1" && host != "[::1]" {
                return Err(anyhow::anyhow!(
                    "SEC-02: OIDC URL must use HTTPS (got '{}')",
                    url_str
                ));
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "SEC-02: OIDC URL has unsupported scheme: {}",
                parsed.scheme()
            ));
        }
    }

    // Block cloud metadata endpoints and known dangerous hosts
    let host = parsed.host_str().unwrap_or("");
    let blocked_hosts = [
        "169.254.169.254",      // AWS/GCP/Azure metadata
        "metadata.google.internal",
        "metadata.internal",
        "0.0.0.0",
        "localhost",            // Block localhost for HTTPS (only allow via HTTP check above)
    ];
    if blocked_hosts.contains(&host) {
        return Err(anyhow::anyhow!(
            "SEC-02: OIDC URL targets blocked host '{}'",
            host
        ));
    }

    // Block private/reserved IP ranges
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        let is_private = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || (v4.octets()[0] == 169 && v4.octets()[1] == 254) // link-local
                    || v4.octets()[0] == 127 // loopback
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        };
        if is_private {
            return Err(anyhow::anyhow!(
                "SEC-02: OIDC URL targets private/reserved IP '{}'",
                ip
            ));
        }
    }

    Ok(())
}

/// Fetch JWKS keys for a provider, with caching.
pub async fn get_jwks(jwks_uri: &str) -> anyhow::Result<Jwks> {
    // SEC-02: Validate URL before fetching to prevent SSRF
    validate_oidc_url(jwks_uri)?;

    // Check cache
    if let Some(cached) = JWKS_CACHE.get(jwks_uri) {
        let age = Utc::now() - cached.fetched_at;
        if age < Duration::seconds(JWKS_CACHE_TTL_SECS) {
            return Ok(cached.jwks.clone());
        }
    }

    // Fetch fresh
    tracing::info!(jwks_uri = %jwks_uri, "Fetching JWKS keys");
    let resp = reqwest::get(jwks_uri).await?;
    let jwks: Jwks = resp.json().await?;

    JWKS_CACHE.insert(
        jwks_uri.to_string(),
        CachedJwks {
            jwks: jwks.clone(),
            fetched_at: Utc::now(),
        },
    );

    Ok(jwks)
}

/// SEC-12: Invalidate JWKS cache for a given URI.
/// Called when signature verification fails to force a fresh key fetch on next attempt.
pub fn invalidate_jwks_cache(jwks_uri: &str) {
    if JWKS_CACHE.remove(jwks_uri).is_some() {
        tracing::info!(jwks_uri = %jwks_uri, "SEC-12: Invalidated JWKS cache entry");
    }
}

/// Fetch JWKS keys, bypassing the cache.
/// SEC-12: Used when signature verification fails to force a fresh key fetch.
pub async fn get_jwks_force_refresh(jwks_uri: &str) -> anyhow::Result<Jwks> {
    // SEC-02: Validate URL before fetching to prevent SSRF
    validate_oidc_url(jwks_uri)?;

    // Invalidate cache first
    invalidate_jwks_cache(jwks_uri);

    // Fetch fresh
    tracing::info!(jwks_uri = %jwks_uri, "SEC-12: Force-refreshing JWKS keys");
    let resp = reqwest::get(jwks_uri).await?;
    let jwks: Jwks = resp.json().await?;

    JWKS_CACHE.insert(
        jwks_uri.to_string(),
        CachedJwks {
            jwks: jwks.clone(),
            fetched_at: Utc::now(),
        },
    );

    Ok(jwks)
}

/// Discover OIDC configuration from issuer URL.
pub async fn discover(issuer_url: &str) -> anyhow::Result<OidcDiscovery> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    );

    // SEC-02: Validate URL before fetching to prevent SSRF
    validate_oidc_url(&url)?;

    tracing::info!(url = %url, "OIDC discovery");
    let resp = reqwest::get(&url).await?;
    let discovery: OidcDiscovery = resp.json().await?;
    Ok(discovery)
}

// ── JWT Validation ───────────────────────────────────────────

/// Decode a JWT token (header only — for kid extraction).
/// Returns the key ID (kid) from the JWT header.
/// MED-3: Validates kid format to prevent injection attacks.
pub fn extract_kid(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // Decode header
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header_bytes = engine.decode(parts[0]).ok()?;
    let header: serde_json::Value = serde_json::from_slice(&header_bytes).ok()?;
    let kid = header.get("kid").and_then(|v| v.as_str()).map(String::from)?;

    // MED-3: Validate kid format - alphanumeric, dashes, underscores only, max 128 chars
    // This prevents SQL injection, path traversal, and other injection attacks via kid
    if kid.len() > 128 {
        tracing::warn!(
            kid_len = kid.len(),
            "MED-3: JWT kid exceeds maximum length (128), rejecting"
        );
        return None;
    }

    // Allow alphanumeric, dashes, and underscores only
    if !kid.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        tracing::warn!(
            kid = %kid,
            "MED-3: JWT kid contains invalid characters, rejecting"
        );
        return None;
    }

    // Check for common injection patterns
    let lower_kid = kid.to_lowercase();
    let suspicious_patterns = [
        "select ", "insert ", "update ", "delete ", "drop ", "union ",
        "../", "..\\", "http://", "https://", "file://",
        "<script", "javascript:", "onerror=", "onload=",
    ];
    for pattern in suspicious_patterns {
        if lower_kid.contains(pattern) {
            tracing::warn!(
                kid = %kid,
                pattern = pattern,
                "MED-3: JWT kid contains suspicious pattern, rejecting"
            );
            return None;
        }
    }

    Some(kid)
}

/// Extract the `alg` field from the JWT header.
fn extract_alg(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header_bytes = engine.decode(parts[0]).ok()?;
    let header: serde_json::Value = serde_json::from_slice(&header_bytes).ok()?;
    header.get("alg").and_then(|v| v.as_str()).map(String::from)
}

/// Construct a `jsonwebtoken::DecodingKey` from a JWK.
fn decoding_key_from_jwk(jwk: &Jwk) -> anyhow::Result<jsonwebtoken::DecodingKey> {
    match jwk.kty.as_str() {
        "RSA" => {
            let n = jwk
                .n
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("RSA JWK missing 'n' field"))?;
            let e = jwk
                .e
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("RSA JWK missing 'e' field"))?;
            Ok(jsonwebtoken::DecodingKey::from_rsa_components(n, e)?)
        }
        "EC" => {
            let x = jwk
                .x
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("EC JWK missing 'x' field"))?;
            let y = jwk
                .y
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("EC JWK missing 'y' field"))?;
            Ok(jsonwebtoken::DecodingKey::from_ec_components(x, y)?)
        }
        other => Err(anyhow::anyhow!("Unsupported JWK key type: {}", other)),
    }
}

/// Map a JWT `alg` string to a `jsonwebtoken::Algorithm`.
fn alg_from_str(alg: &str) -> anyhow::Result<jsonwebtoken::Algorithm> {
    match alg {
        "RS256" => Ok(jsonwebtoken::Algorithm::RS256),
        "RS384" => Ok(jsonwebtoken::Algorithm::RS384),
        "RS512" => Ok(jsonwebtoken::Algorithm::RS512),
        "ES256" => Ok(jsonwebtoken::Algorithm::ES256),
        "ES384" => Ok(jsonwebtoken::Algorithm::ES384),
        "PS256" => Ok(jsonwebtoken::Algorithm::PS256),
        "PS384" => Ok(jsonwebtoken::Algorithm::PS384),
        "PS512" => Ok(jsonwebtoken::Algorithm::PS512),
        "EdDSA" => Ok(jsonwebtoken::Algorithm::EdDSA),
        other => Err(anyhow::anyhow!("Unsupported JWT algorithm: {}", other)),
    }
}

/// Verify a JWT's cryptographic signature against the provider's JWKS,
/// then extract and validate claims (exp, iss, aud).
///
/// This is the **primary entry point** for secure JWT validation.
/// SEC-12: Implements cache invalidation on signature failure to support key rotation.
pub async fn verify_jwt_signature(
    token: &str,
    provider: &OidcProvider,
) -> anyhow::Result<OidcClaims> {
    // 1. Resolve JWKS URI (from provider or via discovery)
    let jwks_uri = match &provider.jwks_uri {
        Some(uri) => uri.clone(),
        None => {
            let discovery = discover(&provider.issuer_url).await?;
            discovery.jwks_uri
        }
    };

    // 2. Extract kid + alg from JWT header (needed for key selection)
    let kid = extract_kid(token);
    let alg_str =
        extract_alg(token).ok_or_else(|| anyhow::anyhow!("JWT header missing 'alg' field"))?;
    let algorithm = alg_from_str(&alg_str)?;

    // SEC-11: Require exact kid match - no fallback to first key
    let kid_val = kid.as_ref().ok_or_else(|| {
        anyhow::anyhow!("SEC-11: JWT header missing 'kid' field - key selection requires explicit key ID")
    })?;

    // 3. First attempt: use cached JWKS
    let jwks = get_jwks(&jwks_uri).await?;

    match verify_with_jwks(token, provider, &jwks, kid_val, algorithm).await {
        Ok(claims) => Ok(claims),
        Err(e) => {
            // SEC-12: Signature verification failed - try with fresh keys
            // This handles key rotation where the IdP has new keys we haven't cached yet
            tracing::warn!(
                error = %e,
                jwks_uri = %jwks_uri,
                "SEC-12: JWT verification failed with cached keys, attempting cache refresh"
            );

            // Force refresh JWKS
            let fresh_jwks = get_jwks_force_refresh(&jwks_uri).await?;

            match verify_with_jwks(token, provider, &fresh_jwks, kid_val, algorithm).await {
                Ok(claims) => {
                    tracing::info!(
                        jwks_uri = %jwks_uri,
                        "SEC-12: JWT verification succeeded after cache refresh"
                    );
                    Ok(claims)
                }
                Err(refresh_error) => {
                    // Still failed after refresh - return the original error with context
                    Err(anyhow::anyhow!(
                        "JWT verification failed even after cache refresh: {} (original: {})",
                        refresh_error, e
                    ))
                }
            }
        }
    }
}

/// Helper function to verify JWT with specific JWKS
async fn verify_with_jwks(
    token: &str,
    provider: &OidcProvider,
    jwks: &Jwks,
    kid_val: &str,
    algorithm: jsonwebtoken::Algorithm,
) -> anyhow::Result<OidcClaims> {
    // Find the matching JWK
    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid.as_deref() == Some(kid_val))
        .ok_or_else(|| anyhow::anyhow!("SEC-11: No JWK found with kid='{}'", kid_val))?;

    // Build DecodingKey
    let decoding_key = decoding_key_from_jwk(jwk)?;

    // Build Validation
    let mut validation = jsonwebtoken::Validation::new(algorithm);
    validation.set_issuer(&[&provider.issuer_url]);
    if let Some(ref aud) = provider.audience {
        validation.set_audience(&[aud]);
    } else {
        // HIGH-1: Require explicit opt-in for disabled audience validation
        // Tokens issued for other clients could be accepted without this
        if std::env::var("TRUEFLOW_ALLOW_EMPTY_AUDIENCE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            tracing::warn!(
                provider_id = %provider.id,
                issuer = %provider.issuer_url,
                "HIGH-1: OIDC provider has no audience configured - audience validation DISABLED via TRUEFLOW_ALLOW_EMPTY_AUDIENCE. \
                 This allows potential token replay attacks. Configure 'audience' in provider config."
            );
            validation.validate_aud = false;
        } else {
            return Err(anyhow::anyhow!(
                "HIGH-1: OIDC provider {} has no audience configured. Audience validation is REQUIRED for security. \
                 Set 'audience' in provider config or set TRUEFLOW_ALLOW_EMPTY_AUDIENCE=1 to opt-in to this risk.",
                provider.id
            ));
        }
    }
    validation.validate_exp = true;

    // Decode + verify
    let token_data = jsonwebtoken::decode::<serde_json::Value>(token, &decoding_key, &validation)
        .map_err(|e| anyhow::anyhow!("JWT signature verification failed: {}", e))?;

    extract_claims_from_token(token_data.claims)
}

/// Extract standard claims from a decoded JWT payload.
fn extract_claims_from_token(raw: serde_json::Value) -> anyhow::Result<OidcClaims> {
    let sub = raw
        .get("sub")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("JWT missing 'sub' claim"))?
        .to_string();
    let exp = raw
        .get("exp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| anyhow::anyhow!("JWT missing 'exp' claim"))?;

    Ok(OidcClaims {
        sub,
        email: raw.get("email").and_then(|v| v.as_str()).map(String::from),
        name: raw.get("name").and_then(|v| v.as_str()).map(String::from),
        iss: raw
            .get("iss")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        aud: raw.get("aud").and_then(|v| v.as_str()).map(String::from),
        exp,
        iat: raw.get("iat").and_then(|v| v.as_i64()),
        raw,
    })
}

/// Full OIDC validation pipeline: verify signature → extract claims → map to RBAC.
///
/// Call this from the auth middleware when a Bearer JWT is received.
pub async fn validate_jwt(token: &str, provider: &OidcProvider) -> anyhow::Result<OidcAuthResult> {
    let claims = verify_jwt_signature(token, provider).await?;
    Ok(map_claims_to_rbac(&claims, provider))
}

/// Decode JWT claims **without** cryptographic verification.
///
/// **DEPRECATED** — use `verify_jwt_signature()` for production validation.
/// Kept for backward-compatible unit tests and non-IdP fallback paths.
#[allow(dead_code)]
pub fn decode_claims(token: &str) -> anyhow::Result<OidcClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("Invalid JWT format: expected 3 parts"));
    }

    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload_bytes = engine
        .decode(parts[1])
        .map_err(|e| anyhow::anyhow!("JWT payload decode error: {}", e))?;
    let raw: serde_json::Value = serde_json::from_slice(&payload_bytes)?;

    let sub = raw
        .get("sub")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("JWT missing 'sub' claim"))?
        .to_string();

    let exp = raw
        .get("exp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| anyhow::anyhow!("JWT missing 'exp' claim"))?;

    // Check expiry
    if exp < Utc::now().timestamp() {
        return Err(anyhow::anyhow!("JWT expired"));
    }

    Ok(OidcClaims {
        sub,
        email: raw.get("email").and_then(|v| v.as_str()).map(String::from),
        name: raw.get("name").and_then(|v| v.as_str()).map(String::from),
        iss: raw
            .get("iss")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        aud: raw.get("aud").and_then(|v| v.as_str()).map(String::from),
        exp,
        iat: raw.get("iat").and_then(|v| v.as_i64()),
        raw,
    })
}

/// Map OIDC claims to RBAC attributes using the provider's claim mapping.
///
/// claim_mapping example:
/// ```json
/// {
///   "role": "custom:trueflow_role",
///   "scopes": "custom:trueflow_scopes"
/// }
/// ```
///
/// This means: look for `custom:trueflow_role` in the JWT claims → use as role.
pub fn map_claims_to_rbac(claims: &OidcClaims, provider: &OidcProvider) -> OidcAuthResult {
    let mapping = &provider.claim_mapping;

    // Extract role from mapped claim, fall back to provider default
    let raw_role = mapping
        .get("role")
        .and_then(|v| v.as_str())
        .and_then(|claim_path| claims.raw.get(claim_path))
        .and_then(|v| v.as_str())
        .unwrap_or(&provider.default_role);

    // SEC-07: Sanitize role to prevent injection attacks
    // Only allow known valid role values. "superadmin" is capped to "admin"
    // because SuperAdmin should only come from environment key, not OIDC.
    let role = match raw_role.to_lowercase().as_str() {
        "superadmin" => {
            tracing::warn!(
                sub = %claims.sub,
                "SEC-07: OIDC claim attempted 'superadmin' role, capping at 'admin'"
            );
            "admin".to_string()
        }
        "admin" => "admin".to_string(),
        "member" => "member".to_string(),
        "readonly" | "viewer" | "read_only" | "read-only" => "readonly".to_string(),
        _ => {
            tracing::warn!(
                sub = %claims.sub,
                raw_role = %raw_role,
                "SEC-07: OIDC claim had invalid role, falling back to provider default"
            );
            // Fall back to provider default, also sanitize it
            match provider.default_role.to_lowercase().as_str() {
                "admin" => "admin".to_string(),
                "member" => "member".to_string(),
                "readonly" | "viewer" => "readonly".to_string(),
                _ => "readonly".to_string(), // Safe default
            }
        }
    };

    // Extract scopes from mapped claim, fall back to provider defaults
    let scopes = mapping
        .get("scopes")
        .and_then(|v| v.as_str())
        .and_then(|claim_path| claims.raw.get(claim_path))
        .and_then(|v| v.as_str())
        .unwrap_or(&provider.default_scopes)
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    OidcAuthResult {
        user_id: claims.sub.clone(),
        email: claims.email.clone(),
        role,
        scopes,
        org_id: provider.org_id,
        provider_name: provider.name.clone(),
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_provider() -> OidcProvider {
        OidcProvider {
            id: Uuid::nil(),
            org_id: Uuid::nil(),
            name: "Test Provider".to_string(),
            issuer_url: "https://test.okta.com".to_string(),
            client_id: "test-client-id".to_string(),
            jwks_uri: None,
            audience: None,
            claim_mapping: serde_json::json!({
                "role": "custom:trueflow_role",
                "scopes": "custom:trueflow_scopes"
            }),
            default_role: "viewer".to_string(),
            default_scopes: "audit:read".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn test_extract_kid_from_jwt() {
        // Create a test JWT header: {"alg":"RS256","kid":"test-key-1"}
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(r#"{"alg":"RS256","kid":"test-key-1"}"#);
        let payload = engine.encode(r#"{"sub":"user1","exp":9999999999}"#);
        let token = format!("{}.{}.signature", header, payload);

        let kid = extract_kid(&token);
        assert_eq!(kid, Some("test-key-1".to_string()));
    }

    #[test]
    fn test_extract_kid_missing() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(r#"{"alg":"RS256"}"#);
        let payload = engine.encode(r#"{"sub":"user1"}"#);
        let token = format!("{}.{}.signature", header, payload);

        let kid = extract_kid(&token);
        assert_eq!(kid, None);
    }

    #[test]
    fn test_decode_claims() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(r#"{"alg":"RS256"}"#);
        let payload = engine.encode(r#"{"sub":"user-123","email":"user@example.com","exp":9999999999,"iss":"https://test.okta.com"}"#);
        let token = format!("{}.{}.signature", header, payload);

        let claims = decode_claims(&token).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, Some("user@example.com".to_string()));
        assert_eq!(claims.iss, "https://test.okta.com");
    }

    #[test]
    fn test_decode_claims_expired() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(r#"{"alg":"RS256"}"#);
        let payload = engine.encode(r#"{"sub":"expired-user","exp":1000000000}"#);
        let token = format!("{}.{}.signature", header, payload);

        let result = decode_claims(&token);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[test]
    fn test_map_claims_with_custom_role() {
        let provider = make_test_provider();

        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(r#"{"alg":"RS256"}"#);
        let payload = engine.encode(r#"{"sub":"admin-user","exp":9999999999,"custom:trueflow_role":"admin","custom:trueflow_scopes":"*"}"#);
        let token = format!("{}.{}.signature", header, payload);

        let claims = decode_claims(&token).unwrap();
        let result = map_claims_to_rbac(&claims, &provider);

        assert_eq!(result.user_id, "admin-user");
        assert_eq!(result.role, "admin");
        assert_eq!(result.scopes, vec!["*"]);
        assert_eq!(result.org_id, provider.org_id);
    }

    #[test]
    fn test_map_claims_defaults() {
        let provider = make_test_provider();

        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(r#"{"alg":"RS256"}"#);
        // No custom claims → should use defaults
        let payload = engine.encode(r#"{"sub":"basic-user","exp":9999999999}"#);
        let token = format!("{}.{}.signature", header, payload);

        let claims = decode_claims(&token).unwrap();
        let result = map_claims_to_rbac(&claims, &provider);

        // SEC-07: "viewer" is normalized to "readonly"
        assert_eq!(result.role, "readonly"); // default (normalized from "viewer")
        assert_eq!(result.scopes, vec!["audit:read"]); // default
    }

    #[test]
    fn test_invalid_jwt_format() {
        let result = decode_claims("not-a-jwt");
        assert!(result.is_err());
    }
}
