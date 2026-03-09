//! ObserverHub — unified fan-out for all observability exporters.
//!
//! Called from `AuditBuilder::emit()` to dispatch request telemetry to
//! Prometheus, Langfuse, and/or DataDog in a single call.

use crate::models::audit::AuditEntry;

pub struct ObserverHub {
    pub prometheus: Option<super::metrics::PrometheusRecorder>,
    pub langfuse: Option<super::langfuse::LangfuseExporter>,
    pub datadog: Option<super::datadog::DataDogExporter>,
}

impl ObserverHub {
    /// Create a new ObserverHub from environment configuration.
    pub fn from_env() -> Self {
        let prometheus = Some(super::metrics::PrometheusRecorder::new());

        let langfuse = if std::env::var("LANGFUSE_PUBLIC_KEY").is_ok() {
            match super::langfuse::LangfuseExporter::from_env() {
                Ok(lf) => {
                    tracing::info!("Langfuse exporter enabled");
                    Some(lf)
                }
                Err(e) => {
                    tracing::warn!("Langfuse config error, disabling: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let datadog = if std::env::var("DD_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
        {
            match super::datadog::DataDogExporter::from_env() {
                Ok(dd) => {
                    tracing::info!("DataDog StatsD exporter enabled");
                    Some(dd)
                }
                Err(e) => {
                    tracing::warn!("DataDog config error, disabling: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            prometheus,
            langfuse,
            datadog,
        }
    }

    /// Record a completed request across all configured exporters.
    /// This is designed to be non-blocking — exporters must not panic.
    pub fn record(&self, entry: &AuditEntry) {
        if let Some(prom) = &self.prometheus {
            prom.record(entry);
        }
        if let Some(lf) = &self.langfuse {
            lf.export_async(entry);
        }
        if let Some(dd) = &self.datadog {
            dd.emit(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observer_hub_creation_without_env_vars() {
        // Should not panic even with no env vars set
        let hub = ObserverHub::from_env();
        assert!(hub.prometheus.is_some(), "Prometheus always enabled");
        assert!(
            hub.langfuse.is_none(),
            "Langfuse should be None without env"
        );
        assert!(hub.datadog.is_none(), "DataDog should be None without env");
    }
}
