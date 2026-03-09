use anyhow::Result;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::time::Duration;
use tracing::{debug, info, warn};

// ── Webhook Event Types ───────────────────────────────────────

/// A structured event payload sent to webhook endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookEvent {
    /// Event type identifier, e.g. "policy_violation", "rate_limit_exceeded".
    pub event_type: String,
    /// ISO-8601 timestamp of when the event occurred.
    pub timestamp: String,
    /// The token that triggered the event.
    pub token_id: String,
    /// Human-readable token name.
    pub token_name: String,
    /// Project ID the token belongs to.
    pub project_id: String,
    /// Event-specific details (policy name, reason, limits, etc.).
    pub details: serde_json::Value,
}

impl WebhookEvent {
    pub fn policy_violation(
        token_id: &str,
        token_name: &str,
        project_id: &str,
        policy_name: &str,
        reason: &str,
    ) -> Self {
        Self {
            event_type: "policy_violation".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            token_id: token_id.to_string(),
            token_name: token_name.to_string(),
            project_id: project_id.to_string(),
            details: serde_json::json!({
                "policy": policy_name,
                "reason": reason,
            }),
        }
    }

    pub fn rate_limit_exceeded(
        token_id: &str,
        token_name: &str,
        project_id: &str,
        policy_name: &str,
        max_requests: u64,
        window_secs: u64,
    ) -> Self {
        Self {
            event_type: "rate_limit_exceeded".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            token_id: token_id.to_string(),
            token_name: token_name.to_string(),
            project_id: project_id.to_string(),
            details: serde_json::json!({
                "policy": policy_name,
                "max_requests": max_requests,
                "window_secs": window_secs,
            }),
        }
    }

    pub fn spend_cap_exceeded(
        token_id: &str,
        token_name: &str,
        project_id: &str,
        reason: &str,
    ) -> Self {
        Self {
            event_type: "spend_cap_exceeded".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            token_id: token_id.to_string(),
            token_name: token_name.to_string(),
            project_id: project_id.to_string(),
            details: serde_json::json!({ "reason": reason }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn approval_requested(
        token_id: &str,
        token_name: &str,
        project_id: &str,
        approval_id: &str,
        method: &str,
        path: &str,
        upstream: &str,
        full_body: Option<serde_json::Value>,
    ) -> Self {
        Self {
            event_type: "approval_requested".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            token_id: token_id.to_string(),
            token_name: token_name.to_string(),
            project_id: project_id.to_string(),
            details: serde_json::json!({
                "approval_id": approval_id,
                "method": method,
                "path": path,
                "upstream": upstream,
                "full_body": full_body,
            }),
        }
    }

    /// Anomaly detection alert — triggered when request velocity exceeds baseline.
    pub fn anomaly_detected(
        token_id: &str,
        token_name: &str,
        project_id: &str,
        current_velocity: u64,
        baseline_mean: f64,
        threshold: f64,
    ) -> Self {
        Self {
            event_type: "anomaly_detected".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            token_id: token_id.to_string(),
            token_name: token_name.to_string(),
            project_id: project_id.to_string(),
            details: serde_json::json!({
                "current_velocity": current_velocity,
                "baseline_mean": baseline_mean,
                "threshold": threshold,
                "severity": if current_velocity as f64 > threshold * 2.0 { "critical" } else { "warning" },
            }),
        }
    }
}

// ── HMAC Signing ─────────────────────────────────────────────

/// Compute HMAC-SHA256 of `payload` using `secret`.
/// Returns lowercase hex digest (e.g. "sha256=<hex>").
fn hmac_sha256_hex(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    let result = mac.finalize();
    let bytes = result.into_bytes();
    format!("sha256={}", hex::encode(bytes))
}

// ── Webhook Notifier ──────────────────────────────────────────

/// Dispatches webhook events to one or more configured URLs.
/// Supports:
/// - HMAC-SHA256 signing (X-TrueFlow-Signature header)
/// - Up to 3 retries with exponential back-off (1s → 5s → 25s)
#[derive(Clone)]
pub struct WebhookNotifier {
    client: reqwest::Client,
}

impl WebhookNotifier {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("TrueFlow-Webhook/1.0")
                .build()
                .expect("failed to build webhook HTTP client"),
        }
    }

    /// Send a signed webhook event to a single URL with retry.
    ///
    /// If `signing_secret` is `Some`, the request body is signed with HMAC-SHA256
    /// and the signature is sent in the `X-TrueFlow-Signature` header.
    ///
    /// Retries up to 3 times on failure with exponential back-off.
    /// Returns `Ok(())` if delivery succeeded on any attempt.
    pub async fn send_signed(
        &self,
        url: &str,
        event: &WebhookEvent,
        signing_secret: Option<&str>,
    ) -> Result<()> {
        let payload = serde_json::to_vec(event)
            .map_err(|e| anyhow::anyhow!("webhook serialize error: {}", e))?;
        let delivery_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let signature = signing_secret.map(|s| hmac_sha256_hex(s, &payload));

        let backoff_secs: &[u64] = &[0, 1, 5, 25];

        for (attempt, &delay) in backoff_secs.iter().enumerate() {
            if delay > 0 {
                tracing::debug!(
                    url,
                    attempt,
                    delay_secs = delay,
                    event_type = %event.event_type,
                    "retrying webhook delivery"
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }

            let mut req = self
                .client
                .post(url)
                .header("content-type", "application/json")
                .header("x-trueflow-delivery-id", &delivery_id)
                .header("x-trueflow-timestamp", &timestamp)
                .header("x-trueflow-event", &event.event_type);

            if let Some(ref sig) = signature {
                req = req.header("x-trueflow-signature", sig.as_str());
            }

            let result = req.body(payload.clone()).send().await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    info!(
                        url,
                        event_type = %event.event_type,
                        delivery_id = %delivery_id,
                        attempt,
                        status = %resp.status(),
                        "webhook delivered successfully"
                    );
                    return Ok(());
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warn!(
                        url,
                        event_type = %event.event_type,
                        delivery_id = %delivery_id,
                        attempt,
                        status = %status,
                        body = %body,
                        "webhook delivery failed (non-2xx), will retry"
                    );
                }
                Err(e) => {
                    warn!(
                        url,
                        event_type = %event.event_type,
                        delivery_id = %delivery_id,
                        attempt,
                        error = %e,
                        "webhook request error, will retry"
                    );
                }
            }
        }

        // All attempts exhausted
        warn!(
            url,
            event_type = %event.event_type,
            delivery_id = %delivery_id,
            "webhook delivery failed after all retries"
        );
        Err(anyhow::anyhow!(
            "webhook delivery failed after 3 retries: {}",
            url
        ))
    }

