//! DataDog StatsD metrics exporter for TrueFlow Gateway.
//!
//! Sends metrics via DogStatsD protocol (UDP) to the DataDog Agent.
//!
//! Config env vars:
//!   DD_ENABLED      = true
//!   DD_AGENT_HOST   = localhost (default)
//!   DD_AGENT_PORT   = 8125 (default)
//!   DD_CUSTOM_TAGS  = team,env (custom_properties keys to include as DD tags)

use crate::models::audit::AuditEntry;
use cadence::prelude::*;
use cadence::{BufferedUdpMetricSink, StatsdClient, QueuingMetricSink};
use rust_decimal::prelude::ToPrimitive;
use std::net::UdpSocket;

#[allow(dead_code)]
pub struct DataDogExporter {
    client: StatsdClient,
    custom_tag_keys: Vec<String>,
}

impl DataDogExporter {
    /// Create from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        let host = std::env::var("DD_AGENT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port: u16 = std::env::var("DD_AGENT_PORT")
            .unwrap_or_else(|_| "8125".to_string())
            .parse()
            .unwrap_or(8125);

        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;

        let udp_sink = BufferedUdpMetricSink::from(
            format!("{}:{}", host, port),
            socket,
        )?;
        let queuing_sink = QueuingMetricSink::from(udp_sink);
        let client = StatsdClient::from_sink("trueflow", queuing_sink);

        let custom_tag_keys: Vec<String> = std::env::var("DD_CUSTOM_TAGS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        Ok(Self {
            client,
            custom_tag_keys,
        })
    }

    /// Emit metrics for a completed request via DogStatsD.
    pub fn emit(&self, entry: &AuditEntry) {
        let model = entry.model.as_deref().unwrap_or("unknown");
        let status = entry
            .upstream_status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "0".to_string());

        // Extract custom tag values from entry.custom_properties for DD_CUSTOM_TAGS keys
        let custom_tags: Vec<(String, String)> = self
            .custom_tag_keys
            .iter()
            .filter_map(|key| {
                entry
                    .custom_properties
                    .as_ref()
                    .and_then(|p| p.get(key))
                    .and_then(|v| v.as_str())
                    .map(|val| (key.clone(), val.to_string()))
            })
            .collect();

        // Request counter
        {
            let mut builder = self
                .client
                .count_with_tags("request.count", 1)
                .with_tag("model", model)
                .with_tag("status", &status)
                .with_tag("cache_hit", if entry.cache_hit { "true" } else { "false" });
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }

        // Request duration (histogram via timer)
        {
            let duration_ms = entry.response_latency_ms;
            let mut builder = self
                .client
                .time_with_tags("request.duration", duration_ms)
                .with_tag("model", model)
                .with_tag("status", &status);
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }

        // Token counters
        if let Some(prompt) = entry.prompt_tokens {
            let mut builder = self
                .client
                .count_with_tags("tokens", prompt as i64)
                .with_tag("model", model)
                .with_tag("type", "prompt");
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }
        if let Some(completion) = entry.completion_tokens {
            let mut builder = self
                .client
                .count_with_tags("tokens", completion as i64)
                .with_tag("model", model)
                .with_tag("type", "completion");
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }

        // Cost counter (in millicents to avoid float precision issues)
        if let Some(cost) = entry.estimated_cost_usd {
            if let Some(cost_f64) = cost.to_f64() {
                let millicents = (cost_f64 * 100_000.0).round() as i64;
                let mut builder = self
                    .client
                    .count_with_tags("cost.millicents", millicents)
                    .with_tag("model", model);
                for (k, v) in &custom_tags {
                    builder = builder.with_tag(k, v);
                }
                let _ = builder.try_send();
            }
        }

        // Error counter
        if let Some(error_type) = &entry.error_type {
            let mut builder = self
                .client
                .count_with_tags("errors", 1)
                .with_tag("model", model)
                .with_tag("error_type", error_type);
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }

        // TTFT (streaming)
        if let Some(ttft_ms) = entry.ttft_ms {
            let mut builder = self
                .client
                .time_with_tags("ttft", ttft_ms)
                .with_tag("model", model);
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }

        // Cache hit counter
        if entry.cache_hit {
            let mut builder = self
                .client
                .count_with_tags("cache.hits", 1)
                .with_tag("model", model);
            for (k, v) in &custom_tags {
                builder = builder.with_tag(k, v);
            }
            let _ = builder.try_send();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_custom_tag_key_parsing() {
        // Simulate parsing DD_CUSTOM_TAGS
        let input = "team, env, feature";
        let keys: Vec<String> = input
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        assert_eq!(keys, vec!["team", "env", "feature"]);
    }

    #[test]
    fn test_empty_custom_tags() {
        let input = "";
        let keys: Vec<String> = input
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_millicent_conversion() {
        let cost_usd = 0.0045_f64;
        let millicents = (cost_usd * 100_000.0).round() as i64;
        assert_eq!(millicents, 450);
    }
}
