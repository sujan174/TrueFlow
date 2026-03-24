// Policy types matching gateway/src/models/policy.rs

export type PolicyMode = 'enforce' | 'shadow'
export type PolicyPhase = 'pre' | 'post'

// Condition tree
export interface ConditionCheck {
  field: string
  op: 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'in' | 'glob' | 'regex' | 'contains' | 'exists' | 'starts_with' | 'ends_with'
  value: unknown
}

export interface ConditionAll {
  all: Condition[]
}

export interface ConditionAny {
  any: Condition[]
}

export interface ConditionNot {
  not: Condition
}

export interface ConditionAlways {
  always: boolean
}

export type Condition =
  | ConditionCheck
  | ConditionAll
  | ConditionAny
  | ConditionNot
  | ConditionAlways

// Rate limit key
export type RateLimitKey = 'ip' | 'user' | 'token' | 'global'

// Redact direction
export type RedactDirection = 'request' | 'response' | 'both'

// Redact on match
export type RedactOnMatch = 'redact' | 'block'

// Action types (simplified for TypeScript - covers the main action variants)
export interface ActionDeny {
  action: 'deny'
  status?: number
  message?: string
}

export interface ActionAllow {
  action: 'allow'
}

export interface ActionRequireApproval {
  action: 'require_approval'
  timeout?: string
  fallback?: string
}

export interface ActionRateLimit {
  action: 'rate_limit'
  window: string
  max_requests: number
  key?: RateLimitKey
}

export interface ActionThrottle {
  action: 'throttle'
  delay_ms: number
}

export interface ActionRedact {
  action: 'redact'
  direction?: RedactDirection
  patterns?: string[]
  fields?: string[]
  on_match?: RedactOnMatch
}

export interface ActionTransform {
  action: 'transform'
  operations: Array<{
    type: 'set_header' | 'append_system_prompt' | 'prepend_system_prompt'
    [key: string]: unknown
  }>
}

export interface ActionOverride {
  action: 'override'
  set_body_fields: Record<string, unknown>
}

export interface ActionLog {
  action: 'log'
  level?: string
  tags?: Record<string, string>
}

export interface ActionTag {
  action: 'tag'
  key: string
  value: string
}

export interface ActionWebhook {
  action: 'webhook'
  url: string
  timeout_ms?: number
}

export interface ActionContentFilter {
  action: 'content_filter'
  block_jailbreak?: boolean
  block_harmful?: boolean
  block_code_injection?: boolean
  block_profanity?: boolean
  block_bias?: boolean
  block_competitor_mention?: boolean
  block_sensitive_topics?: boolean
  block_gibberish?: boolean
  block_contact_info?: boolean
  block_ip_leakage?: boolean
  competitor_names?: string[]
  topic_allowlist?: string[]
  topic_denylist?: string[]
  custom_patterns?: string[]
  risk_threshold?: number
  max_content_length?: number
}

export interface ActionSplit {
  action: 'split'
  variants: Array<{
    weight: number
    name: string
    set_body_fields?: Record<string, unknown>
  }>
  experiment?: string
}

export interface ActionDynamicRoute {
  action: 'dynamic_route'
  strategy: 'lowest_cost' | 'lowest_latency' | 'round_robin'
  pool: Array<{
    model: string
    upstream_url: string
  }>
  fallback?: {
    model: string
    upstream_url: string
  }
}

export interface ActionValidateSchema {
  action: 'validate_schema'
  schema: Record<string, unknown>
  not?: boolean
  message?: string
}

export interface ActionConditionalRoute {
  action: 'conditional_route'
  branches: Array<{
    condition: Condition
    target: {
      model: string
      upstream_url: string
    }
  }>
  fallback?: {
    model: string
    upstream_url: string
  }
}

export interface ActionExternalGuardrail {
  action: 'external_guardrail'
  vendor: 'azure_content_safety' | 'aws_comprehend' | 'llama_guard' | 'palo_alto_airs' | 'prompt_security'
  endpoint: string
  api_key_env?: string
  threshold?: number
  on_fail?: string
  on_error?: string
}

export interface ActionToolScope {
  action: 'tool_scope'
  allowed_tools?: string[]
  blocked_tools?: string[]
  deny_message?: string
}

export type Action =
  | ActionAllow
  | ActionDeny
  | ActionRequireApproval
  | ActionRateLimit
  | ActionThrottle
  | ActionRedact
  | ActionTransform
  | ActionOverride
  | ActionLog
  | ActionTag
  | ActionWebhook
  | ActionContentFilter
  | ActionSplit
  | ActionDynamicRoute
  | ActionValidateSchema
  | ActionConditionalRoute
  | ActionExternalGuardrail
  | ActionToolScope

// Rule
export interface Rule {
  when: Condition
  then: Action | Action[]
  async_check?: boolean
}

// Retry configuration
export interface RetryConfig {
  max_retries?: number
  base_backoff_ms?: number
  max_backoff_ms?: number
  jitter_ms?: number
  status_codes?: number[]
  max_total_timeout_ms?: number
}

// Policy row from database
export interface PolicyRow {
  id: string
  project_id: string
  name: string
  mode: PolicyMode
  phase: PolicyPhase
  rules: Rule[]
  retry?: RetryConfig
  is_active: boolean
  created_at: string
}

// Policy version history
export interface PolicyVersionRow {
  id: string
  policy_id: string
  version: number
  name: string
  mode: PolicyMode
  phase: PolicyPhase
  rules: Rule[]
  retry?: RetryConfig
  changed_by?: string
  created_at: string
}

// API request/response types
export interface CreatePolicyRequest {
  name: string
  mode?: PolicyMode
  phase?: PolicyPhase
  rules: Rule[]
  retry?: RetryConfig
  project_id?: string
}

export interface UpdatePolicyRequest {
  name?: string
  mode?: PolicyMode
  phase?: PolicyPhase
  rules?: Rule[]
  retry?: RetryConfig
}

export interface PolicyResponse {
  id: string
  name: string
  message: string
}

// Helper function to check if an action is a guardrail
export function isGuardrailAction(action: Action): boolean {
  return action.action === 'content_filter' || action.action === 'external_guardrail'
}

// Helper function to get action type display name
export function getActionDisplayName(action: Action): string {
  const names: Record<string, string> = {
    allow: 'Allow',
    deny: 'Deny',
    require_approval: 'Require Approval',
    rate_limit: 'Rate Limit',
    throttle: 'Throttle',
    redact: 'Redact PII',
    transform: 'Transform',
    override: 'Override',
    log: 'Log',
    tag: 'Tag',
    webhook: 'Webhook',
    content_filter: 'Content Filter',
    split: 'Traffic Split',
    dynamic_route: 'Dynamic Route',
    validate_schema: 'Validate Schema',
    conditional_route: 'Conditional Route',
    external_guardrail: 'External Guardrail',
    tool_scope: 'Tool Scope',
  }
  return names[action.action] || action.action
}