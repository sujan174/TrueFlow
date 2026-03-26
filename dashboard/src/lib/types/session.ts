// Session types - from gateway/src/store/postgres/types.rs

/// Session status enum
export type SessionStatus = "active" | "paused" | "completed" | "expired"

/// Session row for list view (from GET /sessions)
/// Matches gateway/src/store/postgres/types.rs::SessionSummaryRow
export interface SessionRow {
  session_id: string | null
  total_requests: number
  total_cost_usd: number | null
  total_prompt_tokens: number
  total_completion_tokens: number
  total_latency_ms: number
  models_used: string[] | null
  first_request_at: string
  last_request_at: string
}

/// Individual request within a session
export interface SessionRequest {
  id: string
  created_at: string
  model: string | null
  prompt_tokens: number | null
  completion_tokens: number | null
  estimated_cost_usd: number | null
  upstream_status: number | null
  response_latency_ms: number
  policy_result: string
  method: string
  path: string
}

/// Session entity (from GET /sessions/:id/entity)
/// Matches gateway/src/store/postgres/types.rs::SessionEntity
export interface SessionEntity {
  id: string
  session_id: string
  project_id: string
  token_id: string | null
  status: SessionStatus
  spend_cap_usd: number | null
  total_cost_usd: number
  total_tokens: number
  total_requests: number
  metadata: Record<string, unknown> | null
  created_at: string
  updated_at: string
  completed_at: string | null
}

/// Session detail with request breakdown (GET /sessions/:id)
export interface SessionDetail {
  session_id: string | null
  total_requests: number
  total_cost_usd: number | null
  total_prompt_tokens: number
  total_completion_tokens: number
  total_latency_ms: number
  models_used: string[] | null
  first_request_at: string
  last_request_at: string
  requests: SessionRequest[]
}

/// Filter parameters for session queries
export interface SessionFilters {
  status?: SessionStatus
  token_id?: string
  start_time?: string
  end_time?: string
}

/// Update session status request
export interface UpdateSessionStatusRequest {
  status: "paused" | "active" | "completed"
}

/// Set spend cap request
export interface SetSessionSpendCapRequest {
  spend_cap_usd: number | null
}

/// Helper to get display text for session status
export function getSessionStatusDisplay(status: SessionStatus): string {
  switch (status) {
    case "active":
      return "Active"
    case "paused":
      return "Paused"
    case "completed":
      return "Completed"
    case "expired":
      return "Expired"
    default:
      return status
  }
}

/// Helper to get badge variant for session status
export function getSessionStatusVariant(status: SessionStatus): "success" | "outline" | "secondary" | "destructive" {
  switch (status) {
    case "active":
      return "success"
    case "paused":
      return "outline" // Yellow-ish style via CSS
    case "completed":
      return "secondary"
    case "expired":
      return "destructive"
    default:
      return "secondary"
  }
}