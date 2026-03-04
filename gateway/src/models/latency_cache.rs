//! In-memory latency cache backed by `audit_logs`.
//!
//! Stores the p50 response latency per model, refreshed every 5 minutes
//! by a background job in `main.rs`. Used by the smart router's
//! `lowest_latency` strategy to rank candidates.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared, cheaply-cloneable latency cache.
#[derive(Clone)]
pub struct LatencyCache(Arc<RwLock<HashMap<String, f64>>>);

impl Default for LatencyCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyCache {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    /// Reload latency data from `audit_logs` for the last 24 hours.
    /// Groups by the `model` field in the response body and computes p50.
    pub async fn reload(&self, pool: &sqlx::PgPool) {
        match fetch_latency_p50(pool).await {
            Ok(map) => {
                *self.0.write().await = map;
                tracing::debug!("latency_cache: reloaded {} model entries", {
                    self.0.read().await.len()
                });
            }
            Err(e) => {
                tracing::error!("latency_cache: reload failed: {}", e);
            }
        }
    }

    /// Get the p50 latency in ms for a model. Returns `None` if unknown.
    pub async fn get_p50(&self, model: &str) -> Option<f64> {
        self.0.read().await.get(model).copied()
    }

    /// Return all entries (for diagnostics).
    pub async fn all(&self) -> HashMap<String, f64> {
        self.0.read().await.clone()
    }
}

/// Query audit_logs for p50 latency per model over the last 24 hours.
async fn fetch_latency_p50(pool: &sqlx::PgPool) -> anyhow::Result<HashMap<String, f64>> {
    #[derive(sqlx::FromRow)]
    struct LatencyRow {
        model: String,
        p50: f64,
    }

    let rows = sqlx::query_as::<_, LatencyRow>(
        r#"
        SELECT
            COALESCE(response_model, model, 'unknown') AS model,
            PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY response_latency_ms)::float8 AS p50
        FROM audit_logs
        WHERE
            created_at >= NOW() - INTERVAL '24 hours'
            AND response_latency_ms IS NOT NULL
            AND response_latency_ms > 0
        GROUP BY COALESCE(response_model, model, 'unknown')
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::with_capacity(rows.len());
    for row in rows {
        map.insert(row.model, row.p50);
    }

    Ok(map)
}
