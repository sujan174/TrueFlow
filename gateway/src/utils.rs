//! Shared utilities for TrueFlow.

/// Glob pattern matching for model/provider names.
/// Supports:
/// - `*` matches any sequence of characters
/// - `?` matches any single character
/// - Literal characters match exactly (case-insensitive)
///
/// # Security
/// Limits recursion depth to prevent stack overflow on malicious patterns.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    // Case-insensitive matching for model names (GPT-4O == gpt-4o)
    let pattern_lower: String = pattern.to_lowercase();
    let text_lower: String = text.to_lowercase();
    let pattern_chars: Vec<char> = pattern_lower.chars().collect();
    let text_chars: Vec<char> = text_lower.chars().collect();

    const MAX_DEPTH: usize = 256;

    fn match_helper(pattern: &[char], text: &[char], depth: usize) -> bool {
        // Prevent stack overflow on deeply nested wildcards
        if depth > MAX_DEPTH {
            tracing::warn!("glob_match exceeded max recursion depth");
            return false;
        }

        match (pattern.first(), text.first()) {
            (None, None) => true, // Both exhausted = match
            (Some('*'), _) => {
                // Try matching * with zero chars, or with one+ chars
                match_helper(&pattern[1..], text, depth + 1)
                    || (!text.is_empty() && match_helper(pattern, &text[1..], depth + 1))
            }
            (Some('?'), Some(_)) => {
                // ? matches exactly one char
                match_helper(&pattern[1..], &text[1..], depth + 1)
            }
            (Some(p), Some(t)) if *p == *t => {
                // Exact match, advance both
                match_helper(&pattern[1..], &text[1..], depth + 1)
            }
            _ => false,
        }
    }

    match_helper(&pattern_chars, &text_chars, 0)
}

/// SEC: Validate that a webhook URL from a policy definition is safe to call.
/// Blocks private IPs (v4 + v6), cloud metadata endpoints, and non-HTTP(S) schemes.
/// Returns `true` if `ip` is a public, routable IP address.
/// Returns `false` for loopback, private, link-local, unspecified, and cloud-metadata ranges.
pub fn is_public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            // SEC: Allow loopback for integration tests (wiremock)
            if v4.is_loopback() && std::env::var("TRUEFLOW_TEST_ALLOW_LOCAL_WEBHOOKS").is_ok() {
                return true;
            }
            !v4.is_loopback()
                && !v4.is_private()
                && !v4.is_link_local()
                && !v4.is_unspecified()
                && !(v4.octets()[0] == 169 && v4.octets()[1] == 254)
                // Alibaba Cloud ECS metadata service
                && !(v4.octets() == [100, 100, 100, 200])
        }
        std::net::IpAddr::V6(v6) => {
            // SEC: Allow loopback for integration tests (wiremock)
            if v6.is_loopback() && std::env::var("TRUEFLOW_TEST_ALLOW_LOCAL_WEBHOOKS").is_ok() {
                return true;
            }
            !v6.is_loopback()
                && !v6.is_unspecified()
                // Full unique-local fc00::/7 (covers both fc00::/8 and fd00::/8)
                && (v6.segments()[0] & 0xfe00) != 0xfc00
                // Link-local fe80::/10
                && (v6.segments()[0] & 0xffc0) != 0xfe80
                // IPv4-mapped ::ffff:x.x.x.x — validate the embedded v4
                && !v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| !is_public_ip(std::net::IpAddr::V4(v4)))
        }
    }
}

/// SEC: SSRF protection for policy-defined webhook URLs.
///
/// Two-stage check:
/// 1. Scheme must be http or https.
/// 2. If the host is a literal IP → validate it immediately.
/// 3. If the host is a domain name → resolve ALL returned A/AAAA records via DNS
///    and validate each IP. This prevents DNS-rebinding attacks where a domain
///    initially points to a public IP (passes a static check) and is later
///    rebound to 169.254.169.254 (metadata service) before the actual HTTP call.
///
/// Fails closed on DNS failure (no resolution → blocked).
pub async fn is_safe_webhook_url(url_str: &str) -> bool {
    let parsed = match reqwest::Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return false,
    };

    // Only allow HTTP(S)
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }

    let host = match parsed.host_str() {
        Some(h) => h,
        None => return false,
    };

    // Block cloud metadata hostnames and localhost by literal name
    // SEC: Allow localhost for integration tests when TRUEFLOW_TEST_ALLOW_LOCAL_WEBHOOKS is set
    let allow_local = std::env::var("TRUEFLOW_TEST_ALLOW_LOCAL_WEBHOOKS").is_ok();
    let blocked_hosts = [
        "169.254.169.254",
        "metadata.google.internal",
        "metadata.internal",
        "0.0.0.0",
        "localhost",
        "ip6-localhost",
        "ip6-loopback",
        // AWS IMDSv2 IPv6
        "fd00:ec2::254",
        "[fd00:ec2::254]",
        "[::1]",
    ];
    if blocked_hosts.contains(&host) && !(allow_local && (host == "localhost" || host == "ip6-localhost")) {
        return false;
    }

    // If host is a literal IP address, validate immediately (no DNS lookup needed)
    if let Ok(ip) = host.trim_matches(|c| c == '[' || c == ']').parse::<std::net::IpAddr>() {
        return is_public_ip(ip);
    }

    // Host is a domain name — resolve ALL addresses and validate each one.
    // Using port 443 as the lookup port (any port works for DNS resolution).
    let port = parsed.port_or_known_default().unwrap_or(443);
    match tokio::net::lookup_host(format!("{}:{}", host, port)).await {
        Ok(addrs) => {
            let addrs: Vec<_> = addrs.collect();
            if addrs.is_empty() {
                // DNS resolution returned no addresses — fail closed
                tracing::warn!(host = %host, "SSRF check: DNS returned no addresses, blocking");
                return false;
            }
            // ALL resolved IPs must be public — a single private IP in the list blocks it.
            // This prevents multi-A-record tricks (one public + one private IP).
            let all_public = addrs.iter().all(|addr| is_public_ip(addr.ip()));
            if !all_public {
                tracing::warn!(
                    host = %host,
                    "SSRF check: DNS resolved to private/reserved IP, blocking"
                );
            }
            all_public
        }
        Err(e) => {
            // DNS failure — fail closed (prevents TOCTOU via intermittent resolution)
            tracing::warn!(host = %host, error = %e, "SSRF check: DNS lookup failed, blocking");
            false
        }
    }
}
