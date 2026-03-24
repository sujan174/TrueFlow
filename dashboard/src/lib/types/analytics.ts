// Analytics types matching gateway models

// Re-export TokenRow from token.ts for consistency
export type { TokenRow } from "./token"

export interface AnalyticsSummary {
  total_requests: number
  success_count: number
  error_count: number
  avg_latency: number
  total_cost: number
  total_tokens: number
}

export interface AnalyticsTimeseriesPoint {
  bucket: string
  request_count: number
  error_count: number
  cost: number
  lat: number
}

export interface LatencyStat {
  p50: number
  p90: number
  p99: number
  avg: number
}

export interface TokenSummary {
  token_id: string
  total_requests: number
  errors: number
  avg_latency_ms: number
  last_active: string | null
}

export interface SpendCap {
  token_id: string
  token_name: string
  spend_used_usd: number | null
  spend_cap_usd: number
}

export interface UpstreamStatus {
  name: string
  healthy: boolean
  last_check: string
  latency_ms: number | null
  error: string | null
}

// Provider analytics types
export interface ModelUsageStat {
  model: string
  request_count: number
  cost_usd: number
}

export interface ProviderSpendStat {
  provider: string
  spend_usd: number
  rate_per_1k: number
}

export interface ProviderLatencyStat {
  provider: string
  latency_ms: number
}

// Traffic analytics types (Traffic Tab)
export interface TrafficTimeseriesPoint {
  bucket: string
  total_count: number
  passed_count: number
  throttled_count: number
  blocked_count: number
  hitl_paused_count: number
}

export interface LatencyTimeseriesPoint {
  bucket: string
  p50: number
  p90: number
  p99: number
}

export interface AuditLogRow {
  request_id: string
  token_id: string
  model: string | null
  method: string
  response_latency_ms: number
  upstream_status: number | null
  policy_result: string
  estimated_cost_usd: number | null
  created_at: string
}

// Cost analytics types (Cost Tab)

export interface BudgetHealthStatus {
  tokens_above_80_percent: number
  tokens_without_cap: number
  total_tokens: number
}

export interface SpendTimeseriesPoint {
  bucket: string
  dimension: string
  spend_usd: number
  request_count: number
}

export interface CostEfficiencyPoint {
  bucket: string
  model: string
  cost_per_1k_tokens: number
}

export interface BudgetBurnRate {
  days_elapsed: number
  days_remaining: number
  budget_usd: number
  spent_usd: number
  percent_used: number
  needed_per_day: number
  actual_per_day: number
  on_track: boolean
}

export interface TokenSpendWithCap {
  token_id: string
  token_name: string
  provider: string
  total_spend_usd: number
  spend_cap_usd: number | null
  percent_cap_used: number | null
  request_count: number
  cost_per_1k: number
}

// Spend breakdown types
export interface SpendByDimension {
  dimension: string
  total_cost_usd: number
  request_count: number
  total_prompt_tokens: number
  total_completion_tokens: number
}

// ── Users & Tokens Analytics Types ─────────────────────────────────────

/// User growth timeseries point for the Users & Tokens tab.
export interface UserGrowthPoint {
  bucket: string
  new_users: number
  cumulative_users: number
}

/// Engagement tier breakdown for the Users & Tokens tab.
export interface EngagementTiersResponse {
  power_users: number
  regular_users: number
  light_users: number
  total_users: number
}

/// A token that is at or approaching rate limit.
export interface RateLimitedToken {
  token_name: string
  percent: number
}

/// Token alerts for the Users & Tokens tab.
export interface TokenAlertsResponse {
  active_tokens: number
  token_limit: number | null
  tokens_at_rate_limit: number
  rate_limited_tokens: RateLimitedToken[]
}

/// Requests per user timeseries point for the Users & Tokens tab.
export interface RequestsPerUserPoint {
  bucket: string
  user_count: number
  request_count: number
  avg_per_user: number
}

// ── Cache Analytics Types ─────────────────────────────────────

/// Cache summary statistics for the Cache tab ribbon.
export interface CacheSummaryStats {
  hit_rate: number
  cost_avoided_usd: number
  cache_size_bytes: number
  top_cached_model: string | null
}

/// Cache hit rate timeseries point for the Cache tab chart.
export interface CacheHitRatePoint {
  bucket: string
  hit_rate: number
  hit_count: number
  total_count: number
}

/// Top cached query entry for the Cache tab table.
export interface CachedQueryRow {
  query_hash: string
  model: string | null
  hits: number
  last_hit_at: string
  cache_age_seconds: number
  expires_in_seconds: number | null
}

/// Model-level cache efficiency for the Cache tab.
export interface ModelCacheEfficiency {
  model: string
  hit_rate: number
  total_requests: number
  cache_hits: number
}

/// Cache latency comparison for the Cache tab.
export interface CacheLatencyComparison {
  cached_latency_ms: number
  uncached_latency_ms: number
  speedup_factor: number
  cached_sample_count: number
  uncached_sample_count: number
}

// ── Model Analytics Types (Models Tab) ─────────────────────────────────────

/// Model usage timeseries point for the Models tab.
export interface ModelUsageTimeseriesPoint {
  bucket: string
  model: string
  value: number
}

/// Error rate by model for the Models tab.
export interface ModelErrorRate {
  model: string
  error_rate: number
  total_requests: number
  error_count: number
}

/// Latency by model for the Models tab.
export interface ModelLatencyStat {
  model: string
  avg_latency_ms: number
  p50: number
  p90: number
  p99: number
  sample_count: number
}

/// Combined model stats for the Models tab table.
export interface ModelStatsRow {
  model: string
  total_tokens: number
  total_cost_usd: number
  error_rate: number
  avg_latency_ms: number
  request_count: number
}

