use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct VolumeStat {
    pub bucket: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StatusStat {
    pub status_class: i32,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LatencyStat {
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
    pub avg: f64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenUsageBucket {
    pub bucket: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenUsageStats {
    pub total_requests: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub avg_latency_ms: f64,
    pub total_cost_usd: f64,
    pub hourly: Vec<TokenUsageBucket>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnalyticsSummary {
    pub total_requests: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub avg_latency: f64,
    pub total_cost: f64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnalyticsTimeseriesPoint {
    pub bucket: DateTime<Utc>,
    pub request_count: i64,
    pub error_count: i64,
    pub cost: f64,
    pub lat: f64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExperimentSummary {
    pub experiment_name: String,
    pub variant_name: String,
    pub total_requests: i64,
    pub avg_latency_ms: f64,
    pub total_cost_usd: f64,
    pub avg_tokens: f64,
    pub error_count: i64,
}

// ── Provider Analytics Types ──────────────────────────────────────

/// Model usage breakdown for analytics dashboard
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelUsageStat {
    pub model: String,
    pub request_count: i64,
    pub cost_usd: f64,
}

/// Spend by provider for analytics dashboard
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderSpendStat {
    pub provider: String,
    pub spend_usd: f64,
    pub rate_per_1k: f64,
}

/// Latency by provider for analytics dashboard
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderLatencyStat {
    pub provider: String,
    pub latency_ms: f64,
}

// ── Traffic Analytics Types ──────────────────────────────────────

/// Traffic timeseries point with status breakdown for the Traffic tab.
/// Provides counts by policy_result category: passed, blocked, throttled, hitl-paused.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrafficTimeseriesPoint {
    pub bucket: DateTime<Utc>,
    pub total_count: i64,
    pub passed_count: i64,
    pub throttled_count: i64,
    pub blocked_count: i64,
    pub hitl_paused_count: i64,
}

/// Latency timeseries point with percentile breakdown for the Traffic tab.
/// Provides p50, p90, p99 latency values over time buckets.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LatencyTimeseriesPoint {
    pub bucket: DateTime<Utc>,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
}

// ── Cost Analytics Types ──────────────────────────────────────

/// Budget health status for the alert strip
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BudgetHealthStatus {
    pub tokens_above_80_percent: i64,
    pub tokens_without_cap: i64,
    pub total_tokens: i64,
}

/// Spend timeseries grouped by a dimension (provider/model/token)
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpendTimeseriesPoint {
    pub bucket: DateTime<Utc>,
    pub dimension: String,
    pub spend_usd: f64,
    pub request_count: i64,
}

/// Cost efficiency by model over time
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CostEfficiencyPoint {
    pub bucket: DateTime<Utc>,
    pub model: String,
    pub cost_per_1k_tokens: f64,
}

/// Budget burn rate calculation
#[derive(Debug, Serialize, Deserialize)]
pub struct BudgetBurnRate {
    pub days_elapsed: i32,
    pub days_remaining: i32,
    pub budget_usd: f64,
    pub spent_usd: f64,
    pub percent_used: f64,
    pub needed_per_day: f64,
    pub actual_per_day: f64,
    pub on_track: bool,
}

/// Enhanced spend breakdown with cap info
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenSpendWithCap {
    pub token_id: String,
    pub token_name: String,
    pub provider: String,
    pub total_spend_usd: f64,
    pub spend_cap_usd: Option<f64>,
    pub percent_cap_used: Option<f64>,
    pub request_count: i64,
    pub cost_per_1k: f64,
}

// ── Cache Analytics Types ──────────────────────────────────────

/// Cache summary statistics for the Cache tab ribbon.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheSummaryStats {
    /// Overall cache hit rate as percentage (0-100)
    pub hit_rate: f64,
    /// Estimated cost avoided by cache hits (USD)
    pub cost_avoided_usd: f64,
    /// Total cache size in bytes (from Redis INFO)
    pub cache_size_bytes: i64,
    /// Most frequently cached model
    pub top_cached_model: Option<String>,
}

/// Cache hit rate timeseries point for the Cache tab chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CacheHitRatePoint {
    pub bucket: DateTime<Utc>,
    pub hit_rate: f64,
    pub hit_count: i64,
    pub total_count: i64,
}

/// Top cached query entry for the Cache tab table.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CachedQueryRow {
    /// Hash of the cache key (truncated for display)
    pub query_hash: String,
    /// Model associated with this cached query
    pub model: Option<String>,
    /// Number of cache hits for this query
    pub hits: i64,
    /// Time since last cache hit (human readable, calculated in frontend)
    pub last_hit_at: DateTime<Utc>,
    /// Age of the cache entry in seconds
    pub cache_age_seconds: i64,
    /// Seconds until cache expires (TTL remaining)
    pub expires_in_seconds: Option<i64>,
}

/// Model-level cache efficiency for the Cache tab.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelCacheEfficiency {
    pub model: String,
    /// Cache hit rate for this model (0-100)
    pub hit_rate: f64,
    pub total_requests: i64,
    pub cache_hits: i64,
}

/// Cache latency comparison for the Cache tab.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheLatencyComparison {
    /// Average latency for cached responses (ms)
    pub cached_latency_ms: f64,
    /// Average latency for uncached responses (ms)
    pub uncached_latency_ms: f64,
    /// Speedup factor (uncached / cached)
    pub speedup_factor: f64,
    /// Sample size for cached responses
    pub cached_sample_count: i64,
    /// Sample size for uncached responses
    pub uncached_sample_count: i64,
}

