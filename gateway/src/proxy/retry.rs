use crate::models::policy::RetryConfig;
use anyhow::{Context, Result};
use bytes::Bytes;
use rand::Rng;
use reqwest::{Client, Method, RequestBuilder, Response};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Execute a request with configurable retries, backoff, jitter, and Retry-After support.
/// When `config.max_total_timeout_ms` is set, the total retry budget is enforced:
/// the function returns an error if the deadline is exceeded, even if retries remain.
pub async fn robust_request(
    client: &Client,
    method: Method,
    url: &str,
    headers: reqwest::header::HeaderMap,
    body: Bytes,
    config: &RetryConfig,
) -> Result<Response> {
    let mut attempt = 0;
    let start = std::time::Instant::now();
    let deadline = config
        .max_total_timeout_ms
        .map(|ms| start + Duration::from_millis(ms));

    loop {
        attempt += 1;

        // Check deadline before attempting (skip first attempt — always try at least once)
        if attempt > 1 {
            if let Some(dl) = deadline {
                if std::time::Instant::now() >= dl {
                    let elapsed = start.elapsed();
                    warn!(
                        "Retry time budget exhausted after {:?} ({} attempts) for {} {}",
                        elapsed,
                        attempt - 1,
                        method,
                        url
                    );
                    anyhow::bail!(
                        "Retry time budget of {}ms exceeded after {} attempts (elapsed: {:?})",
                        config.max_total_timeout_ms.unwrap_or(0),
                        attempt - 1,
                        elapsed
                    );
                }
            }
        }

        // Clone headers and body for this attempt
        let req_builder = client
            .request(method.clone(), url)
            .headers(headers.clone())
            .body(body.clone());

        match execute_attempt(req_builder).await {
            Ok(response) => {
                let status = response.status();

                // If success (not a retryable error code), return immediately
                if !config.status_codes.contains(&status.as_u16()) {
                    return Ok(response);
                }

                // If we've exhausted retries, return the last response (even if error)
                if attempt > config.max_retries {
                    debug!(
                        "Exhausted {} retries for {} {}; last status: {}",
                        config.max_retries, method, url, status
                    );
                    return Ok(response);
                }

                // Calculate wait time
                let wait_duration = calculate_wait_time(&response, config, attempt);

                // Check if sleeping would exceed deadline
                if let Some(dl) = deadline {
                    if std::time::Instant::now() + wait_duration > dl {
                        let remaining = dl.saturating_duration_since(std::time::Instant::now());
                        warn!(
                            "Retry sleep of {:?} would exceed time budget. Remaining: {:?}. Returning last response.",
                            wait_duration, remaining
                        );
                        return Ok(response);
                    }
                }

                warn!(
                    "Attempt {}/{} failed with status {}. Retrying in {:?}...",
                    attempt,
                    config.max_retries + 1,
                    status,
                    wait_duration
                );

                sleep(wait_duration).await;
            }
            Err(e) => {
                // Network errors (DNS, connection refused) are always retryable
                if attempt > config.max_retries {
                    return Err(e).context(format!("Request failed after {} attempts", attempt));
                }

                let wait_duration = calculate_backoff(config, attempt);

                // Check if sleeping would exceed deadline
                if let Some(dl) = deadline {
                    if std::time::Instant::now() + wait_duration > dl {
                        return Err(e).context(format!(
                            "Retry time budget of {}ms exceeded after {} attempts",
                            config.max_total_timeout_ms.unwrap_or(0),
                            attempt
                        ));
                    }
                }

                warn!(
                    "Attempt {}/{} failed with error: {}. Retrying in {:?}...",
                    attempt,
                    config.max_retries + 1,
                    e,
                    wait_duration
                );

                sleep(wait_duration).await;
            }
        }
    }
}

async fn execute_attempt(builder: RequestBuilder) -> Result<Response> {
    builder.send().await.map_err(|e| e.into())
}

fn calculate_wait_time(response: &Response, config: &RetryConfig, attempt: u32) -> Duration {
    // 1. Check Retry-After header (RFC 7231 §7.1.3)
    if let Some(retry_after) = response.headers().get(reqwest::header::RETRY_AFTER) {
        if let Ok(retry_after_str) = retry_after.to_str() {
            // Try parsing as delta-seconds (e.g. "120")
            if let Ok(seconds) = retry_after_str.parse::<u64>() {
                return Duration::from_secs(seconds);
            }
            // FIX 4B-2: Try parsing as HTTP-date (IMF-fixdate per RFC 2616)
            // e.g. "Fri, 31 Dec 1999 23:59:59 GMT"
            if let Ok(date) = chrono::DateTime::parse_from_rfc2822(retry_after_str) {
                let now = chrono::Utc::now();
                let target = date.with_timezone(&chrono::Utc);
                if target > now {
                    let delta = (target - now).num_seconds().max(0) as u64;
                    return Duration::from_secs(delta);
                }
                // Date is in the past → retry immediately (0-second wait)
                return Duration::from_secs(0);
            }
        }
    }

    // 2. Fallback to Exponential Backoff
    calculate_backoff(config, attempt)
}

