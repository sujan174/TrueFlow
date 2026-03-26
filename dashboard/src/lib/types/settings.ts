// Gateway Settings
export interface GatewaySettings {
  default_rate_limit?: number;
  default_rate_limit_window?: number;
  hitl_timeout_minutes?: number;
  max_request_body_bytes?: number;
  audit_retention_days?: number;
  enable_response_cache?: boolean;
  enable_guardrails?: boolean;
  slack_webhook_url?: string;
}

// Pricing
export interface PricingEntry {
  id: string;
  provider: string;
  model_pattern: string;
  input_per_m: number;
  output_per_m: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface UpsertPricingRequest {
  provider: string;
  model_pattern: string;
  input_per_m: number;
  output_per_m: number;
}

// Webhooks
export interface Webhook {
  id: string;
  project_id: string;
  url: string;
  events: string[];
  is_active: boolean;
  created_at: string;
  signing_secret?: string;
}

export interface CreateWebhookRequest {
  url: string;
  events?: string[];
}

export interface TestWebhookResponse {
  success: boolean;
  message: string;
}

// Notifications
export interface Notification {
  id: string;
  project_id: string;
  type: string;
  title: string;
  body?: string;
  metadata?: Record<string, unknown>;
  is_read: boolean;
  created_at: string;
}

// Config Export/Import
export interface PolicyExport {
  name: string;
  mode: string;
  phase: string;
  rules: unknown;
  retry?: unknown;
}

export interface TokenExport {
  name: string;
  upstream_url: string;
  policies: string[];
  log_level?: string;
}

export interface ConfigDocument {
  version: string;
  policies: PolicyExport[];
  tokens: TokenExport[];
}

export interface ImportResult {
  policies_created: number;
  policies_updated: number;
  tokens_created: number;
  tokens_updated: number;
}

// Cache Stats
export interface CacheStats {
  cache_key_count: number;
  estimated_size_bytes: number;
  default_ttl_secs: number;
  max_entry_bytes: number;
  namespace_counts: {
    llm_cache: number;
    spend_tracking: number;
    rate_limits: number;
  };
  sample_entries: Array<{
    key: string;
    full_key: string;
    size_bytes: number;
    ttl_secs: number;
  }>;
}

// Webhook Event Types (predefined options)
export const WEBHOOK_EVENT_TYPES = [
  { value: "request.completed", label: "Request Completed", description: "Fired when a request completes successfully" },
  { value: "request.failed", label: "Request Failed", description: "Fired when a request fails" },
  { value: "policy.denied", label: "Policy Denied", description: "Fired when a policy blocks a request" },
  { value: "spend.cap.breached", label: "Spend Cap Breached", description: "Fired when a token exceeds its spend cap" },
  { value: "anomaly.detected", label: "Anomaly Detected", description: "Fired when unusual traffic patterns are detected" },
] as const;