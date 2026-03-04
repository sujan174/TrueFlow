// ═══════════════════════════════════════════════════════════════
// AWS Signature Version 4 (SigV4) Signing
// ═══════════════════════════════════════════════════════════════
//
// FIX(C1): Implements SigV4 request signing for Amazon Bedrock.
// Uses `sha2` and `hmac` crates (already in Cargo.toml) — no
// new dependencies required.
//
// SigV4 is a header-based signing scheme. For each outgoing
// request to Bedrock, we compute a signature over:
//   - HTTP method, canonical URI, query string
//   - Sorted headers (host, content-type, x-amz-date, etc.)
//   - SHA-256 hash of the request body
//
// The credential format in the vault is:
//   "ACCESS_KEY_ID:SECRET_ACCESS_KEY"
// The region is extracted from the upstream URL:
//   "https://bedrock-runtime.us-east-1.amazonaws.com/..."
//
// Reference: https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_aws-signing.html

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use url::Url;

type HmacSha256 = Hmac<Sha256>;

/// Extract the AWS region from a Bedrock-style URL.
/// E.g. `https://bedrock-runtime.us-east-1.amazonaws.com/...` → `us-east-1`
pub fn extract_region(url: &str) -> Option<String> {
    // Pattern: bedrock-runtime.{region}.amazonaws.com
    let url_lower = url.to_lowercase();
    if let Some(start) = url_lower.find("bedrock-runtime.") {
        let after = &url_lower[start + "bedrock-runtime.".len()..];
        if let Some(end) = after.find(".amazonaws.com") {
            return Some(after[..end].to_string());
        }
    }
    // Fallback: try to parse from any amazonaws.com URL
    // e.g. {service}.{region}.amazonaws.com
    if let Some(host_start) = url_lower.find("://") {
        let host_part = &url_lower[host_start + 3..];
        if let Some(host_end) = host_part.find('/') {
            let host = &host_part[..host_end];
            let parts: Vec<&str> = host.split('.').collect();
            // service.region.amazonaws.com → parts[1] is region
            if parts.len() >= 4 && parts[parts.len() - 2] == "amazonaws" {
                return Some(parts[1].to_string());
            }
        }
    }
    None
}

/// SHA-256 hash of data, returned as lowercase hex string.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// HMAC-SHA256 signing.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

/// Derive the SigV4 signing key.
/// key = HMAC("AWS4" + secret, date) → HMAC(_, region) → HMAC(_, service) → HMAC(_, "aws4_request")
fn derive_signing_key(secret: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret).as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

/// Sign a request using AWS Signature Version 4 and inject the
/// `Authorization`, `X-Amz-Date`, and `X-Amz-Content-Sha256` headers
/// into the provided header map.
///
/// # Arguments
/// * `method` — HTTP method (GET, POST, etc.)
/// * `url` — Full request URL
/// * `headers` — Mutable header map to inject auth headers into
/// * `body` — Request body bytes (used for payload hash)
/// * `access_key` — AWS access key ID
/// * `secret_key` — AWS secret access key
/// * `region` — AWS region (e.g. "us-east-1")
/// * `service` — AWS service name (e.g. "bedrock")
#[allow(clippy::too_many_arguments)]
pub fn sign_request(
    method: &str,
    url: &str,
    headers: &mut reqwest::header::HeaderMap,
    body: &[u8],
    access_key: &str,
    secret_key: &str,
    region: &str,
    service: &str,
) -> Result<(), anyhow::Error> {
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    // Parse URL for canonical URI and query string
    let parsed_url = Url::parse(url)
        .map_err(|e| anyhow::anyhow!("SigV4: invalid URL: {}", e))?;
    let host = parsed_url.host_str()
        .ok_or_else(|| anyhow::anyhow!("SigV4: URL has no host"))?;
    let canonical_uri = if parsed_url.path().is_empty() { "/" } else { parsed_url.path() };

    // Canonical query string (sorted)
    let mut query_pairs: Vec<(String, String)> = parsed_url.query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    query_pairs.sort();
    let canonical_querystring: String = query_pairs.iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // Payload hash
    let payload_hash = sha256_hex(body);

    // Inject X-Amz-Date and host headers so they're part of the signed headers
    headers.insert(
        reqwest::header::HeaderName::from_static("x-amz-date"),
        reqwest::header::HeaderValue::from_str(&amz_date)?,
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("x-amz-content-sha256"),
        reqwest::header::HeaderValue::from_str(&payload_hash)?,
    );
    // Ensure host header is set
    headers.entry(reqwest::header::HOST)
        .or_insert(reqwest::header::HeaderValue::from_str(host)?);

    // Canonical headers (sorted by lowercase name)
    // SigV4 requires: host, content-type (if present), x-amz-date, x-amz-content-sha256
    let mut signed_header_list = vec!["host", "x-amz-content-sha256", "x-amz-date"];
    if headers.contains_key("content-type") {
        signed_header_list.push("content-type");
    }
    signed_header_list.sort();
    let signed_headers = signed_header_list.join(";");

    let mut canonical_headers_str = String::new();
    for &name in &signed_header_list {
        let value = headers.get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        canonical_headers_str.push_str(&format!("{}:{}\n", name, value.trim()));
    }

    // Canonical request
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method,
        canonical_uri,
        canonical_querystring,
        canonical_headers_str,
        signed_headers,
        payload_hash
    );

    // String to sign
    let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, region, service);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        credential_scope,
        sha256_hex(canonical_request.as_bytes())
    );

    // Signing key and signature
    let signing_key = derive_signing_key(secret_key, &date_stamp, region, service);
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    // Authorization header
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        access_key, credential_scope, signed_headers, signature
    );

    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&authorization)?,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_region() {
        assert_eq!(
            extract_region("https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3"),
            Some("us-east-1".to_string())
        );
        assert_eq!(
            extract_region("https://bedrock-runtime.eu-west-1.amazonaws.com/model/meta.llama3"),
            Some("eu-west-1".to_string())
        );
        assert_eq!(
            extract_region("https://api.openai.com/v1/chat/completions"),
            None
        );
    }

    #[test]
    fn test_sha256_hex() {
        // SHA-256 of empty string is a well-known constant
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sign_request_produces_authorization_header() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());

        let result = sign_request(
            "POST",
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-sonnet/converse",
            &mut headers,
            b"{\"messages\":[]}",
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "bedrock",
        );

        assert!(result.is_ok(), "signing should succeed");
        assert!(headers.contains_key("authorization"), "must have Authorization");
        assert!(headers.contains_key("x-amz-date"), "must have X-Amz-Date");
        assert!(headers.contains_key("x-amz-content-sha256"), "must have X-Amz-Content-Sha256");

        let auth = headers.get("authorization").unwrap().to_str().unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIA"), "auth header format: {}", auth);
        assert!(auth.contains("SignedHeaders="), "must have signed headers");
        assert!(auth.contains("Signature="), "must have signature");
    }
}
