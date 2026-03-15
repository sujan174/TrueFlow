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
                tracing::warn!("Upstream request failed: {}", e);
                crate::errors::AppError::Upstream(e.to_string())
            })
    }

    /// Forward a request and return the raw response without consuming the body.
    /// Used for streaming (SSE) requests where we want to pipe bytes directly
    /// to the client.
    ///
    /// HIGH-8: Retries ONCE for connection-level failures (before any bytes received).
    /// SSE streams are not idempotent once data starts flowing, but connection
    /// failures before any data is received are safe to retry.
    pub async fn forward_raw(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: bytes::Bytes,
    ) -> Result<reqwest::Response, crate::errors::AppError> {
        // HIGH-8: Single retry for connection-level failures
        for attempt in 0..2 {
            match self
                .client
                .request(method.clone(), url)
                .headers(headers.clone())
                .body(body.clone())
                .send()
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Only retry on connection-level errors (no bytes received)
                    // These are safe to retry because the server never started processing
                    let error_str = e.to_string();
                    let is_connection_error = e.is_connect()
                        || e.is_timeout()
                        || error_str.contains("connection reset")
                        || error_str.contains("broken pipe");

                    if attempt == 0 && is_connection_error {
                        tracing::warn!(
                            error = %error_str,
                            attempt = attempt + 1,
                            "HIGH-8: Connection-level failure for streaming request, retrying once"
                        );
                        // Small delay before retry
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                    tracing::warn!("Upstream streaming request failed: {}", error_str);
                    return Err(crate::errors::AppError::Upstream(error_str));
                }
            }
        }

        // Should not reach here
        Err(crate::errors::AppError::Upstream(
            "unexpected retry loop exit".to_string(),
        ))
    }
}
