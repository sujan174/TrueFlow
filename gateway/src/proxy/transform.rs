/// URL rewriting and header mutation for upstream requests.
/// Strips TrueFlow-specific headers, rewrites the Host header,
/// and rebuilds the URL from the token's upstream_url + request path.
pub fn rewrite_url(upstream_base: &str, original_path: &str) -> String {
    format!("{}{}", upstream_base.trim_end_matches('/'), original_path)
}
