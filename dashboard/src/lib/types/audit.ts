// Audit log types matching gateway models

/// Audit log row for list view (from audit_logs table).
/// Matches gateway/src/store/postgres/types.rs::AuditLogRow
export interface AuditLogRow {
  id: string
  created_at: string
  token_id: string | null
  method: string
  path: string
  upstream_status: number | null
  response_latency_ms: number
  agent_name: string | null
  policy_result: string
  estimated_cost_usd: number | null
  shadow_violations: string[] | null
  fields_redacted: string[] | null
  // Phase 4 columns
  prompt_tokens: number | null
  completion_tokens: number | null
  model: string | null
  tokens_per_second: number | null
  user_id: string | null
  tenant_id: string | null
  external_request_id: string | null
  log_level: number | null
  // Phase 5: LLM Observability
  tool_call_count: number | null
  finish_reason: string | null
  error_type: string | null
  is_streaming: boolean | null
  // Phase 6: Response Cache
  cache_hit: boolean | null
}

/// Detailed audit log row with joined body data (from audit_log_bodies table).
/// Matches gateway/src/store/postgres/types.rs::AuditLogDetailRow
export interface AuditLogDetailRow extends AuditLogRow {
  upstream_url: string
  policy_mode: string | null
  deny_reason: string | null
  // Phase 5: LLM Observability
  tool_calls: Record<string, unknown> | null
  session_id: string | null
  parent_span_id: string | null
  ttft_ms: number | null
  // Phase 6: Router Debugger
  router_info: Record<string, unknown> | null
  // From audit_log_bodies JOIN
  request_body: string | null
  response_body: string | null
  request_headers: Record<string, string> | null
  response_headers: Record<string, string> | null
}

/// Filter parameters for audit log queries.
/// All fields are optional - undefined means no filter for that field.
export interface AuditFilters {
  status?: number
  token_id?: string
  model?: string
  policy_result?: string
  method?: string
  path_contains?: string
  agent_name?: string
  error_type?: string
  start_time?: string
  end_time?: string
}

/// Policy result type for display
export type PolicyResultType =
  | "allow"
  | "deny"
  | "shadow_deny"
  | "hitl_approved"
  | "hitl_rejected"
  | "hitl_timeout"

/// Helper to get display text for policy result
export function getPolicyResultDisplay(result: string): string {
  switch (result) {
    case "allow":
      return "Allowed"
    case "deny":
      return "Denied"
    case "shadow_deny":
      return "Shadow Deny"
    case "hitl_approved":
      return "HITL Approved"
    case "hitl_rejected":
      return "HITL Rejected"
    case "hitl_timeout":
      return "HITL Timeout"
    default:
      return result
  }
}

/// Helper to get badge color for policy result
export function getPolicyResultColor(result: string): string {
  switch (result) {
    case "allow":
      return "bg-success/10 text-success"
    case "deny":
      return "bg-destructive/10 text-destructive"
    case "shadow_deny":
      return "bg-warning/10 text-warning"
    case "hitl_approved":
      return "bg-success/10 text-success"
    case "hitl_rejected":
      return "bg-destructive/10 text-destructive"
    case "hitl_timeout":
      return "bg-warning/10 text-warning"
    default:
      return "bg-muted text-muted-foreground"
  }
}

/// Helper to format relative time
export function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffSecs = Math.floor(diffMs / 1000)
  const diffMins = Math.floor(diffSecs / 60)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)

  if (diffSecs < 60) return `${diffSecs}s`
  if (diffMins < 60) return `${diffMins}m`
  if (diffHours < 24) return `${diffHours}h`
  if (diffDays < 7) return `${diffDays}d`
  return date.toLocaleDateString()
}

/// Helper to format latency
export function formatLatency(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

/// Helper to format tokens
export function formatTokens(prompt: number | null, completion: number | null): string {
  if (prompt === null && completion === null) return "—"
  const p = prompt ?? 0
  const c = completion ?? 0
  return `${(p + c).toLocaleString()}`
}

/// Helper to format cost
export function formatCost(usd: number | null): string {
  if (usd === null) return "—"
  if (usd < 0.01) return `<$0.01`
  return `$${usd.toFixed(4)}`
}