/// Cost vs latency scatter point for the Models tab bubble chart.
export interface CostLatencyScatterPoint {
  model: string
  avg_cost_per_request: number
  avg_latency_ms: number
  total_spend_usd: number
  total_requests: number
}

/// A/B test lift metrics for the Models tab.
export interface ABTestLiftResponse {
  experiment_id: string
  experiment_name: string
  champion: ABTestVariant
  challenger: ABTestVariant
  cost_delta_percent: number
  quality_metrics: QualityMetricDelta[]
}

/// A/B test variant info.
export interface ABTestVariant {
  model: string
  weight_percent: number
}

/// Quality metric delta for A/B test lift.
export interface QualityMetricDelta {
  metric: string
  delta: string
}

/// Experiment summary for A/B testing.
export interface ExperimentSummary {
  experiment_name: string
  variant_name: string
  total_requests: number
  avg_latency_ms: number
  total_cost_usd: number
  avg_tokens: number
  error_count: number
}

// ── Security Analytics Types (Security Tab) ─────────────────────────────────────

/// Security KPI summary for the Security tab ribbon.
export interface SecuritySummaryStats {
  /// Total PII redactions
  pii_redactions: number
  /// Total guardrail blocks
  guardrail_blocks: number
  /// Total shadow mode violations
  shadow_violations: number
  /// External guardrail blocks
  external_blocks: number
}

/// Guardrail trigger count by category for the Security tab.
export interface GuardrailTriggerStat {
  /// Guardrail category
  category: string
  /// Number of triggers
  count: number
}

/// PII breakdown by pattern type for the Security tab.
export interface PiiBreakdownStat {
  /// PII pattern type
  pattern: string
  /// Number of redactions
  count: number
}

/// Policy action count for the Security tab.
export interface PolicyActionStat {
  /// Policy action type
  action: string
  /// Number of times applied
  count: number
}

/// Shadow mode policy with violation stats for the Security tab.
export interface ShadowPolicyStat {
  /// Policy name
  policy_name: string
  /// Number of shadow violations
  violations: number
  /// Token with most violations
  top_token: string
  /// Policy status
  status: string
}

/// Data residency stats for the Security tab.
export interface DataResidencyStats {
  /// Percentage of EU-routed requests
  eu_percent: number
  /// Percentage of US-routed requests
  us_percent: number
}

// ── HITL Analytics Types (HITL Tab) ─────────────────────────────────────

/// HITL KPI summary for the HITL tab ribbon.
export interface HitlSummaryStats {
  /// Number of pending approval requests
  pending_count: number
  /// Average wait time in seconds for completed approvals
  avg_wait_seconds: number
  /// Approval rate as percentage (0-100)
  approval_rate: number
}

/// HITL volume timeseries point for the HITL tab chart.
export interface HitlVolumePoint {
  /// Time bucket
  bucket: string
  /// Count of approved requests
  approved_count: number
  /// Count of rejected requests
  rejected_count: number
  /// Count of expired requests (proxy for escalated)
  expired_count: number
  /// Count of pending requests
  pending_count: number
}

/// HITL latency stats for the SLA card.
export interface HitlLatencyStats {
  /// P50 approval time in seconds
  p50_seconds: number
  /// P90 approval time in seconds
  p90_seconds: number
  /// P99 approval time in seconds
  p99_seconds: number
  /// Average approval time in seconds
  avg_seconds: number
}

/// Rejection reason breakdown for the HITL tab.
export interface RejectionReason {
  /// Reason for rejection
  reason: string
  /// Percentage of total rejections
  percentage: number
}

/// Approval request from the gateway.
export interface ApprovalRequest {
  id: string
  token_id: string
  project_id: string
  idempotency_key: string | null
  request_summary: {
    method: string
    path: string
    agent?: string
    upstream?: string
    body_preview?: string
  } | null
  status: "pending" | "approved" | "rejected" | "expired"
  reviewed_by: string | null
  reviewed_at: string | null
  expires_at: string
  created_at: string
}

// ── Error Analytics Types (Errors Tab) ─────────────────────────────────────

/// Error KPI summary for the Errors tab ribbon.
export interface ErrorSummaryStats {
  /// Total error count
  total_errors: number
  /// Error rate as percentage (0-100)
  error_rate: number
  /// Circuit breaker trip count
  circuit_breaker_trips: number
  /// Rate limit hit count
  rate_limit_hits: number
  /// Most common error type
  top_error_type: string | null
  /// Prior period total errors for delta
  prior_total_errors: number
  /// Prior period total requests for delta
  prior_total_requests: number
}

/// Error timeseries point for the Errors tab chart.
export interface ErrorTimeseriesPoint {
  /// Time bucket
  bucket: string
  /// Timeout error count
  timeout_count: number
  /// Rate limit error count
  rate_limit_count: number
  /// Upstream 5xx error count
  upstream_5xx_count: number
  /// Circuit breaker trip count
  circuit_breaker_count: number
}

/// Error type breakdown for the Errors tab bar chart.
export interface ErrorTypeBreakdown {
  /// Error type
  error_type: string
  /// Count of errors
  count: number
  /// Percentage of total errors
  percentage: number
}

/// Error log row for the Errors tab table.
export interface ErrorLogRow {
  /// Request ID
  request_id: string
  /// Token ID
  token_id: string
  /// Token name
  token_name: string | null
  /// Model used
  model: string | null
  /// Error type
  error_type: string | null
  /// HTTP status from upstream
  upstream_status: number | null
  /// Response latency in ms
  response_latency_ms: number | null
  /// Deny reason
  deny_reason: string | null
  /// Timestamp
  created_at: string
}