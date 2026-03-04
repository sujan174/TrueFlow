//! Langfuse trace exporter for TrueFlow Gateway.
//!
//! Sends LLM generation traces to Langfuse's ingestion API.
//! Supports batching (up to 50 events per flush, every 5 seconds).
//!
//! Config env vars:
//!   LANGFUSE_HOST          = https://cloud.langfuse.com (or self-hosted)
//!   LANGFUSE_PUBLIC_KEY    = pk-lf-...
//!   LANGFUSE_SECRET_KEY    = sk-lf-...
//!   LANGFUSE_FLUSH_INTERVAL_MS = 5000 (default)

use crate::models::audit::AuditEntry;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Langfuse generation event payload.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LangfuseGeneration {
    id: String,
    trace_id: String,
    name: String,
    model: Option<String>,
    start_time: String,
    end_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    usage: LangfuseUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_message: Option<String>,
    level: String,
}

/// Langfuse trace event — parent container for generations.
/// Required so Langfuse UI can group generations by session/agent.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LangfuseTrace {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
    timestamp: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LangfuseUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Serialize)]
struct LangfuseIngestion {
    batch: Vec<LangfuseIngestionEvent>,
}

#[derive(Debug, Serialize)]
struct LangfuseIngestionEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    timestamp: String,
    body: serde_json::Value,
}

pub struct LangfuseExporter {
    host: String,
    public_key: String,
    secret_key: String,
    client: reqwest::Client,
    buffer: Arc<Mutex<Vec<LangfuseIngestionEvent>>>,
    max_batch_size: usize,
}