    /// Send without signing (backwards compat for env-var driven config webhooks).
    pub async fn send(&self, url: &str, event: &WebhookEvent) -> Result<()> {
        self.send_signed(url, event, None).await
    }

    /// Dispatch an event to all configured webhook URLs (fire-and-forget).
    ///
    /// Each URL is attempted independently with retry; failures in one do not block others.
    pub async fn dispatch(&self, urls: &[String], event: WebhookEvent) {
        if urls.is_empty() {
            return;
        }

        let notifier = self.clone();
        let urls = urls.to_vec();

        tokio::spawn(async move {
            for url in &urls {
                if let Err(e) = notifier.send(url, &event).await {
                    warn!(url, error = %e, "webhook dispatch ultimately failed");
                }
            }
        });
    }

    /// Dispatch a signed event to per-webhook DB records (URL + optional signing secret).
    pub async fn dispatch_signed(&self, targets: &[(String, Option<String>)], event: WebhookEvent) {
        if targets.is_empty() {
            debug!("dispatch_signed: no webhook targets, skipping");
            return;
        }

        let notifier = self.clone();
        let targets = targets.to_vec();

        tokio::spawn(async move {
            for (url, secret) in &targets {
                if let Err(e) = notifier.send_signed(url, &event, secret.as_deref()).await {
                    warn!(url, error = %e, "signed webhook dispatch ultimately failed");
                }
            }
        });
    }
}

impl Default for WebhookNotifier {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_violation_event_type() {
        let event =
            WebhookEvent::policy_violation("tok1", "my-token", "proj1", "deny-all", "blocked");
        assert_eq!(event.event_type, "policy_violation");
        assert_eq!(event.token_id, "tok1");
        assert_eq!(event.details["policy"], "deny-all");
        assert_eq!(event.details["reason"], "blocked");
    }

    #[test]
    fn test_rate_limit_event_type() {
        let event =
            WebhookEvent::rate_limit_exceeded("tok1", "my-token", "proj1", "rl-policy", 100, 60);
        assert_eq!(event.event_type, "rate_limit_exceeded");
        assert_eq!(event.details["max_requests"], 100);
        assert_eq!(event.details["window_secs"], 60);
    }

    #[test]
    fn test_spend_cap_event_type() {
        let event =
            WebhookEvent::spend_cap_exceeded("tok1", "my-token", "proj1", "daily cap exceeded");
        assert_eq!(event.event_type, "spend_cap_exceeded");
        assert_eq!(event.details["reason"], "daily cap exceeded");
    }

    #[test]
    fn test_event_serializes_to_json() {
        let event = WebhookEvent::policy_violation("t", "n", "p", "pol", "reason");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("policy_violation"));
        assert!(json.contains("timestamp"));
    }

    #[test]
    fn test_hmac_signature_deterministic() {
        let sig1 = hmac_sha256_hex("secret123", b"payload");
        let sig2 = hmac_sha256_hex("secret123", b"payload");
        assert_eq!(sig1, sig2);
        assert!(sig1.starts_with("sha256="));
    }

    #[test]
    fn test_hmac_signature_different_secret() {
        let sig1 = hmac_sha256_hex("secret1", b"payload");
        let sig2 = hmac_sha256_hex("secret2", b"payload");
        assert_ne!(sig1, sig2);
    }
}
