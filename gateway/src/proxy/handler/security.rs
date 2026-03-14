//! Security utilities for the proxy handler.
//!
//! Re-exports shared SSRF protection functions from the utils module.

// Re-export SSRF protection from utils module
pub use crate::utils::is_safe_webhook_url;

// ── Unit Tests ──────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    // ── SSRF: is_safe_webhook_url ──────────────────────────────────────
    // NOTE: is_safe_webhook_url is async (DNS-resolving). Literal-IP tests complete
    // instantly (no DNS); domain tests resolve via the system resolver.

    #[tokio::test]
    async fn test_ssrf_blocks_ipv4_private() {
        assert!(!is_safe_webhook_url("http://127.0.0.1/callback").await);
        assert!(!is_safe_webhook_url("http://10.0.0.1/hook").await);
        assert!(!is_safe_webhook_url("http://192.168.1.1:8080/hook").await);
        assert!(!is_safe_webhook_url("http://172.16.0.1/hook").await);
        assert!(!is_safe_webhook_url("http://169.254.169.254/latest/meta-data/").await);
        assert!(!is_safe_webhook_url("http://0.0.0.0/hook").await);
    }

    #[tokio::test]
    async fn test_ssrf_blocks_ipv6_private() {
        assert!(!is_safe_webhook_url("http://[::1]/callback").await);
        assert!(!is_safe_webhook_url("http://[fd00::1]/hook").await);
        assert!(!is_safe_webhook_url("http://[fe80::1]/hook").await);
        // IPv4-mapped IPv6 — loopback
        assert!(!is_safe_webhook_url("http://[::ffff:127.0.0.1]/hook").await);
        // IPv4-mapped IPv6 — private
        assert!(!is_safe_webhook_url("http://[::ffff:10.0.0.1]/hook").await);
    }

    #[tokio::test]
    async fn test_ssrf_blocks_localhost() {
        assert!(!is_safe_webhook_url("http://localhost/hook").await);
        assert!(!is_safe_webhook_url("http://localhost:3000/callback").await);
    }

    #[tokio::test]
    async fn test_ssrf_allows_public_literal_ip() {
        // Literal public IP — no DNS lookup needed; resolves instantly
        assert!(is_safe_webhook_url("http://203.0.113.1/hook").await); // TEST-NET, not private
    }

    #[tokio::test]
    async fn test_ssrf_blocks_ftp_scheme() {
        assert!(!is_safe_webhook_url("ftp://evil.com/payload").await);
        assert!(!is_safe_webhook_url("file:///etc/passwd").await);
        assert!(!is_safe_webhook_url("gopher://evil.com").await);
    }
}