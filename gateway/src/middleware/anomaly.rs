//! Anomaly Detection v1 — per-token request velocity monitoring.
//!
//! Uses a Redis sorted set as a sliding window to track request timestamps
//! per token. When the current velocity (requests per 5-minute window) exceeds
//! the rolling 24h baseline mean + 3σ, fires a webhook alert.
//!
//! Design: stateless — all state lives in Redis. Gateway nodes share the same
//! sorted sets, so horizontal scaling doesn't break detection.

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

/// Configuration for anomaly detection.
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Window size for velocity measurement (seconds). Default: 300 (5 min).
    pub window_secs: u64,
    /// Number of stddev from mean to trigger alert. Default: 3.0.
    pub sigma_threshold: f64,
    /// How many historical windows to keep for baseline (seconds). Default: 86400 (24h).
    pub baseline_secs: u64,
    /// Minimum number of data points before alerting (avoids false positives on new tokens).
    pub min_datapoints: usize,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            window_secs: 300,
            sigma_threshold: 3.0,
            baseline_secs: 86400,
            min_datapoints: 12, // ~1 hour of 5-min windows
        }
    }
}

/// Result of an anomaly check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyResult {
    pub is_anomalous: bool,
    /// Current velocity (requests in the current window).
    pub current_velocity: u64,
    /// Rolling mean velocity.
    pub baseline_mean: f64,
    /// Rolling standard deviation.
    pub baseline_stddev: f64,
    /// Threshold that was exceeded (mean + sigma_threshold * stddev).
    pub threshold: f64,
    /// Token ID being monitored.
    pub token_id: String,
}

/// Record a request timestamp and check for anomalous velocity.
///
/// Steps:
/// 1. ZADD current timestamp to sorted set `anomaly:tok:{token_id}`
/// 2. ZREMRANGEBYSCORE to remove entries older than `baseline_secs`
/// 3. ZRANGEBYSCORE for the current window to get velocity
/// 4. ZRANGEBYSCORE over the full baseline to compute historical bucket counts
/// 5. Calculate mean + stddev → compare velocity against threshold
pub async fn record_and_check(
    redis: &mut redis::aio::ConnectionManager,
    token_id: &str,
    config: &AnomalyConfig,
) -> anyhow::Result<AnomalyResult> {
    let now = chrono::Utc::now().timestamp() as f64;
    let key = format!("anomaly:tok:{}", token_id);

    // 1. Add timestamp
    let _: () = redis.zadd(&key, now, format!("{:.6}", now)).await?;

    // 2. Prune old entries (keep only baseline_secs worth of history)
    let cutoff = now - config.baseline_secs as f64;
    let _: () = redis.zrembyscore(&key, f64::NEG_INFINITY, cutoff).await?;

    // 3. Set TTL on the key (auto-cleanup if token goes idle)
    let _: () = redis
        .expire(&key, (config.baseline_secs + config.window_secs) as i64)
        .await?;

    // 4. Count requests in current window
    let window_start = now - config.window_secs as f64;
    let current_velocity: u64 = redis.zcount(&key, window_start, now).await?;

    // 5. Get all timestamps for baseline calculation
    let timestamps: Vec<f64> = redis.zrangebyscore(&key, cutoff, now).await?;

    // 6. Bucket timestamps into windows and calculate stats
    let bucket_counts =
        bucket_velocities(&timestamps, config.window_secs, now, config.baseline_secs);

    if bucket_counts.len() < config.min_datapoints {
        // Not enough data — don't alert
        return Ok(AnomalyResult {
            is_anomalous: false,
            current_velocity,
            baseline_mean: 0.0,
            baseline_stddev: 0.0,
            threshold: 0.0,
            token_id: token_id.to_string(),
        });
    }

    let (mean, stddev) = mean_stddev(&bucket_counts);
    let threshold = mean + config.sigma_threshold * stddev;
    let is_anomalous = current_velocity as f64 > threshold;

    if is_anomalous {
        tracing::warn!(
            token_id = %token_id,
            current_velocity = current_velocity,
            mean = mean,
            stddev = stddev,
            threshold = threshold,
            "Anomaly detected: velocity spike"
        );
    }

    Ok(AnomalyResult {
        is_anomalous,
        current_velocity,
        baseline_mean: mean,
        baseline_stddev: stddev,
        threshold,
        token_id: token_id.to_string(),
    })
}

