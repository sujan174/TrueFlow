import type {
  AnalyticsSummary,
  AnalyticsTimeseriesPoint,
  LatencyStat,
  TokenSummary,
  TokenRow,
  SpendCap,
  ModelUsageStat,
  ProviderSpendStat,
  ProviderLatencyStat,
  UpstreamStatus,
  TrafficTimeseriesPoint,
  LatencyTimeseriesPoint,
  AuditLogRow,
  BudgetHealthStatus,
  SpendTimeseriesPoint,
  CostEfficiencyPoint,
  BudgetBurnRate,
  TokenSpendWithCap,
  SpendByDimension,
  UserGrowthPoint,
  EngagementTiersResponse,
  TokenAlertsResponse,
  RequestsPerUserPoint,
  CacheSummaryStats,
  CacheHitRatePoint,
  CachedQueryRow,
  ModelCacheEfficiency,
  CacheLatencyComparison,
  ModelUsageTimeseriesPoint,
  ModelErrorRate,
  ModelLatencyStat,
  ModelStatsRow,
  CostLatencyScatterPoint,
  ExperimentSummary,
  SecuritySummaryStats,
  GuardrailTriggerStat,
  PiiBreakdownStat,
  PolicyActionStat,
  ShadowPolicyStat,
  DataResidencyStats,
  HitlSummaryStats,
  HitlVolumePoint,
  HitlLatencyStats,
  RejectionReason,
  ApprovalRequest,
  ErrorSummaryStats,
  ErrorTimeseriesPoint,
  ErrorTypeBreakdown,
  ErrorLogRow,
} from "./types/analytics"
import type { Project } from "./types/project"
import type {
  TokenRow as TokenType,
  CreateTokenRequest,
  CreateTokenResponse,
  TokenUsageStats,
  CircuitBreakerConfig,
  CredentialMeta,
  CreateCredentialRequest,
  CreateCredentialResponse,
  BulkCreateTokenRequest,
  BulkCreateTokenResponse,
  BulkRevokeRequest,
  BulkRevokeResponse,
  DeleteResponse,
} from "./types/token"
import type {
  PolicyRow,
  PolicyVersionRow,
  CreatePolicyRequest,
  UpdatePolicyRequest,
  PolicyResponse,
  Rule,
  Action,
} from "./types/policy"
import type {
  AuditLogRow as AuditLogRowType,
  AuditLogDetailRow,
  AuditFilters,
} from "./types/audit"

// Re-export types for consumers
export type {
  AnalyticsSummary,
  AnalyticsTimeseriesPoint,
  LatencyStat,
  TokenSummary,
  TokenRow,
  SpendCap,
  ModelUsageStat,
  ProviderSpendStat,
  ProviderLatencyStat,
  UpstreamStatus,
  TrafficTimeseriesPoint,
  LatencyTimeseriesPoint,
  AuditLogRow,
  BudgetHealthStatus,
  SpendTimeseriesPoint,
  CostEfficiencyPoint,
  BudgetBurnRate,
  TokenSpendWithCap,
  SpendByDimension,
  UserGrowthPoint,
  EngagementTiersResponse,
  TokenAlertsResponse,
  RequestsPerUserPoint,
  CacheSummaryStats,
  CacheHitRatePoint,
  CachedQueryRow,
  ModelCacheEfficiency,
  CacheLatencyComparison,
  ModelUsageTimeseriesPoint,
  ModelErrorRate,
  ModelLatencyStat,
  ModelStatsRow,
  CostLatencyScatterPoint,
  ExperimentSummary,
  SecuritySummaryStats,
  GuardrailTriggerStat,
  PiiBreakdownStat,
  PolicyActionStat,
  ShadowPolicyStat,
  DataResidencyStats,
  HitlSummaryStats,
  HitlVolumePoint,
  HitlLatencyStats,
  RejectionReason,
  ApprovalRequest,
  Project,
  ErrorSummaryStats,
  ErrorTimeseriesPoint,
  ErrorTypeBreakdown,
  ErrorLogRow,
  PolicyRow,
  PolicyVersionRow,
  CreatePolicyRequest,
  UpdatePolicyRequest,
  PolicyResponse,
  Rule,
}

// Re-export audit types
export type {
  AuditLogRow as AuditLogRowType,
  AuditLogDetailRow,
  AuditFilters,
} from "./types/audit"

// Organization types
import type {
  Team,
  TeamMember,
  TeamSpend,
  CreateTeamRequest,
  UpdateTeamRequest,
  ApiKey,
  CreateApiKeyRequest,
  CreateApiKeyResponse,
  WhoAmIResponse,
  ModelAccessGroup,
  CreateModelAccessGroupRequest,
  UpdateModelAccessGroupRequest,
  User,
} from "./types/organization"

export type {
  Team,
  TeamMember,
  TeamSpend,
  CreateTeamRequest,
  UpdateTeamRequest,
  ApiKey,
  CreateApiKeyRequest,
  CreateApiKeyResponse,
  WhoAmIResponse,
  ModelAccessGroup,
  CreateModelAccessGroupRequest,
  UpdateModelAccessGroupRequest,
  User,
}

// Session types
import type {
  SessionStatus,
  SessionRow,
  SessionDetail,
  SessionEntity,
  SessionFilters,
  SessionRequest,
  UpdateSessionStatusRequest,
  SetSessionSpendCapRequest,
} from "./types/session"

export type {
  SessionStatus,
  SessionRow,
  SessionDetail,
  SessionEntity,
  SessionFilters,
  SessionRequest,
  UpdateSessionStatusRequest,
  SetSessionSpendCapRequest,
}

// Use local API proxy for client-side calls (handles auth server-side)
interface FetchOptions {
  cache?: RequestCache
  next?: { revalidate?: number }
}

/**
 * Error class for API errors with additional context
 */
class ApiError extends Error {
  status: number
  statusText: string
  endpoint: string
  responseBody?: string

  constructor(
    status: number,
    statusText: string,
    endpoint: string,
    responseBody?: string
  ) {
    const message = responseBody
      ? `API error ${status} ${statusText} for ${endpoint}: ${responseBody.slice(0, 200)}`
      : `API error ${status} ${statusText} for ${endpoint}`
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.statusText = statusText
    this.endpoint = endpoint
    this.responseBody = responseBody
  }
}