impl LangfuseExporter {
    /// Create from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        let host = std::env::var("LANGFUSE_HOST")
            .unwrap_or_else(|_| "https://cloud.langfuse.com".to_string());
        let public_key = std::env::var("LANGFUSE_PUBLIC_KEY")
            .map_err(|_| anyhow::anyhow!("LANGFUSE_PUBLIC_KEY not set"))?;
        let secret_key = std::env::var("LANGFUSE_SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("LANGFUSE_SECRET_KEY not set"))?;

        let exporter = Self {
            host,
            public_key,
            secret_key,
            client: reqwest::Client::new(),
            buffer: Arc::new(Mutex::new(Vec::with_capacity(50))),
            max_batch_size: 50,
        };

        // Start background flush task
        let buffer = exporter.buffer.clone();
        let flush_host = exporter.host.clone();
        let flush_pk = exporter.public_key.clone();
        let flush_sk = exporter.secret_key.clone();
        let flush_client = exporter.client.clone();
        let flush_interval_ms: u64 = std::env::var("LANGFUSE_FLUSH_INTERVAL_MS")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()
            .unwrap_or(5000);

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_millis(flush_interval_ms));
            loop {
                interval.tick().await;
                let events: Vec<LangfuseIngestionEvent> = {
                    let mut buf = buffer.lock().await;
                    if buf.is_empty() {
                        continue;
                    }
                    buf.drain(..).collect()
                };
                if let Err(e) = Self::flush_batch(
                    &flush_client,
                    &flush_host,
                    &flush_pk,
                    &flush_sk,
                    events,
                )
                .await
                {
                    tracing::warn!("Langfuse flush failed: {}", e);
                }
            }
        });

        Ok(exporter)
    }

    /// Queue a trace + generation event pair for async batched export.
    /// The trace groups generations by session/agent in the Langfuse UI.
    pub fn export_async(&self, entry: &AuditEntry) {
        let prompt_tokens = entry.prompt_tokens.unwrap_or(0);
        let completion_tokens = entry.completion_tokens.unwrap_or(0);
        let end_time = entry.timestamp;
        let start_time =
            end_time - chrono::Duration::milliseconds(entry.response_latency_ms as i64);

        // Use session_id as trace grouping key when available,
        // otherwise each request gets its own trace.
        let trace_id = entry
            .session_id
            .clone()
            .unwrap_or_else(|| entry.request_id.to_string());

        let generation = LangfuseGeneration {
            id: format!("gen-{}", entry.request_id),
            trace_id: trace_id.clone(),
            name: entry
                .model
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            model: entry.model.clone(),
            start_time: start_time.to_rfc3339(),
            end_time: end_time.to_rfc3339(),
            input: entry.request_body.clone(),
            output: entry.response_body.clone(),
            usage: LangfuseUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            metadata: entry.custom_properties.clone(),
            status_message: entry.error_type.clone(),
            level: if entry.error_type.is_some() {
                "ERROR".to_string()
            } else {
                "DEFAULT".to_string()
            },
        };

        // Build parent trace so Langfuse groups generations by session/agent
        let trace = LangfuseTrace {
            id: trace_id,
            name: entry
                .agent_name
                .clone()
                .unwrap_or_else(|| "trueflow-proxy".to_string()),
            session_id: entry.session_id.clone(),
            user_id: entry.user_id.clone(),
            metadata: entry.custom_properties.clone(),
            timestamp: start_time.to_rfc3339(),
        };

        let ts = end_time.to_rfc3339();
        let trace_event = LangfuseIngestionEvent {
            id: format!("trace-{}", entry.request_id),
            event_type: "trace-create".to_string(),
            timestamp: ts.clone(),
            body: serde_json::to_value(&trace).unwrap_or_default(),
        };
        let gen_event = LangfuseIngestionEvent {
            id: generation.id.clone(),
            event_type: "generation-create".to_string(),
            timestamp: ts,
            body: serde_json::to_value(&generation).unwrap_or_default(),
        };

        let buffer = self.buffer.clone();
        let max_batch = self.max_batch_size;
        let host = self.host.clone();
        let pk = self.public_key.clone();
        let sk = self.secret_key.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            let should_flush = {
                let mut buf = buffer.lock().await;
                buf.push(trace_event);
                buf.push(gen_event);
                buf.len() >= max_batch
            };

            // Flush immediately if buffer is full
            if should_flush {
                let events: Vec<LangfuseIngestionEvent> = {
                    let mut buf = buffer.lock().await;
                    buf.drain(..).collect()
                };
                if let Err(e) = Self::flush_batch(&client, &host, &pk, &sk, events).await {
                    tracing::warn!("Langfuse flush failed: {}", e);
                }
            }
        });
    }

    async fn flush_batch(
        client: &reqwest::Client,
        host: &str,
        public_key: &str,
        secret_key: &str,
        batch: Vec<LangfuseIngestionEvent>,
    ) -> anyhow::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let count = batch.len();
        let payload = LangfuseIngestion { batch };

        let resp = client
            .post(format!("{}/api/public/ingestion", host))
            .basic_auth(public_key, Some(secret_key))
            .json(&payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::debug!("Langfuse: flushed {} events", count);
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("Langfuse ingest failed ({}): {}", status, body);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_langfuse_generation_serialization() {
        let gen = LangfuseGeneration {
            id: "gen-123".into(),
            trace_id: "trace-456".into(),
            name: "gpt-4o".into(),
            model: Some("gpt-4o".into()),
            start_time: "2026-01-01T00:00:00Z".into(),
            end_time: "2026-01-01T00:00:01Z".into(),
            input: None,
            output: None,
            usage: LangfuseUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            metadata: None,
            status_message: None,
            level: "DEFAULT".into(),
        };

        let json = serde_json::to_value(&gen).unwrap();
        assert_eq!(json["traceId"], "trace-456");
        assert_eq!(json["usage"]["promptTokens"], 100);
        assert_eq!(json["usage"]["totalTokens"], 150);
        assert!(json.get("input").is_none()); // skip_serializing_if = None
    }

    #[test]
    fn test_ingestion_event_type() {
        let gen = LangfuseGeneration {
            id: "gen-1".into(),
            trace_id: "t1".into(),
            name: "test".into(),
            model: None,
            start_time: "2026-01-01T00:00:00Z".into(),
            end_time: "2026-01-01T00:00:01Z".into(),
            input: None,
            output: None,
            usage: LangfuseUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            metadata: None,
            status_message: None,
            level: "DEFAULT".into(),
        };

        let event = LangfuseIngestionEvent {
            id: "gen-1".into(),
            event_type: "generation-create".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            body: serde_json::to_value(&gen).unwrap(),
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "generation-create");
        // Verify nested generation body is present
        assert_eq!(json["body"]["traceId"], "t1");
    }

    #[test]
    fn test_trace_create_event() {
        let trace = LangfuseTrace {
            id: "session-abc".into(),
            name: "my-agent".into(),
            session_id: Some("session-abc".into()),
            user_id: Some("user-1".into()),
            metadata: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
        };

        let event = LangfuseIngestionEvent {
            id: "trace-req-1".into(),
            event_type: "trace-create".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            body: serde_json::to_value(&trace).unwrap(),
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "trace-create");
        assert_eq!(json["body"]["sessionId"], "session-abc");
        assert_eq!(json["body"]["name"], "my-agent");
        assert_eq!(json["body"]["userId"], "user-1");
    }
}