/// Bucket timestamps into fixed-size windows and count requests per window.
fn bucket_velocities(
    timestamps: &[f64],
    window_secs: u64,
    now: f64,
    baseline_secs: u64,
) -> Vec<f64> {
    let num_buckets = (baseline_secs / window_secs) as usize;
    let mut buckets = vec![0.0_f64; num_buckets];

    for &ts in timestamps {
        let age = now - ts;
        let bucket_idx = (age / window_secs as f64) as usize;
        if bucket_idx < num_buckets {
            buckets[bucket_idx] += 1.0;
        }
    }

    // Only return non-zero buckets (avoid inflating mean with empty/idle periods)
    buckets.into_iter().filter(|&count| count > 0.0).collect()
}

/// Calculate mean and sample standard deviation (Bessel's correction, N-1).
///
/// 5E-1 FIX: Uses N-1 denominator for variance so we don't underestimate
/// stddev at small sample sizes (min_datapoints = 12 → ~4.2% correction).
fn mean_stddev(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if n <= 1.0 {
        // With 0 or 1 data points, stddev is undefined / zero
        let mean = if n == 1.0 { values[0] } else { 0.0 };
        return (mean, 0.0);
    }

    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let stddev = variance.sqrt();

    (mean, stddev)
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_stddev_basic() {
        let values = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let (mean, stddev) = mean_stddev(&values);
        assert!((mean - 10.0).abs() < f64::EPSILON);
        assert!(stddev.abs() < f64::EPSILON); // No variation
    }

    #[test]
    fn test_mean_stddev_with_variation() {
        let values = vec![10.0, 12.0, 8.0, 14.0, 6.0];
        let (mean, stddev) = mean_stddev(&values);
        assert!((mean - 10.0).abs() < f64::EPSILON);
        assert!(stddev > 0.0);
    }

    #[test]
    fn test_mean_stddev_empty() {
        let values: Vec<f64> = vec![];
        let (mean, stddev) = mean_stddev(&values);
        assert!((mean - 0.0).abs() < f64::EPSILON);
        assert!((stddev - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bucket_velocities() {
        let now = 1000.0;
        // bucket_idx = floor(age / window_secs)
        // age 1,2,3 → idx 0; age 5,6 → idx 1; age 10,11 → idx 2
        let timestamps = vec![
            999.0, 998.0, 997.0, // age 1,2,3 → bucket 0
            995.0, 994.0, // age 5,6 → bucket 1
            990.0, 989.0, // age 10,11 → bucket 2
        ];
        let buckets = bucket_velocities(&timestamps, 5, now, 20);
        assert_eq!(buckets.len(), 3); // 3 non-zero buckets (empty bucket filtered out)
        assert_eq!(buckets[0], 3.0); // 999, 998, 997
        assert_eq!(buckets[1], 2.0); // 995, 994
        assert_eq!(buckets[2], 2.0); // 990, 989
    }

    #[test]
    fn test_anomaly_config_default() {
        let config = AnomalyConfig::default();
        assert_eq!(config.window_secs, 300);
        assert_eq!(config.sigma_threshold, 3.0);
        assert_eq!(config.baseline_secs, 86400);
        assert_eq!(config.min_datapoints, 12);
    }

    #[test]
    fn test_anomaly_result_detects_spike() {
        // Simulate: baseline of 10 req/window, current = 100
        let values = vec![10.0; 100]; // stable baseline
        let (mean, stddev) = mean_stddev(&values);
        let threshold = mean + 3.0 * stddev;
        // With zero stddev, threshold = mean = 10.0
        // 100 > 10.0 → anomalous
        assert!(100.0 > threshold);
    }
}
