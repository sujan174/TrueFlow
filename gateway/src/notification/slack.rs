use anyhow::Context;
use serde::Serialize;

#[derive(Clone)]
pub struct SlackNotifier {
    client: reqwest::Client,
    webhook_url: Option<String>,
}

impl SlackNotifier {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            webhook_url,
        }
    }

    pub async fn send_approval_request(
        &self,
        approval_id: &uuid::Uuid,
        summary: &serde_json::Value,
        expires_at: &chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<()> {
        let url = match &self.webhook_url {
            Some(u) => u,
            None => {
                // If no webhook configured, do nothing (log debug)
                tracing::debug!("No Slack webhook URL configured, skipping notification");
                return Ok(());
            }
        };

        let message = SlackMessage {
            text: format!("🚨 *Human Approval Required* 🚨\n\nRequest ID: `{}`\nExpires: {}\nSummary:\n```{}```\n\nRun `trueflow approval approve {}` or `trueflow approval reject {}`",
                approval_id, expires_at, serde_json::to_string_pretty(summary).unwrap_or_default(), approval_id, approval_id
            ),
        };

        let resp = self
            .client
            .post(url)
            .json(&message)
            .send()
            .await
            .context("failed to send slack notification")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("slack returned error: status={}, body={}", status, body);
        }

        tracing::info!("Sent Slack notification for approval {}", approval_id);
        Ok(())
    }
}

#[derive(Serialize)]
struct SlackMessage {
    text: String,
}
