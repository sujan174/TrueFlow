use crate::models::policy::RetryConfig;
use reqwest::Client;
use std::time::Duration;

#[derive(Clone)]
pub struct UpstreamClient {
    client: Client,
}

impl UpstreamClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .use_rustls_tls()
            .pool_max_idle_per_host(32)
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!("Failed to init upstream HTTP client: {:?}", e);
                std::process::exit(1);
            });

        Self { client }
    }

    pub async fn forward(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: bytes::Bytes,
        retry_config: &RetryConfig,
    ) -> Result<reqwest::Response, crate::errors::AppError> {
        crate::proxy::retry::robust_request(&self.client, method, url, headers, body, retry_config)
            .await
            .map_err(|e| {
                eprintln!("DEBUG upstream error: url={} err={:?}", url, e);
                tracing::warn!("Upstream request failed: {}", e);
                crate::errors::AppError::Upstream(e.to_string())
            })
    }

    /// Forward a request and return the raw response without consuming the body.
    /// Used for streaming (SSE) requests where we want to pipe bytes directly
    /// to the client. Does NOT retry — SSE streams are not idempotent.
    pub async fn forward_raw(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: bytes::Bytes,
    ) -> Result<reqwest::Response, crate::errors::AppError> {
        self.client
            .request(method, url)
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Upstream streaming request failed: {}", e);
                crate::errors::AppError::Upstream(e.to_string())
            })
    }
}