async function gatewayFetch<T>(
  endpoint: string,
  options: FetchOptions = {}
): Promise<T> {
  const response = await fetch(`/api/gateway${endpoint}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
    },
  })

  if (!response.ok) {
    // Try to get error details from response body
    let errorBody: string | undefined
    try {
      errorBody = await response.text()
    } catch {
      // Ignore text parsing errors
    }
    throw new ApiError(response.status, response.statusText, endpoint, errorBody)
  }

  // Parse JSON with proper error handling
  try {
    const text = await response.text()
    if (!text) {
      // Empty response - return empty object for object types, undefined for others
      return {} as T
    }
    return JSON.parse(text) as T
  } catch (parseError) {
    throw new Error(
      `Failed to parse JSON response from ${endpoint}: ${parseError instanceof Error ? parseError.message : 'Unknown error'}`
    )
  }
}

// Analytics endpoints
export async function getAnalyticsSummary(hours = 24): Promise<AnalyticsSummary> {
  return gatewayFetch<AnalyticsSummary>(`/analytics/summary?range=${hours}`, {
    next: { revalidate: 60 }, // Cache for 60 seconds
  })
}

export async function getAnalyticsTimeseries(hours = 24): Promise<AnalyticsTimeseriesPoint[]> {
  return gatewayFetch<AnalyticsTimeseriesPoint[]>(`/analytics/timeseries?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getLatencyPercentiles(): Promise<LatencyStat> {
  return gatewayFetch<LatencyStat>("/analytics/latency", {
    next: { revalidate: 60 },
  })
}

export async function getTokenAnalytics(): Promise<TokenSummary[]> {
  return gatewayFetch<TokenSummary[]>("/analytics/tokens", {
    next: { revalidate: 60 },
  })
}

// New provider analytics endpoints
export async function getModelUsage(hours = 24): Promise<ModelUsageStat[]> {
  return gatewayFetch<ModelUsageStat[]>(`/analytics/models?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getSpendByProvider(hours = 24): Promise<ProviderSpendStat[]> {
  return gatewayFetch<ProviderSpendStat[]>(`/analytics/spend/provider?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getSpendByModel(hours = 24): Promise<SpendByDimension[]> {
  return gatewayFetch<SpendByDimension[]>(`/analytics/spend/breakdown?group_by=model&hours=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getLatencyByProvider(hours = 24): Promise<ProviderLatencyStat[]> {
  return gatewayFetch<ProviderLatencyStat[]>(`/analytics/latency/provider?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

// Token endpoints
export async function listTokens(limit = 100): Promise<TokenRow[]> {
  return gatewayFetch<TokenRow[]>(`/tokens?limit=${limit}`, {
    next: { revalidate: 30 },
  })
}

export async function getTokenSpendCaps(): Promise<SpendCap[]> {
  const tokens = await listTokens(100)
  return tokens
    .filter((t) => t.spend_cap_usd !== null && t.spend_cap_usd > 0)
    .map((t) => ({
      token_id: t.id,
      token_name: t.name,
      spend_used_usd: t.spend_used_usd,
      spend_cap_usd: t.spend_cap_usd!,
    }))
    .sort((a, b) => (b.spend_used_usd || 0) - (a.spend_used_usd || 0))
    .slice(0, 5)
}

// Upstream health
export async function getUpstreamHealth(): Promise<UpstreamStatus[]> {
  try {
    return gatewayFetch("/health/upstreams", { next: { revalidate: 30 } })
  } catch {
    return []
  }
}

// Calculate derived metrics
export function formatNumber(num: number): string {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + "M"
  }
  if (num >= 1000) {
    return (num / 1000).toFixed(1) + "k"
  }
  return num.toString()
}

export function formatCurrency(num: number): string {
  return "$" + num.toFixed(2)
}

export function formatLatency(ms: number): string {
  return Math.round(ms) + "ms"
}

// Traffic analytics endpoints (Traffic Tab)
export async function getTrafficTimeseries(hours = 24): Promise<TrafficTimeseriesPoint[]> {
  return gatewayFetch<TrafficTimeseriesPoint[]>(`/analytics/traffic/timeseries?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getLatencyTimeseries(hours = 24): Promise<LatencyTimeseriesPoint[]> {
  return gatewayFetch<LatencyTimeseriesPoint[]>(`/analytics/latency/timeseries?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getAuditLogs(limit = 50): Promise<AuditLogRow[]> {
  return gatewayFetch<AuditLogRow[]>(`/audit?limit=${limit}`, {
    next: { revalidate: 30 },
  })
}

// Audit log endpoints with filtering
export async function listAuditLogs(
  projectId: string,
  filters?: AuditFilters,
  limit = 50,
  offset = 0
): Promise<AuditLogRowType[]> {
  const params = new URLSearchParams()
  params.set("project_id", projectId)
  params.set("limit", String(limit))
  params.set("offset", String(offset))

  if (filters) {
    if (filters.status !== undefined) params.set("status", String(filters.status))
    if (filters.token_id) params.set("token_id", filters.token_id)
    if (filters.model) params.set("model", filters.model)
    if (filters.policy_result) params.set("policy_result", filters.policy_result)
    if (filters.method) params.set("method", filters.method)
    if (filters.path_contains) params.set("path_contains", filters.path_contains)
    if (filters.agent_name) params.set("agent_name", filters.agent_name)
    if (filters.error_type) params.set("error_type", filters.error_type)
    if (filters.start_time) params.set("start_time", filters.start_time)
    if (filters.end_time) params.set("end_time", filters.end_time)
  }

  return gatewayFetch<AuditLogRowType[]>(`/audit?${params}`, {
    next: { revalidate: 30 },
  })
}

export async function getAuditLogDetail(id: string, projectId: string): Promise<AuditLogDetailRow> {
  return gatewayFetch<AuditLogDetailRow>(`/audit/${id}?project_id=${projectId}`, {
    next: { revalidate: 30 },
  })
}

// Cost analytics endpoints (Cost Tab)
export async function getBudgetHealth(): Promise<BudgetHealthStatus> {
  return gatewayFetch<BudgetHealthStatus>("/analytics/budget-health", {
    next: { revalidate: 60 },
  })
}

export async function getSpendTimeseries(
  groupBy: "provider" | "model" | "token" = "provider",
  hours = 168
): Promise<SpendTimeseriesPoint[]> {
  return gatewayFetch<SpendTimeseriesPoint[]>(
    `/analytics/spend/timeseries?group_by=${groupBy}&range=${hours}`,
    { next: { revalidate: 60 } }
  )
}

export async function getCostEfficiency(hours = 168): Promise<CostEfficiencyPoint[]> {
  return gatewayFetch<CostEfficiencyPoint[]>(`/analytics/cost-efficiency?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getBudgetBurnRate(): Promise<BudgetBurnRate> {
  return gatewayFetch<BudgetBurnRate>("/analytics/burn-rate", {
    next: { revalidate: 60 },
  })
}

export async function getTokenSpendWithCaps(hours = 168): Promise<TokenSpendWithCap[]> {
  return gatewayFetch<TokenSpendWithCap[]>(`/analytics/token-spend?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

// Project endpoints - use local API proxy routes for client-side calls
export async function listProjects(): Promise<Project[]> {
  const response = await fetch("/api/projects", {
    cache: "no-store",
  })

  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`)
  }

  return response.json()
}

export async function createProject(name: string): Promise<Project> {
  const response = await fetch("/api/projects", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ name }),
  })

  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`)
  }

  return response.json()
}

export async function updateProject(id: string, name: string): Promise<Project> {
  const response = await fetch(`/api/projects/${id}`, {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ name }),
  })

  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`)
  }

  return response.json()
}

export async function deleteProject(id: string): Promise<void> {
  const response = await fetch(`/api/projects/${id}`, {
    method: "DELETE",
  })

  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`)
  }
}

// User preferences endpoints
export interface CurrentUser {
  id: string
  org_id: string
  email: string
  role: string
  supabase_id: string | null
  name: string | null
  picture_url: string | null
  last_project_id: string | null
}

export async function getCurrentUser(): Promise<CurrentUser> {
  return gatewayFetch<CurrentUser>("/users/me", {
    next: { revalidate: 0 }, // Don't cache - need fresh user data
  })
}

export async function updateLastProject(projectId: string): Promise<void> {
  const response = await fetch("/api/gateway/users/me/last-project", {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ project_id: projectId }),
  })

  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`)
  }
}

// Users & Tokens analytics endpoints (Users & Tokens Tab)
export async function getUserGrowth(hours = 720): Promise<UserGrowthPoint[]> {
  return gatewayFetch<UserGrowthPoint[]>(`/analytics/users/growth?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getEngagementTiers(hours = 720): Promise<EngagementTiersResponse> {
  return gatewayFetch<EngagementTiersResponse>(`/analytics/users/engagement?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getTokenAlerts(hours = 24): Promise<TokenAlertsResponse> {
  return gatewayFetch<TokenAlertsResponse>(`/analytics/tokens/alerts?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getRequestsPerUser(hours = 168): Promise<RequestsPerUserPoint[]> {
  return gatewayFetch<RequestsPerUserPoint[]>(`/analytics/users/requests?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

// Cache analytics endpoints (Cache Tab)
export async function getCacheSummary(hours = 24): Promise<CacheSummaryStats> {
  return gatewayFetch<CacheSummaryStats>(`/analytics/cache/summary?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getCacheHitRateTimeseries(hours = 168): Promise<CacheHitRatePoint[]> {
  return gatewayFetch<CacheHitRatePoint[]>(`/analytics/cache/hit-rate-timeseries?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getTopCachedQueries(limit = 25): Promise<CachedQueryRow[]> {
  return gatewayFetch<CachedQueryRow[]>(`/analytics/cache/top-queries?limit=${limit}`, {
    next: { revalidate: 60 },
  })
}

export async function getModelCacheEfficiency(hours = 168): Promise<ModelCacheEfficiency[]> {
  return gatewayFetch<ModelCacheEfficiency[]>(`/analytics/cache/model-efficiency?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getCacheLatencyComparison(hours = 168): Promise<CacheLatencyComparison> {
  return gatewayFetch<CacheLatencyComparison>(`/analytics/cache/latency-comparison?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

// Model analytics endpoints (Models Tab)
export async function getModelUsageTimeseries(
  groupBy: "requests" | "cost" | "cache_hits" = "requests",
  hours = 168
): Promise<ModelUsageTimeseriesPoint[]> {
  return gatewayFetch<ModelUsageTimeseriesPoint[]>(
    `/analytics/models/usage-timeseries?group_by=${groupBy}&range=${hours}`,
    { next: { revalidate: 60 } }
  )
}

export async function getModelErrorRates(hours = 168): Promise<ModelErrorRate[]> {
  return gatewayFetch<ModelErrorRate[]>(`/analytics/models/error-rates?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getModelLatency(hours = 168): Promise<ModelLatencyStat[]> {
  return gatewayFetch<ModelLatencyStat[]>(`/analytics/models/latency?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getModelStats(hours = 168): Promise<ModelStatsRow[]> {
  return gatewayFetch<ModelStatsRow[]>(`/analytics/models/stats?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getCostLatencyScatter(hours = 168): Promise<CostLatencyScatterPoint[]> {
  return gatewayFetch<CostLatencyScatterPoint[]>(`/analytics/models/cost-latency-scatter?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

// Experiment analytics endpoints
export async function getAnalyticsExperiments(): Promise<ExperimentSummary[]> {
  return gatewayFetch<ExperimentSummary[]>("/analytics/experiments", {
    next: { revalidate: 60 },
  })
}

// Security analytics endpoints (Security Tab)
export async function getSecuritySummary(hours = 168): Promise<SecuritySummaryStats> {
  return gatewayFetch<SecuritySummaryStats>(`/analytics/security/summary?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getGuardrailTriggers(hours = 168): Promise<GuardrailTriggerStat[]> {
  return gatewayFetch<GuardrailTriggerStat[]>(`/analytics/security/guardrail-triggers?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getPiiBreakdown(hours = 168): Promise<PiiBreakdownStat[]> {
  return gatewayFetch<PiiBreakdownStat[]>(`/analytics/security/pii-breakdown?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getPolicyActions(hours = 168): Promise<PolicyActionStat[]> {
  return gatewayFetch<PolicyActionStat[]>(`/analytics/security/policy-actions?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getShadowPolicies(hours = 168): Promise<ShadowPolicyStat[]> {
  return gatewayFetch<ShadowPolicyStat[]>(`/analytics/security/shadow-policies?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getDataResidency(hours = 168): Promise<DataResidencyStats> {
  return gatewayFetch<DataResidencyStats>(`/analytics/security/data-residency?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

// HITL analytics endpoints (HITL Tab)
export async function getHitlSummary(hours = 168): Promise<HitlSummaryStats> {
  return gatewayFetch<HitlSummaryStats>(`/analytics/hitl/summary?range=${hours}`, {
    next: { revalidate: 30 },
  })
}

export async function getHitlVolume(hours = 168): Promise<HitlVolumePoint[]> {
  return gatewayFetch<HitlVolumePoint[]>(`/analytics/hitl/volume?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getHitlLatency(hours = 168): Promise<HitlLatencyStats> {
  return gatewayFetch<HitlLatencyStats>(`/analytics/hitl/latency?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getHitlRejectionReasons(hours = 168): Promise<RejectionReason[]> {
  return gatewayFetch<RejectionReason[]>(`/analytics/hitl/reasons?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function listApprovals(): Promise<ApprovalRequest[]> {
  return gatewayFetch<ApprovalRequest[]>("/approvals")
}

export async function decideApproval(
  id: string,
  decision: "approved" | "rejected"
): Promise<{ id: string; status: string; updated: boolean }> {
  const response = await fetch(`/api/gateway/approvals/${id}/decision`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ decision }),
  })

  if (!response.ok) {
    throw new Error(`Failed to ${decision} approval: ${response.status}`)
  }

  return response.json()
}

// Error analytics endpoints (Errors Tab)
export async function getErrorSummary(hours = 168): Promise<ErrorSummaryStats> {
  return gatewayFetch<ErrorSummaryStats>(`/analytics/errors/summary?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getErrorTimeseries(hours = 168): Promise<ErrorTimeseriesPoint[]> {
  return gatewayFetch<ErrorTimeseriesPoint[]>(`/analytics/errors/timeseries?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getErrorBreakdown(hours = 168): Promise<ErrorTypeBreakdown[]> {
  return gatewayFetch<ErrorTypeBreakdown[]>(`/analytics/errors/breakdown?range=${hours}`, {
    next: { revalidate: 60 },
  })
}

export async function getErrorLogs(limit = 50): Promise<ErrorLogRow[]> {
  return gatewayFetch<ErrorLogRow[]>(`/analytics/errors/logs?limit=${limit}`, {
    next: { revalidate: 30 },
  })
}

// ── Token Management API ─────────────────────────────────────────────────────

interface ListTokensParams {
  limit?: number
  offset?: number
  external_user_id?: string
  team_id?: string
}

export async function listTokensWithParams(params?: ListTokensParams): Promise<TokenType[]> {
  const searchParams = new URLSearchParams()
  if (params?.limit) searchParams.set("limit", params.limit.toString())
  if (params?.offset) searchParams.set("offset", params.offset.toString())
  if (params?.external_user_id) searchParams.set("external_user_id", params.external_user_id)
  if (params?.team_id) searchParams.set("team_id", params.team_id)

  const queryString = searchParams.toString()
  return gatewayFetch<TokenType[]>(`/tokens${queryString ? `?${queryString}` : ""}`)
}

export async function getToken(id: string): Promise<TokenType> {
  return gatewayFetch<TokenType>(`/tokens/${id}`)
}

export async function createToken(data: CreateTokenRequest): Promise<CreateTokenResponse> {
  const response = await fetch("/api/gateway/tokens", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    throw new Error(`Failed to create token: ${response.status}`)
  }

  return response.json()
}

export async function revokeToken(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/tokens/${id}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to revoke token: ${response.status}`)
  }
}

export async function bulkCreateTokens(data: BulkCreateTokenRequest): Promise<BulkCreateTokenResponse> {
  const response = await fetch("/api/gateway/tokens/bulk", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    throw new Error(`Failed to bulk create tokens: ${response.status}`)
  }

  return response.json()
}

export async function bulkRevokeTokens(data: BulkRevokeRequest): Promise<BulkRevokeResponse> {
  const response = await fetch("/api/gateway/tokens/bulk-revoke", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    throw new Error(`Failed to bulk revoke tokens: ${response.status}`)
  }

  return response.json()
}

export async function getTokenUsage(id: string, hours = 168): Promise<TokenUsageStats> {
  return gatewayFetch<TokenUsageStats>(`/tokens/${id}/usage?hours=${hours}`)
}

export async function getCircuitBreaker(id: string): Promise<CircuitBreakerConfig> {
  return gatewayFetch<CircuitBreakerConfig>(`/tokens/${id}/circuit-breaker`)
}

export async function updateCircuitBreaker(
  id: string,
  config: Partial<CircuitBreakerConfig>
): Promise<CircuitBreakerConfig> {
  const response = await fetch(`/api/gateway/tokens/${id}/circuit-breaker`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  })

  if (!response.ok) {
    throw new Error(`Failed to update circuit breaker: ${response.status}`)
  }

  return response.json()
}

// ── Credentials (Vault) API ──────────────────────────────────────────────────

export async function listCredentials(): Promise<CredentialMeta[]> {
  return gatewayFetch<CredentialMeta[]>("/credentials")
}

export async function createCredential(data: CreateCredentialRequest): Promise<CreateCredentialResponse> {
  const response = await fetch("/api/gateway/credentials", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    throw new Error(`Failed to create credential: ${response.status}`)
  }

  return response.json()
}

export async function deleteCredential(id: string): Promise<DeleteResponse> {
  const response = await fetch(`/api/gateway/credentials/${id}`, {
    method: "DELETE",
  })

  if (!response.ok) {
    throw new Error(`Failed to delete credential: ${response.status}`)
  }

  return response.json()
}

// Re-export token types (TokenRow already exported from analytics)
export type {
  CreateTokenRequest,
  CreateTokenResponse,
  TokenUsageStats,
  CircuitBreakerConfig,
  CredentialMeta,
  CreateCredentialRequest,
  CreateCredentialResponse,
  BulkCreateTokenRequest,
  BulkCreateTokenResponse,
  BulkRevokeRequest,
  BulkRevokeResponse,
  DeleteResponse,
} from "./types/token"

// Re-export policy types
export type {
  PolicyMode,
  PolicyPhase,
  Condition,
  ConditionCheck,
  ConditionAll,
  ConditionAny,
  ConditionNot,
  ConditionAlways,
  Action,
  ActionDeny,
  ActionAllow,
  ActionRequireApproval,
  ActionRateLimit,
  ActionThrottle,
  ActionRedact,
  ActionTransform,
  ActionOverride,
  ActionLog,
  ActionTag,
  ActionWebhook,
  ActionContentFilter,
  ActionSplit,
  ActionDynamicRoute,
  ActionValidateSchema,
  ActionConditionalRoute,
  ActionExternalGuardrail,
  ActionToolScope,
  RetryConfig,
  isGuardrailAction,
  getActionDisplayName,
} from "./types/policy"

// ── Policy Management API ─────────────────────────────────────────────────────

interface ListPoliciesParams {
  limit?: number
  offset?: number
  project_id?: string
}

export async function listPolicies(params?: ListPoliciesParams): Promise<PolicyRow[]> {
  const searchParams = new URLSearchParams()
  if (params?.limit) searchParams.set("limit", params.limit.toString())
  if (params?.offset) searchParams.set("offset", params.offset.toString())
  if (params?.project_id) searchParams.set("project_id", params.project_id)

  const queryString = searchParams.toString()
  return gatewayFetch<PolicyRow[]>(`/policies${queryString ? `?${queryString}` : ""}`)
}

export async function createPolicy(data: CreatePolicyRequest): Promise<PolicyResponse> {
  const response = await fetch("/api/gateway/policies", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create policy: ${response.status} ${error}`)
  }

  return response.json()
}

export async function updatePolicy(id: string, data: UpdatePolicyRequest): Promise<PolicyResponse> {
  const response = await fetch(`/api/gateway/policies/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update policy: ${response.status} ${error}`)
  }

  return response.json()
}

export async function deletePolicy(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/policies/${id}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to delete policy: ${response.status}`)
  }
}

export async function getPolicyVersions(id: string): Promise<PolicyVersionRow[]> {
  return gatewayFetch<PolicyVersionRow[]>(`/policies/${id}/versions`)
}

// ── Guardrail Presets API ─────────────────────────────────────────

export interface GuardrailScope {
  models?: string[]
  paths?: string[]
}

export interface EnableGuardrailsRequest {
  token_id: string
  presets: string[]
  source?: "sdk" | "dashboard" | "header"
  topic_allowlist?: string[]
  topic_denylist?: string[]
  scope?: GuardrailScope
}

export interface GuardrailsResponse {
  success: boolean
  applied_presets: string[]
  policy_id: string | null
  policy_name: string
  skipped: string[]
  previous_source: string | null
}

export interface GuardrailsStatus {
  token_id: string
  has_guardrails: boolean
  source: string | null
  policy_id: string | null
  policy_name: string | null
  presets: string[]
}

export interface GuardrailPreset {
  name: string
  description: string
  category: string
  actions: string[]
  warning?: string
}

export interface ListPresetsResponse {
  presets: GuardrailPreset[]
  total: number
}

/**
 * Enable guardrails on a token using presets.
 * @example
 * await enableGuardrails({
 *   token_id: "tf_v1_xxx",
 *   presets: ["prompt_injection", "pii_redaction"],
 *   source: "dashboard",
 *   scope: { models: ["gpt-4"] }
 * })
 */
export async function enableGuardrails(
  data: EnableGuardrailsRequest
): Promise<GuardrailsResponse> {
  const response = await fetch("/api/gateway/guardrails/enable", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      ...data,
      source: data.source || "dashboard",
    }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to enable guardrails: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Disable guardrails on a token.
 */
export async function disableGuardrails(
  tokenId: string,
  policyNamePrefix?: string
): Promise<void> {
  const body: Record<string, string> = { token_id: tokenId }
  if (policyNamePrefix) {
    body.policy_name_prefix = policyNamePrefix
  }

  const response = await fetch("/api/gateway/guardrails/disable", {
    method: "DELETE",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to disable guardrails: ${response.status}`)
  }
}

/**
 * Get guardrail status for a token.
 */
export async function getGuardrailStatus(tokenId: string): Promise<GuardrailsStatus> {
  return gatewayFetch<GuardrailsStatus>(`/guardrails/status?token_id=${encodeURIComponent(tokenId)}`)
}

/**
 * List available guardrail presets.
 */
export async function listGuardrailPresets(): Promise<ListPresetsResponse> {
  return gatewayFetch<ListPresetsResponse>("/guardrails/presets")
}

// ── MCP Server Management API ─────────────────────────────────────────────────

import type {
  McpServerInfo,
  McpToolDef,
  DiscoveryResult,
  RegisterMcpServerRequest,
  RegisterMcpServerResponse,
  TestMcpServerResponse,
  ReauthResponse,
} from "./types/mcp"

export type {
  McpServerInfo,
  McpToolDef,
  DiscoveryResult,
  RegisterMcpServerRequest,
  RegisterMcpServerResponse,
  TestMcpServerResponse,
  ReauthResponse,
}

/**
 * List all registered MCP servers.
 */
export async function listMcpServers(): Promise<McpServerInfo[]> {
  return gatewayFetch<McpServerInfo[]>("/mcp/servers", {
    next: { revalidate: 30 },
  })
}

/**
 * Get a single MCP server by ID.
 */
export async function getMcpServer(id: string): Promise<McpServerInfo> {
  return gatewayFetch<McpServerInfo>(`/mcp/servers/${id}`)
}

/**
 * Register a new MCP server.
 */
export async function registerMcpServer(data: RegisterMcpServerRequest): Promise<RegisterMcpServerResponse> {
  const response = await fetch("/api/gateway/mcp/servers", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to register MCP server: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Delete an MCP server.
 */
export async function deleteMcpServer(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/mcp/servers/${id}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to delete MCP server: ${response.status}`)
  }
}

/**
 * Get tools for a specific MCP server.
 */
export async function getMcpServerTools(id: string): Promise<McpToolDef[]> {
  return gatewayFetch<McpToolDef[]>(`/mcp/servers/${id}/tools`, {
    next: { revalidate: 60 },
  })
}

/**
 * Refresh tool cache for an MCP server.
 */
export async function refreshMcpServer(id: string): Promise<McpToolDef[]> {
  const response = await fetch(`/api/gateway/mcp/servers/${id}/refresh`, {
    method: "POST",
  })

  if (!response.ok) {
    throw new Error(`Failed to refresh MCP server: ${response.status}`)
  }

  return response.json()
}

/**
 * Discover MCP server (dry-run without registration).
 */
export async function discoverMcpServer(endpoint: string): Promise<DiscoveryResult> {
  const response = await fetch("/api/gateway/mcp/servers/discover", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ endpoint }),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Discovery failed: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Test MCP server connection.
 */
export async function testMcpServer(data: RegisterMcpServerRequest): Promise<TestMcpServerResponse> {
  const response = await fetch("/api/gateway/mcp/servers/test", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    throw new Error(`Test failed: ${response.status}`)
  }

  return response.json()
}

/**
 * Re-authenticate OAuth MCP server.
 */
export async function reauthMcpServer(id: string): Promise<ReauthResponse> {
  const response = await fetch(`/api/gateway/mcp/servers/${id}/reauth`, {
    method: "POST",
  })

  if (!response.ok) {
    throw new Error(`Re-authentication failed: ${response.status}`)
  }

  return response.json()
}

/**
 * Update token MCP tool access configuration.
 */
export async function updateTokenMcpTools(
  tokenId: string,
  data: { mcp_allowed_tools?: string[] | null; mcp_blocked_tools?: string[] | null }
): Promise<void> {
  const response = await fetch(`/api/gateway/tokens/${tokenId}/mcp-tools`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update MCP tools: ${response.status} ${error}`)
  }
}

// ── Teams API ──────────────────────────────────────────────────────────────

export async function listTeams(): Promise<Team[]> {
  return gatewayFetch<Team[]>("/teams", {
    next: { revalidate: 30 },
  })
}

export async function createTeam(data: CreateTeamRequest): Promise<Team> {
  const response = await fetch("/api/gateway/teams", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create team: ${response.status} ${error}`)
  }

  return response.json()
}

export async function updateTeam(id: string, data: UpdateTeamRequest): Promise<Team> {
  const response = await fetch(`/api/gateway/teams/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update team: ${response.status} ${error}`)
  }

  return response.json()
}

export async function deleteTeam(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/teams/${id}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to delete team: ${response.status}`)
  }
}

export async function listTeamMembers(teamId: string): Promise<TeamMember[]> {
  return gatewayFetch<TeamMember[]>(`/teams/${teamId}/members`, {
    next: { revalidate: 30 },
  })
}

export async function addTeamMember(
  teamId: string,
  data: { user_id: string; role?: string }
): Promise<TeamMember> {
  const response = await fetch(`/api/gateway/teams/${teamId}/members`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to add team member: ${response.status} ${error}`)
  }

  return response.json()
}

export async function removeTeamMember(teamId: string, userId: string): Promise<void> {
  const response = await fetch(`/api/gateway/teams/${teamId}/members/${userId}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to remove team member: ${response.status}`)
  }
}

export async function getTeamSpend(teamId: string): Promise<TeamSpend[]> {
  return gatewayFetch<TeamSpend[]>(`/teams/${teamId}/spend`, {
    next: { revalidate: 60 },
  })
}

// ── API Keys (Auth) ────────────────────────────────────────────────────────

export async function listApiKeys(): Promise<ApiKey[]> {
  return gatewayFetch<ApiKey[]>("/auth/keys", {
    next: { revalidate: 30 },
  })
}

export async function createApiKey(data: CreateApiKeyRequest): Promise<CreateApiKeyResponse> {
  const response = await fetch("/api/gateway/auth/keys", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create API key: ${response.status} ${error}`)
  }

  return response.json()
}

export async function revokeApiKey(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/auth/keys/${id}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to revoke API key: ${response.status}`)
  }
}

export interface UpdateApiKeyRequest {
  name?: string
  scopes?: string[]
}

export interface UpdateApiKeyResponse {
  id: string
  name: string
  scopes: string[]
  message: string
}

export async function updateApiKey(
  id: string,
  data: UpdateApiKeyRequest
): Promise<UpdateApiKeyResponse> {
  const response = await fetch(`/api/gateway/auth/keys/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.json().catch(() => ({}))
    throw new Error(error.error?.message || `Failed to update API key: ${response.status}`)
  }

  return response.json()
}

export async function whoami(): Promise<WhoAmIResponse> {
  return gatewayFetch<WhoAmIResponse>("/auth/whoami")
}

// ── Model Access Groups API ────────────────────────────────────────────────

export async function listModelAccessGroups(): Promise<ModelAccessGroup[]> {
  return gatewayFetch<ModelAccessGroup[]>("/model-access-groups", {
    next: { revalidate: 30 },
  })
}

export async function createModelAccessGroup(
  data: CreateModelAccessGroupRequest
): Promise<ModelAccessGroup> {
  const response = await fetch("/api/gateway/model-access-groups", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create model access group: ${response.status} ${error}`)
  }

  return response.json()
}

export async function updateModelAccessGroup(
  id: string,
  data: UpdateModelAccessGroupRequest
): Promise<ModelAccessGroup> {
  const response = await fetch(`/api/gateway/model-access-groups/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update model access group: ${response.status} ${error}`)
  }

  return response.json()
}

export async function deleteModelAccessGroup(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/model-access-groups/${id}`, {
    method: "DELETE",
  })

  if (!response.ok && response.status !== 204) {
    throw new Error(`Failed to delete model access group: ${response.status}`)
  }
}

// ── Users API (for team member selection) ──────────────────────────────────

export async function listUsers(): Promise<User[]> {
  return gatewayFetch<User[]>("/users", {
    next: { revalidate: 60 },
  })
}

// ── Settings Types ───────────────────────────────────────────────────

import type {
  GatewaySettings,
  PricingEntry,
  UpsertPricingRequest,
  Webhook,
  CreateWebhookRequest,
  TestWebhookResponse,
  Notification,
  CacheStats,
  ImportResult,
} from "./types/settings"

export type {
  GatewaySettings,
  PricingEntry,
  UpsertPricingRequest,
  Webhook,
  CreateWebhookRequest,
  TestWebhookResponse,
  Notification,
  CacheStats,
  ImportResult,
}

// ── Settings API ──────────────────────────────────────────────────────

export async function getSettings(): Promise<GatewaySettings> {
  return gatewayFetch<GatewaySettings>("/settings")
}

export async function updateSettings(settings: Partial<GatewaySettings>): Promise<{ success: boolean }> {
  const response = await fetch("/api/gateway/settings", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ settings }),
  })
  if (!response.ok) throw new Error(`Failed to update settings: ${response.status}`)
  return response.json()
}

export async function getCacheStats(): Promise<CacheStats> {
  return gatewayFetch<CacheStats>("/system/cache-stats")
}

export async function flushCache(): Promise<{ success: boolean; keys_deleted: number }> {
  const response = await fetch("/api/gateway/system/flush-cache", { method: "POST" })
  if (!response.ok) throw new Error(`Failed to flush cache: ${response.status}`)
  return response.json()
}

// ── Pricing API ────────────────────────────────────────────────────────

export async function listPricing(): Promise<PricingEntry[]> {
  return gatewayFetch<PricingEntry[]>("/pricing")
}

export async function upsertPricing(data: UpsertPricingRequest): Promise<{ success: boolean }> {
  const response = await fetch("/api/gateway/pricing", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })
  if (!response.ok) throw new Error(`Failed to upsert pricing: ${response.status}`)
  return response.json()
}

export async function deletePricing(id: string): Promise<{ id: string; deleted: boolean }> {
  const response = await fetch(`/api/gateway/pricing/${id}`, { method: "DELETE" })
  if (!response.ok) throw new Error(`Failed to delete pricing: ${response.status}`)
  return response.json()
}

// ── Webhooks API ───────────────────────────────────────────────────────

export async function listWebhooks(): Promise<Webhook[]> {
  return gatewayFetch<Webhook[]>("/webhooks")
}

export async function createWebhook(data: CreateWebhookRequest): Promise<Webhook> {
  const response = await fetch("/api/gateway/webhooks", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })
  if (!response.ok) throw new Error(`Failed to create webhook: ${response.status}`)
  return response.json()
}

export async function deleteWebhook(id: string): Promise<void> {
  const response = await fetch(`/api/gateway/webhooks/${id}`, { method: "DELETE" })
  if (!response.ok && response.status !== 204) throw new Error(`Failed to delete webhook: ${response.status}`)
}

export async function testWebhook(url: string): Promise<TestWebhookResponse> {
  const response = await fetch("/api/gateway/webhooks/test", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url }),
  })
  if (!response.ok) throw new Error(`Failed to test webhook: ${response.status}`)
  return response.json()
}

// ── Notifications API ──────────────────────────────────────────────────

export async function listNotifications(): Promise<Notification[]> {
  return gatewayFetch<Notification[]>("/notifications")
}

export async function getUnreadCount(): Promise<{ count: number }> {
  return gatewayFetch<{ count: number }>("/notifications/unread")
}

export async function markNotificationRead(id: string): Promise<{ success: boolean }> {
  const response = await fetch(`/api/gateway/notifications/${id}/read`, { method: "POST" })
  if (!response.ok) throw new Error(`Failed to mark notification read: ${response.status}`)
  return response.json()
}

export async function markAllNotificationsRead(): Promise<{ success: boolean }> {
  const response = await fetch("/api/gateway/notifications/read-all", { method: "POST" })
  if (!response.ok) throw new Error(`Failed to mark all read: ${response.status}`)
  return response.json()
}

// ── Config Export/Import API ───────────────────────────────────────────

export async function exportConfig(format: "yaml" | "json" = "yaml"): Promise<string> {
  const response = await fetch(`/api/gateway/config/export?format=${format}`)
  if (!response.ok) throw new Error(`Failed to export config: ${response.status}`)
  return response.text()
}

export async function importConfig(content: string, format: "yaml" | "json" = "yaml"): Promise<ImportResult> {
  const contentType = format === "json" ? "application/json" : "application/yaml"
  const response = await fetch("/api/gateway/config/import", {
    method: "POST",
    headers: { "Content-Type": contentType },
    body: content,
  })
  if (!response.ok) throw new Error(`Failed to import config: ${response.status}`)
  return response.json()
}

// ── PII Rehydrate API ──────────────────────────────────────────────────

export async function rehydratePii(tokens: string[]): Promise<{ values: Record<string, string>; token_count: number }> {
  const response = await fetch("/api/gateway/pii/rehydrate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ tokens }),
  })
  if (!response.ok) throw new Error(`Failed to rehydrate PII: ${response.status}`)
  return response.json()
}

// ── Sessions API ────────────────────────────────────────────────────────

export async function listSessions(
  projectId: string,
  filters?: SessionFilters,
  limit = 50
): Promise<SessionRow[]> {
  const params = new URLSearchParams()
  params.set("project_id", projectId)
  params.set("limit", String(limit))
  if (filters?.status) params.set("status", filters.status)
  if (filters?.token_id) params.set("token_id", filters.token_id)
  if (filters?.start_time) params.set("start_time", filters.start_time)
  if (filters?.end_time) params.set("end_time", filters.end_time)

  return gatewayFetch<SessionRow[]>(`/sessions?${params}`)
}

export async function getSession(sessionId: string, projectId: string): Promise<SessionDetail> {
  const params = new URLSearchParams()
  params.set("project_id", projectId)
  return gatewayFetch<SessionDetail>(`/sessions/${sessionId}?${params}`)
}

export async function getSessionEntity(sessionId: string, projectId: string): Promise<SessionEntity> {
  const params = new URLSearchParams()
  params.set("project_id", projectId)
  return gatewayFetch<SessionEntity>(`/sessions/${sessionId}/entity?${params}`)
}

export async function updateSessionStatus(
  sessionId: string,
  projectId: string,
  status: "paused" | "active" | "completed"
): Promise<SessionEntity> {
  const params = new URLSearchParams()
  params.set("project_id", projectId)
  const response = await fetch(`/api/gateway/sessions/${sessionId}/status?${params}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ status }),
  })
  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update session status: ${response.status} ${error}`)
  }
  return response.json()
}

export async function setSessionSpendCap(
  sessionId: string,
  projectId: string,
  spendCapUsd: number | null
): Promise<SessionEntity> {
  const params = new URLSearchParams()
  params.set("project_id", projectId)
  const response = await fetch(`/api/gateway/sessions/${sessionId}/spend-cap?${params}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ spend_cap_usd: spendCapUsd }),
  })
  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to set session spend cap: ${response.status} ${error}`)
  }
  return response.json()
}

// ═══════════════════════════════════════════════════════════════════════════
// PROMPT MANAGEMENT API
// ═══════════════════════════════════════════════════════════════════════════

import type {
  PromptRow,
  PromptVersionRow,
  PromptListResponse,
  PromptDetailResponse,
  CreatePromptRequest,
  CreatePromptResponse,
  UpdatePromptRequest,
  CreateVersionRequest,
  CreateVersionResponse,
  DeployRequest,
  RenderRequest,
  RenderResponse,
} from "./types/prompt"

export type {
  PromptRow,
  PromptVersionRow,
  PromptListResponse,
  PromptDetailResponse,
  CreatePromptRequest,
  CreatePromptResponse,
  UpdatePromptRequest,
  CreateVersionRequest,
  CreateVersionResponse,
  DeployRequest,
  RenderRequest,
  RenderResponse,
}

/**
 * List all prompts with optional folder filter
 */
export async function listPrompts(folder?: string): Promise<PromptListResponse[]> {
  const params = folder ? `?folder=${encodeURIComponent(folder)}` : ""
  return gatewayFetch<PromptListResponse[]>(`/prompts${params}`)
}

/**
 * Get a single prompt with all versions
 */
export async function getPrompt(id: string): Promise<PromptDetailResponse> {
  return gatewayFetch<PromptDetailResponse>(`/prompts/${id}`)
}

/**
 * Create a new prompt
 */
export async function createPrompt(data: CreatePromptRequest): Promise<CreatePromptResponse> {
  const response = await fetch("/api/gateway/prompts", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create prompt: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Update prompt metadata (name, description, folder, tags)
 */
export async function updatePrompt(id: string, data: UpdatePromptRequest): Promise<{ message: string }> {
  const response = await fetch(`/api/gateway/prompts/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update prompt: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Soft delete a prompt
 */
export async function deletePrompt(id: string): Promise<{ message: string }> {
  const response = await fetch(`/api/gateway/prompts/${id}`, {
    method: "DELETE",
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to delete prompt: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * List all versions for a prompt
 */
export async function listPromptVersions(promptId: string): Promise<PromptVersionRow[]> {
  return gatewayFetch<PromptVersionRow[]>(`/prompts/${promptId}/versions`)
}

/**
 * Get a specific version
 */
export async function getPromptVersion(promptId: string, version: number): Promise<PromptVersionRow> {
  return gatewayFetch<PromptVersionRow>(`/prompts/${promptId}/versions/${version}`)
}

/**
 * Create a new version (immutable)
 */
export async function createPromptVersion(promptId: string, data: CreateVersionRequest): Promise<CreateVersionResponse> {
  const response = await fetch(`/api/gateway/prompts/${promptId}/versions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create version: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Deploy a version to a label (atomic label move)
 */
export async function deployPromptVersion(promptId: string, data: DeployRequest): Promise<{ message: string; version: number; label: string }> {
  const response = await fetch(`/api/gateway/prompts/${promptId}/deploy`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to deploy version: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Render a prompt by slug with variables
 */
export async function renderPrompt(slug: string, data?: RenderRequest): Promise<RenderResponse> {
  const response = await fetch(`/api/gateway/prompts/by-slug/${slug}/render`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data || {}),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to render prompt: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * List all unique folders
 */
export async function listPromptFolders(): Promise<string[]> {
  return gatewayFetch<string[]>("/prompts/folders")
}

// ═══════════════════════════════════════════════════════════════════════════
// EXPERIMENTS API (A/B Testing)
// ═══════════════════════════════════════════════════════════════════════════

import type {
  Experiment,
  ExperimentVariant,
  ExperimentResult,
  ExperimentWithResults,
  CreateExperimentRequest,
  UpdateExperimentRequest,
  ExperimentTimeseriesPoint,
} from "./types/experiment"

export type {
  Experiment,
  ExperimentVariant,
  ExperimentResult,
  ExperimentWithResults,
  CreateExperimentRequest,
  UpdateExperimentRequest,
  ExperimentTimeseriesPoint,
}

/**
 * List all experiments
 */
export async function listExperiments(): Promise<Experiment[]> {
  return gatewayFetch<Experiment[]>("/experiments", {
    next: { revalidate: 30 },
  })
}

/**
 * Get a single experiment with results
 */
export async function getExperiment(id: string): Promise<ExperimentWithResults> {
  return gatewayFetch<ExperimentWithResults>(`/experiments/${id}`, {
    next: { revalidate: 30 },
  })
}

/**
 * Create a new experiment
 */
export async function createExperiment(data: CreateExperimentRequest): Promise<Experiment> {
  const response = await fetch("/api/gateway/experiments", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to create experiment: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Update experiment variants
 */
export async function updateExperiment(id: string, data: UpdateExperimentRequest): Promise<Experiment> {
  const response = await fetch(`/api/gateway/experiments/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to update experiment: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Stop an experiment
 */
export async function stopExperiment(id: string): Promise<{ id: string; status: string }> {
  const response = await fetch(`/api/gateway/experiments/${id}/stop`, {
    method: "POST",
  })

  if (!response.ok) {
    const error = await response.text()
    throw new Error(`Failed to stop experiment: ${response.status} ${error}`)
  }

  return response.json()
}

/**
 * Get experiment results (per-variant metrics)
 */
export async function getExperimentResults(id: string): Promise<ExperimentWithResults> {
  return gatewayFetch<ExperimentWithResults>(`/experiments/${id}/results`, {
    next: { revalidate: 30 },
  })
}

/**
 * Get experiment timeseries data for charts
 */
export async function getExperimentTimeseries(
  id: string,
  hours = 24
): Promise<ExperimentTimeseriesPoint[]> {
  return gatewayFetch<ExperimentTimeseriesPoint[]>(
    `/experiments/${id}/timeseries?range=${hours}`,
    { next: { revalidate: 60 } }
  )
}