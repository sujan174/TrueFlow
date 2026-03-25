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
  strategy: 'lowest_cost' | 'lowest_latency' | 'round_robin' | 'least_busy' | 'weighted_random'
  pool: Array<{
    model: string
    upstream_url: string
    weight?: number
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

// Extract action type strings for type-safe action discrimination
export type ActionType = Action['action']

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

// ============================================================================
// UI-Specific Types for Visual Policy Builder
// ============================================================================

// Operator type extracted for UI use
export type ConditionOperator = ConditionCheck['op']

// UI-friendly condition node for visual builder
export type UINestedCondition =
  | { type: 'leaf'; field: string; operator: ConditionOperator; value: string }
  | { type: 'group'; logic: 'AND' | 'OR'; children: UINestedCondition[] }
  | { type: 'not'; child: UINestedCondition }

// Available condition fields for the builder
export interface ConditionFieldDefinition {
  name: string
  label: string
  category: 'request' | 'response' | 'token' | 'agent' | 'context' | 'usage'
  valueType: 'string' | 'number' | 'array' | 'boolean'
  description: string
  operators: ConditionOperator[]
}

// Condition field definitions for dropdown
export const CONDITION_FIELDS: ConditionFieldDefinition[] = [
  // Request fields
  { name: 'request.method', label: 'HTTP Method', category: 'request', valueType: 'string', description: 'The HTTP method (GET, POST, etc.)', operators: ['eq', 'neq', 'in'] },
  { name: 'request.path', label: 'Request Path', category: 'request', valueType: 'string', description: 'The request URL path', operators: ['eq', 'neq', 'contains', 'starts_with', 'ends_with', 'glob', 'regex', 'in'] },
  { name: 'request.body_size', label: 'Body Size (bytes)', category: 'request', valueType: 'number', description: 'Size of request body in bytes', operators: ['eq', 'neq', 'gt', 'gte', 'lt', 'lte'] },
  { name: 'request.body.model', label: 'Model Name', category: 'request', valueType: 'string', description: 'The model requested in the body', operators: ['eq', 'neq', 'contains', 'starts_with', 'ends_with', 'glob', 'regex', 'in'] },
  { name: 'request.headers.*', label: 'Request Header', category: 'request', valueType: 'string', description: 'Any request header value', operators: ['eq', 'neq', 'contains', 'starts_with', 'ends_with', 'glob', 'regex', 'in', 'exists'] },
  { name: 'request.query.*', label: 'Query Parameter', category: 'request', valueType: 'string', description: 'Any query parameter value', operators: ['eq', 'neq', 'contains', 'starts_with', 'ends_with', 'in', 'exists'] },

  // Response fields
  { name: 'response.status', label: 'Response Status', category: 'response', valueType: 'number', description: 'HTTP response status code', operators: ['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'in'] },
  { name: 'response.body.*', label: 'Response Body Field', category: 'response', valueType: 'string', description: 'Any field in the response body', operators: ['eq', 'neq', 'contains', 'starts_with', 'ends_with', 'glob', 'regex', 'exists'] },

  // Token fields
  { name: 'token.id', label: 'Token ID', category: 'token', valueType: 'string', description: 'The virtual token ID', operators: ['eq', 'neq', 'contains', 'in'] },
  { name: 'token.name', label: 'Token Name', category: 'token', valueType: 'string', description: 'The token display name', operators: ['eq', 'neq', 'contains', 'starts_with', 'in'] },
  { name: 'token.purpose', label: 'Token Purpose', category: 'token', valueType: 'string', description: 'The token purpose field', operators: ['eq', 'neq', 'contains', 'in'] },

  // Agent fields
  { name: 'agent.name', label: 'Agent Name', category: 'agent', valueType: 'string', description: 'The name of the agent making requests', operators: ['eq', 'neq', 'contains', 'starts_with', 'in'] },

  // Context fields
  { name: 'context.ip', label: 'Client IP', category: 'context', valueType: 'string', description: 'Client IP address', operators: ['eq', 'neq', 'contains', 'in', 'glob'] },
  { name: 'context.time.hour', label: 'Hour of Day', category: 'context', valueType: 'number', description: 'Current hour (0-23)', operators: ['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'in'] },
  { name: 'context.time.weekday', label: 'Day of Week', category: 'context', valueType: 'string', description: 'Current day (monday, tuesday, etc.)', operators: ['eq', 'neq', 'in'] },
  { name: 'context.time.date', label: 'Current Date', category: 'context', valueType: 'string', description: 'Current date (YYYY-MM-DD)', operators: ['eq', 'neq', 'glob', 'regex'] },

  // Usage fields
  { name: 'usage.spend_today_usd', label: "Today's Spend (USD)", category: 'usage', valueType: 'number', description: 'Total spend today in USD', operators: ['eq', 'gt', 'gte', 'lt', 'lte'] },
  { name: 'usage.requests_today', label: "Today's Requests", category: 'usage', valueType: 'number', description: 'Total requests today', operators: ['eq', 'gt', 'gte', 'lt', 'lte'] },
]

// Operator display names and descriptions
export const OPERATOR_INFO: Record<ConditionOperator, { label: string; description: string }> = {
  eq: { label: 'equals', description: 'Exact match' },
  neq: { label: 'not equals', description: 'Does not match' },
  gt: { label: 'greater than', description: 'Value is greater than' },
  gte: { label: 'greater or equal', description: 'Value is greater or equal to' },
  lt: { label: 'less than', description: 'Value is less than' },
  lte: { label: 'less or equal', description: 'Value is less or equal to' },
  in: { label: 'in list', description: 'Value is in the list (comma-separated)' },
  contains: { label: 'contains', description: 'String contains value' },
  starts_with: { label: 'starts with', description: 'String starts with value' },
  ends_with: { label: 'ends with', description: 'String ends with value' },
  glob: { label: 'matches pattern', description: 'Glob pattern match (* and ?)' },
  regex: { label: 'matches regex', description: 'Regular expression match' },
  exists: { label: 'exists', description: 'Field is present (not null)' },
}

// Guardrail category definitions
export interface GuardrailCategoryDefinition {
  id: string
  label: string
  description: string
  icon: string
  color: string
}

export const GUARDRAIL_CATEGORIES: GuardrailCategoryDefinition[] = [
  { id: 'jailbreak', label: 'Jailbreak Detection', description: 'DAN, prompt injection, bypass attempts', icon: '🚫', color: 'red' },
  { id: 'harmful', label: 'Harmful Content', description: 'CSAM, violence, illegal content', icon: '⚠️', color: 'red' },
  { id: 'code_injection', label: 'Code Injection', description: 'SQL, XSS, command injection', icon: '💉', color: 'orange' },
  { id: 'profanity', label: 'Profanity', description: 'Toxic language, slurs', icon: '💢', color: 'yellow' },
  { id: 'bias', label: 'Bias', description: 'Discriminatory language, stereotypes', icon: '⚖️', color: 'yellow' },
  { id: 'sensitive_topics', label: 'Sensitive Topics', description: 'Politics, religion, controversial subjects', icon: '🎯', color: 'yellow' },
  { id: 'gibberish', label: 'Gibberish', description: 'Nonsense input detection', icon: '🧩', color: 'blue' },
  { id: 'contact_info', label: 'Contact Info', description: 'Emails, phones, social handles', icon: '📧', color: 'blue' },
  { id: 'competitor', label: 'Competitor Mention', description: 'Configurable competitor brand names', icon: '🏢', color: 'blue' },
  { id: 'ip_leakage', label: 'IP Leakage', description: 'NDA, confidential, source code references', icon: '🔒', color: 'red' },
]

// Routing strategy definitions
export interface RoutingStrategyDefinition {
  id: string
  label: string
  description: string
  icon: string
}

export const ROUTING_STRATEGIES: RoutingStrategyDefinition[] = [
  { id: 'lowest_cost', label: 'Lowest Cost', description: 'Pick model with lowest cost', icon: '💰' },
  { id: 'lowest_latency', label: 'Lowest Latency', description: 'Pick model with fastest response', icon: '⚡' },
  { id: 'round_robin', label: 'Round Robin', description: 'Rotate evenly across models', icon: '🔄' },
  { id: 'least_busy', label: 'Least Busy', description: 'Pick model with fewest in-flight requests', icon: '📊' },
  { id: 'weighted_random', label: 'Weighted Random', description: 'Random selection with custom weights', icon: '🎲' },
]

// PII pattern definitions
export interface PII_PATTERN_DEFINITION {
  id: string
  label: string
  description: string
}

export const PII_REGEX_PATTERNS: PII_PATTERN_DEFINITION[] = [
  { id: 'ssn', label: 'SSN', description: 'Social Security Number (xxx-xx-xxxx)' },
  { id: 'credit_card', label: 'Credit Card', description: 'Credit card numbers' },
  { id: 'email', label: 'Email', description: 'Email addresses' },
  { id: 'phone', label: 'Phone Number', description: 'Phone numbers' },
  { id: 'api_key', label: 'API Keys in URLs', description: 'API keys in query strings or headers' },
]

export const PII_NLP_ENTITIES: PII_PATTERN_DEFINITION[] = [
  { id: 'PERSON', label: 'Person Names', description: 'Names of people' },
  { id: 'LOCATION', label: 'Locations', description: 'Addresses, cities, countries' },
  { id: 'PHONE_NUMBER', label: 'Phone Numbers', description: 'Phone numbers (NLP detected)' },
  { id: 'ORGANIZATION', label: 'Organizations', description: 'Company names, institutions' },
  { id: 'MEDICAL', label: 'Medical Terms', description: 'Medical conditions, treatments' },
]

// Policy preset for UI
export interface PolicyPreset {
  id: string
  name: string
  description: string
  icon: string
  rules: Rule[]
}

// Editor mode
export type PolicyEditorMode = 'visual' | 'json'

// Tab types
export type PolicyEditorTab = 'conditions' | 'guardrails' | 'routing' | 'pii' | 'actions'