// ── Model Analytics Types (Models Tab) ──────────────────────────────────────

/// Model usage timeseries point for the Models tab.
/// Used by the Model Usage Over Time chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelUsageTimeseriesPoint {
    pub bucket: DateTime<Utc>,
    pub model: String,
    pub value: f64,
}

/// Error rate by model for the Models tab.
/// Used by the Error Rate by Model horizontal bar chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelErrorRate {
    pub model: String,
    pub error_rate: f64,
    pub total_requests: i64,
    pub error_count: i64,
}

/// Latency by model for the Models tab.
/// Used by the Model Stats table and scatter chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelLatencyStat {
    pub model: String,
    pub avg_latency_ms: f64,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
    pub sample_count: i64,
}

/// Combined model stats for the Models tab table.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelStatsRow {
    pub model: String,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub request_count: i64,
}

/// Cost vs latency scatter point for the Models tab.
/// Used by the Cost vs Latency bubble chart with proportional dot sizing.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CostLatencyScatterPoint {
    pub model: String,
    pub avg_cost_per_request: f64,
    pub avg_latency_ms: f64,
    pub total_spend_usd: f64,
    pub total_requests: i64,
}

/// A/B test lift metrics for the Models tab.
/// Shows champion vs challenger comparison.
#[derive(Debug, Serialize, Deserialize)]
pub struct ABTestLiftResponse {
    pub experiment_id: String,
    pub experiment_name: String,
    pub champion: ABTestVariant,
    pub challenger: ABTestVariant,
    pub cost_delta_percent: f64,
    pub quality_metrics: Vec<QualityMetricDelta>,
}

/// A/B test variant info.
#[derive(Debug, Serialize, Deserialize)]
pub struct ABTestVariant {
    pub model: String,
    pub weight_percent: f64,
}

/// Quality metric delta for A/B test lift.
#[derive(Debug, Serialize, Deserialize)]
pub struct QualityMetricDelta {
    pub metric: String,
    pub delta: String,
}

// ── Security Analytics Types (Security Tab) ──────────────────────────────────────

/// Security KPI summary for the Security tab ribbon.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SecuritySummaryStats {
    /// Total PII redactions (records with fields_redacted)
    pub pii_redactions: i64,
    /// Total guardrail blocks (Deny results)
    pub guardrail_blocks: i64,
    /// Total shadow mode violations
    pub shadow_violations: i64,
    /// External guardrail blocks (deny_reason contains 'external')
    pub external_blocks: i64,
}

/// Guardrail trigger count by category for the Security tab.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuardrailTriggerStat {
    /// Guardrail category (jailbreak, harmful, injection, profanity, bias, sensitive, competitor)
    pub category: String,
    /// Number of triggers for this category
    pub count: i64,
}

/// PII breakdown by pattern type for the Security tab.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PiiBreakdownStat {
    /// PII pattern type (email, api_key, cc, ssn, phone, nlp)
    pub pattern: String,
    /// Number of redactions for this pattern
    pub count: i64,
}

/// Policy action count for the Security tab.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PolicyActionStat {
    /// Policy action type (redact, content_filter, deny, rate_limit, require_approval, shadow)
    pub action: String,
    /// Number of times this action was applied
    pub count: i64,
}