fn calculate_backoff(config: &RetryConfig, attempt: u32) -> Duration {
    let base = config.base_backoff_ms as f64;
    let max = config.max_backoff_ms as f64;

    // Exponential: base * 2^(attempt - 1)
    let raw_backoff = base * 2_f64.powi((attempt as i32) - 1);
    let capped_backoff = raw_backoff.min(max);

    // FIX 4B-1: Full jitter prevents thundering herds.
    // Old: sleep = backoff + random(0, jitter_ms) — all clients cluster near backoff.
    // New: sleep = random(0, backoff) — fully decorrelates retry timing.
    // Reference: AWS Architecture Blog "Exponential Backoff And Jitter"
    let capped_ms = capped_backoff as u64;
    if capped_ms == 0 {
        return Duration::from_millis(0);
    }
    let jittered = rand::thread_rng().gen_range(0..=capped_ms);

    Duration::from_millis(jittered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_retry_on_500_succeeds() {
        let mock_server = MockServer::start().await;

        // Fail twice with 500, then succeed
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let config = RetryConfig::default(); // 3 retries

        let res = robust_request(
            &client,
            Method::GET,
            &format!("{}/test", mock_server.uri()),
            reqwest::header::HeaderMap::new(),
            Bytes::new(),
            &config,
        )
        .await
        .unwrap();

        assert_eq!(res.status(), 200);
    }

    // ── Chaos: 429 + Retry-After Header ────────────────────────

    /// Upstream returns 429 with `Retry-After: 1` twice, then 200.
    /// Total elapsed time MUST be ≥ 1.8s, proving the header is respected.
    #[tokio::test]
    async fn test_retry_respects_429_retry_after_header() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "1")
                    .set_body_string(r#"{"error":{"message":"Rate limit exceeded"}}"#),
            )
            .up_to_n_times(2)
            .expect(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"id":"chatcmpl-ok","choices":[]}"#),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let config = RetryConfig {
            max_retries: 3,
            status_codes: vec![429, 500, 502, 503],
            base_backoff_ms: 100,
            max_backoff_ms: 5000,
            jitter_ms: 0,
            max_total_timeout_ms: None,
        };

        let start = std::time::Instant::now();
        let resp = robust_request(
            &client,
            Method::POST,
            &format!("{}/v1/chat/completions", mock_server.uri()),
            reqwest::header::HeaderMap::new(),
            Bytes::from(r#"{"model":"gpt-4o"}"#),
            &config,
        )
        .await
        .expect("request should succeed after retries");
        let elapsed = start.elapsed();

        assert_eq!(resp.status(), 200);
        assert!(
            elapsed.as_secs_f64() >= 1.8,
            "Elapsed {:.2}s should be >= 1.8s (two Retry-After: 1 waits)",
            elapsed.as_secs_f64()
        );
    }

    /// 502 Bad Gateway should be retried and eventually succeed.
    #[tokio::test]
    async fn test_retry_handles_502_bad_gateway() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(502).set_body_string("<html>502</html>"))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"ok"}"#))
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let config = RetryConfig {
            max_retries: 3,
            status_codes: vec![429, 500, 502, 503],
            base_backoff_ms: 10,
            max_backoff_ms: 100,
            jitter_ms: 0,
            max_total_timeout_ms: None,
        };

        let resp = robust_request(
            &client,
            Method::POST,
            &format!("{}/v1/chat/completions", mock_server.uri()),
            reqwest::header::HeaderMap::new(),
            Bytes::from("{}"),
            &config,
        )
        .await
        .expect("should succeed after 502 retries");

        assert_eq!(resp.status(), 200);
    }

    /// When all retries are exhausted, return the LAST response (not an error).
    #[tokio::test]
    async fn test_retry_exhausted_returns_last_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_string(r#"{"error":"rate limited"}"#))
            .expect(3) // 1 original + 2 retries
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let config = RetryConfig {
            max_retries: 2,
            status_codes: vec![429],
            base_backoff_ms: 10,
            max_backoff_ms: 50,
            jitter_ms: 0,
            max_total_timeout_ms: None,
        };

        let resp = robust_request(
            &client,
            Method::POST,
            &format!("{}/v1/chat/completions", mock_server.uri()),
            reqwest::header::HeaderMap::new(),
            Bytes::from("{}"),
            &config,
        )
        .await
        .expect("should return last response even on exhaustion");

        assert_eq!(
            resp.status(),
            429,
            "Should return last 429 after retries exhausted"
        );
    }
}