/// Shadow mode policy with violation stats for the Security tab.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ShadowPolicyStat {
    /// Policy name
    pub policy_name: String,
    /// Number of shadow violations
    pub violations: i64,
    /// Token with most violations
    pub top_token: String,
    /// Policy status (Monitoring/Would block)
    pub status: String,
}

/// Data residency stats for the Security tab.
#[derive(Debug, Serialize, Deserialize)]
pub struct DataResidencyStats {
    /// Percentage of EU-routed requests
    pub eu_percent: f64,
    /// Percentage of US-routed requests
    pub us_percent: f64,
}

// ── HITL Analytics Types (HITL Tab) ──────────────────────────────────────

/// HITL KPI summary for the HITL tab ribbon.
#[derive(Debug, Serialize, Deserialize)]
pub struct HitlSummaryStats {
    /// Number of pending approval requests
    pub pending_count: i64,
    /// Average wait time in seconds for completed approvals
    pub avg_wait_seconds: f64,
    /// Approval rate as percentage (0-100)
    pub approval_rate: f64,
}

/// HITL volume timeseries point for the HITL tab chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct HitlVolumePoint {
    /// Time bucket
    pub bucket: chrono::DateTime<chrono::Utc>,
    /// Count of approved requests
    pub approved_count: i64,
    /// Count of rejected requests
    pub rejected_count: i64,
    /// Count of expired requests (proxy for escalated)
    pub expired_count: i64,
    /// Count of pending requests
    pub pending_count: i64,
}

/// HITL latency stats for the SLA card.
#[derive(Debug, Serialize, Deserialize)]
pub struct HitlLatencyStats {
    /// P50 approval time in seconds
    pub p50_seconds: f64,
    /// P90 approval time in seconds
    pub p90_seconds: f64,
    /// P99 approval time in seconds
    pub p99_seconds: f64,
    /// Average approval time in seconds
    pub avg_seconds: f64,
}

/// Rejection reason breakdown for the HITL tab.
/// Note: rejection_reason column does not exist yet, returning mock data.
#[derive(Debug, Serialize, Deserialize)]
pub struct RejectionReason {
    /// Reason for rejection
    pub reason: String,
    /// Percentage of total rejections
    pub percentage: f64,
}

// ── Error Analytics Types (Errors Tab) ──────────────────────────────────────

/// Error KPI summary for the Errors tab ribbon.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorSummaryStats {
    /// Total error count (upstream_status >= 400 OR upstream_status IS NULL)
    pub total_errors: i64,
    /// Error rate as percentage (0-100)
    pub error_rate: f64,
    /// Circuit breaker trip count (deny_reason = 'circuit_breaker_open')
    pub circuit_breaker_trips: i64,
    /// Rate limit hit count (error_type = 'rate_limit')
    pub rate_limit_hits: i64,
    /// Most common error type
    pub top_error_type: Option<String>,
    /// Prior period total errors for delta
    pub prior_total_errors: i64,
    /// Prior period total requests for delta
    pub prior_total_requests: i64,
}

/// Error timeseries point for the Errors tab chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ErrorTimeseriesPoint {
    /// Time bucket
    pub bucket: chrono::DateTime<chrono::Utc>,
    /// Timeout error count
    pub timeout_count: i64,
    /// Rate limit error count
    pub rate_limit_count: i64,
    /// Upstream 5xx error count
    pub upstream_5xx_count: i64,
    /// Circuit breaker trip count
    pub circuit_breaker_count: i64,
}

/// Error type breakdown for the Errors tab bar chart.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ErrorTypeBreakdown {
    /// Error type (rate_limit, timeout, etc.)
    pub error_type: String,
    /// Count of errors
    pub count: i64,
    /// Percentage of total errors
    pub percentage: f64,
}

/// Error log row for the Errors tab table.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ErrorLogRow {
    /// Request ID
    pub request_id: String,
    /// Token ID
    pub token_id: String,
    /// Token name (from JOIN)
    pub token_name: Option<String>,
    /// Model used
    pub model: Option<String>,
    /// Error type
    pub error_type: Option<String>,
    /// HTTP status from upstream
    pub upstream_status: Option<i16>,
    /// Response latency in ms
    pub response_latency_ms: Option<i32>,
    /// Deny reason
    pub deny_reason: Option<String>,
    /// Timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}
