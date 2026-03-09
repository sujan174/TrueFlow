use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Policy ───────────────────────────────────────────────────

/// A policy is a named collection of condition→action rules.
///
/// Policies are attached to tokens (or projects/global scope) and evaluated
/// on every proxied request. Each rule is checked in order; the first matching
/// rule's actions are executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub id: Uuid,
    pub name: String,
    /// Evaluation phase: "pre" (before upstream) or "post" (after upstream).
    #[serde(default = "default_phase")]
    pub phase: Phase,
    /// Enforcement mode.
    #[serde(default)]
    pub mode: PolicyMode,
    /// Ordered list of condition→action rules.
    pub rules: Vec<Rule>,
    /// Optional retry configuration for this policy.
    pub retry: Option<RetryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Pre,
    Post,
}

fn default_phase() -> Phase {
    Phase::Pre
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyMode {
    #[default]
    Enforce,
    Shadow,
}

// ── Retry Configuration ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_base_backoff")]
    pub base_backoff_ms: u64,
    #[serde(default = "default_max_backoff")]
    pub max_backoff_ms: u64,
    #[serde(default = "default_jitter")]
    pub jitter_ms: u64,
    #[serde(default = "default_retry_status_codes")]
    pub status_codes: Vec<u16>,
    /// Maximum total time (in milliseconds) for all retry attempts combined.
    /// When set, the retry loop aborts once the deadline is exceeded, even if
    /// max_retries has not been reached. None = no deadline (existing behaviour).
    #[serde(default)]
    pub max_total_timeout_ms: Option<u64>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            base_backoff_ms: default_base_backoff(),
            max_backoff_ms: default_max_backoff(),
            jitter_ms: default_jitter(),
            status_codes: default_retry_status_codes(),
            max_total_timeout_ms: None,
        }
    }
}

fn default_max_retries() -> u32 {
    3
}
fn default_base_backoff() -> u64 {
    500
}
fn default_max_backoff() -> u64 {
    10_000
}
fn default_jitter() -> u64 {
    200
}
fn default_retry_status_codes() -> Vec<u16> {
    vec![429, 500, 502, 503, 504]
}

// ── Rule ─────────────────────────────────────────────────────

/// A single condition→action rule.
///
/// ```json
/// {
///   "when": { "field": "request.body.amount", "op": "gt", "value": 5000 },
///   "then": { "action": "deny", "status": 403, "message": "Too expensive" }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Condition tree that must evaluate to `true` for actions to fire.
    /// If omitted, the rule always matches (catch-all).
    #[serde(default = "Condition::always")]
    pub when: Condition,
    /// One or more actions to execute when the condition matches.
    /// Accepts a single action object or an array of actions.
    #[serde(deserialize_with = "deserialize_actions")]
    pub then: Vec<Action>,
    /// If `true`, the rule is evaluated asynchronously in a background task.
    /// The upstream response is returned immediately; violations are logged
    /// but cannot block the response. Useful for non-blocking audit guardrails.
    /// Defaults to `false` (blocking, same as existing behavior).
    #[serde(default)]
    pub async_check: bool,
}

// ── Condition ────────────────────────────────────────────────

/// A boolean expression tree evaluated against the request context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    /// All children must be true (AND).
    All { all: Vec<Condition> },
    /// At least one child must be true (OR).
    Any { any: Vec<Condition> },
    /// Negation.
    Not { not: Box<Condition> },
    /// Leaf node: compare a field against a value.
    Check {
        field: String,
        op: Operator,
        value: serde_json::Value,
    },
    /// Always true (catch-all). Serialized as `{"always": true}`.
    Always {
        #[serde(default = "default_true")]
        always: bool,
    },
}

fn default_true() -> bool {
    true
}

impl Condition {
    /// Create a catch-all condition that always matches.
    pub fn always() -> Self {
        Condition::Always { always: true }
    }
}

// ── Operator ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Glob,
    Regex,
    Contains,
    Exists,
    StartsWith,
    EndsWith,
}

// ── Action ───────────────────────────────────────────────────

/// An enforcement action to execute when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Explicitly allow the request (no-op, but stops further processing if we implemented rule-level short-circuiting, which we haven't yet, so just a no-op for now).
    Allow,
    /// Block the request with a custom status code and message.
    Deny {
        #[serde(default = "default_deny_status")]
        status: u16,
        #[serde(default = "default_deny_message")]
        message: String,
    },
    /// Trigger Human-in-the-Loop approval.
    RequireApproval {
        #[serde(default = "default_timeout")]
        timeout: String,
        #[serde(default = "default_fallback")]
        fallback: String,
        /// Optional notification config (Slack, webhook, etc.)
        #[serde(default)]
        notify: Option<NotifyConfig>,
    },
    /// Apply a rate limit.
    RateLimit {
        window: String,
        max_requests: u64,
        #[serde(default)]
        key: RateLimitKey,
    },
    /// Artificially delay the request.
    Throttle { delay_ms: u64 },
    /// Redact sensitive data from request or response body.
    Redact {
        #[serde(default)]
        direction: RedactDirection,
        #[serde(default)]
        patterns: Vec<String>,
        #[serde(default)]
        fields: Vec<String>,
        /// What to do when PII is found: "redact" (replace inline) or "block" (deny the request).
        /// Default is "redact" for backwards compatibility.
        #[serde(default)]
        on_match: RedactOnMatch,
    },
    /// Transform the request (set headers, append system prompt, etc.)
    Transform { operations: Vec<TransformOp> },
    /// Override body fields (e.g. force model downgrade).
    Override {
        set_body_fields: std::collections::HashMap<String, serde_json::Value>,
    },
    /// Log a message without blocking.
    Log {
        #[serde(default = "default_log_level")]
        level: String,
        #[serde(default)]
        tags: std::collections::HashMap<String, String>,
    },
    /// Add metadata tags to the audit log entry.
    Tag { key: String, value: String },
    /// Fire an external webhook.
    Webhook {
        url: String,
        #[serde(default = "default_webhook_timeout")]
        timeout_ms: u64,
        #[serde(default)]
        on_fail: OnFail,
    },
    /// Content safety guardrail — block jailbreak/harmful/off-topic prompts.
    ContentFilter {
        /// Block known jailbreak patterns (DAN, prompt injection, etc.).
        #[serde(default = "default_true")]
        block_jailbreak: bool,
        /// Block CSAM and other categorically harmful content.
        #[serde(default = "default_true")]
        block_harmful: bool,
        /// Block code injection attempts (SQL, shell, JS eval, etc.).
        #[serde(default = "default_true")]
        block_code_injection: bool,
        /// Block profanity, slurs, and toxic language.
        #[serde(default)]
        block_profanity: bool,
        /// Block biased, discriminatory, or stereotyping language.
        #[serde(default)]
        block_bias: bool,
        /// Block mentions of competitor products/services.
        #[serde(default)]
        block_competitor_mention: bool,
        /// Block sensitive topics (political opinions, medical/legal advice, religious content).
        #[serde(default)]
        block_sensitive_topics: bool,
        /// Block gibberish, encoding smuggling (base64 blocks, hex dumps, repeated chars).
        #[serde(default)]
        block_gibberish: bool,
        /// Block contact information exposure (addresses, phone formats, auth URLs).
        #[serde(default)]
        block_contact_info: bool,
        /// Block intellectual property leakage (trade secrets, NDA content, confidential markers).
        #[serde(default)]
        block_ip_leakage: bool,
        /// Custom competitor names to block (used with block_competitor_mention).
        #[serde(default)]
        competitor_names: Vec<String>,
        /// If set, only allow prompts that mention at least one of these topics.
        #[serde(default)]
        topic_allowlist: Vec<String>,
        /// Block prompts that mention any of these topics.
        #[serde(default)]
        topic_denylist: Vec<String>,
        /// Additional custom regex patterns to block.
        #[serde(default)]
        custom_patterns: Vec<String>,
        /// Risk score threshold (0.0–1.0) above which the request is blocked.
        /// Default 0.5 — a single jailbreak match scores 0.5.
        #[serde(default = "default_risk_threshold")]
        risk_threshold: f32,
        /// Maximum allowed content length (in characters). 0 = no limit.
        #[serde(default)]
        max_content_length: u32,
    },
    /// Traffic splitting for A/B testing and canary rollouts.
    ///
    /// Example: send 30% of traffic to GPT-4 and 70% to Claude.
    /// Variant selection is deterministic per request_id (same caller always
    /// gets the same variant within a request).
    ///
    /// ```json
    /// {
    ///   "action": "split",
    ///   "experiment": "model-comparison-q1",
    ///   "variants": [
    ///     {"weight": 70, "name": "control",    "set_body_fields": {"model": "gpt-4o"}},
    ///     {"weight": 30, "name": "experiment", "set_body_fields": {"model": "claude-3-5-sonnet-20241022"}}
    ///   ]
    /// }
    /// ```
    Split {
        /// One or more variants with relative weights (do not need to sum to 100).
        variants: Vec<SplitVariant>,
        /// Optional experiment name used to group audit log entries for analysis.
        #[serde(default)]
        experiment: Option<String>,
    },

    /// Dynamic model routing — select the best model from a pool at request time.
    ///
    /// The gateway picks the best healthy model using the chosen strategy, then
    /// rewrites `body.model` and the upstream URL automatically. Zero app‑code
    /// changes required.
    ///
    /// ```json
    /// {
    ///   "action": "dynamic_route",
    ///   "strategy": "lowest_cost",
    ///   "pool": [
    ///     {"model": "gpt-4o-mini",               "upstream_url": "https://api.openai.com"},
    ///     {"model": "claude-3-haiku-20240307",    "upstream_url": "https://api.anthropic.com"},
    ///     {"model": "gemini-2.0-flash",           "upstream_url": "https://generativelanguage.googleapis.com"}
    ///   ]
    /// }
    /// ```
    DynamicRoute {
        /// How to rank and select from the pool.
        strategy: RoutingStrategy,
        /// Ordered list of candidate models with their upstream base URLs.
        pool: Vec<RouteTarget>,
        /// Used when all pool targets are unhealthy.
        #[serde(default)]
        fallback: Option<RouteTarget>,
    },

    /// Validate the LLM response against a JSON Schema.
    ///
    /// Applied in the response (post-flight) phase. If the response body does not
    /// contain valid JSON matching `schema`, the request is denied.
    ///
    /// ```json
    /// {
    ///   "action": "validate_schema",
    ///   "schema": {
    ///     "type": "object",
    ///     "required": ["answer", "confidence"],
    ///     "properties": {
    ///       "answer": { "type": "string" },
    ///       "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
    ///     }
    ///   },
    ///   "not": false,
    ///   "message": "Response must include answer and confidence fields"
    /// }
    /// ```
    ValidateSchema {
        /// The JSON Schema to validate against (draft 2020-12 compatible).
        schema: serde_json::Value,
        /// If true, the rule passes only when the response does NOT match the schema.
        #[serde(default)]
        not: bool,
        /// Custom error message returned when validation fails.
        #[serde(default)]
        message: Option<String>,
    },

    /// Condition-based routing — select an upstream target based on request properties.
    ///
    /// Evaluates branches in order; the first matching branch wins. Falls back to
    /// `fallback` if no branch matches, or denies the request if no fallback is set.
    ///
    /// ```json
    /// {
    ///   "action": "conditional_route",
    ///   "branches": [
    ///     {
    ///       "condition": {"field": "body.model", "op": "eq", "value": "gpt-4o"},
    ///       "target": {"model": "claude-3-5-sonnet-20241022", "upstream_url": "https://api.anthropic.com"}
    ///     },
    ///     {
    ///       "condition": {"field": "body.stream", "op": "eq", "value": true},
    ///       "target": {"model": "gpt-4o-mini", "upstream_url": "https://api.openai.com"}
    ///     }
    ///   ],
    ///   "fallback": {"model": "gpt-4o-mini", "upstream_url": "https://api.openai.com"}
    /// }
    /// ```
    ConditionalRoute {
        /// Ordered list of condition→target branches.
        branches: Vec<RouteBranch>,
        /// Fallback target used when no branch matches. If absent, the request is denied.
        #[serde(default)]
        fallback: Option<RouteTarget>,
    },

    /// Delegate the guardrail check to an external vendor API.
    ///
    /// Applies to both pre-flight and post-flight phases. If the vendor
    /// reports a violation above `threshold`, the `on_fail` action is applied.
    ///
    /// ```json
    /// {
    ///   "action": "external_guardrail",
    ///   "vendor": "azure_content_safety",
    ///   "endpoint": "https://<your-resource>.cognitiveservices.azure.com",
    ///   "api_key_env": "AZURE_CONTENT_SAFETY_KEY",
    ///   "threshold": 4,
    ///   "on_fail": "deny"
    /// }
    /// ```
    ExternalGuardrail {
        /// Which external vendor to call.
        vendor: ExternalVendor,
        /// The vendor's API endpoint URL. For LlamaGuard: your Ollama/vLLM host.
        endpoint: String,
        /// Environment variable name that holds the API key.
        /// The gateway reads this at runtime so keys stay out of policy configs.
        #[serde(default)]
        api_key_env: Option<String>,
        /// Harm score threshold above which the request/response is blocked.
        /// Range and semantics are vendor-specific (Azure: 0–7, AWS: 0.0–1.0).
        #[serde(default = "default_risk_threshold")]
        threshold: f32,
        /// What to do when the vendor flags a violation: \"deny\" (default) or \"log\".
        #[serde(default = "default_fallback")]
        on_fail: String,
    },

    /// Tool-level RBAC — control which tools agents can invoke.
    ///
    /// Evaluated against `request.body.tool_choice` and `request.body.tools[].function.name`.
    /// If any tool in the request matches `blocked_tools`, the request is denied.
    /// If `allowed_tools` is non-empty and any tool is NOT in the list, the request is denied.
    ///
    /// ```json
    /// {
    ///   "action": "tool_scope",
    ///   "allowed_tools": ["jira.read", "jira.search"],
    ///   "blocked_tools": ["stripe.createCharge", "stripe.refund"],
    ///   "deny_message": "Tool not authorized for this agent"
    /// }
    /// ```
    ToolScope {
        /// Whitelist — if non-empty, ONLY these tools are allowed. Empty = allow all.
        #[serde(default)]
        allowed_tools: Vec<String>,
        /// Blacklist — these tools are always denied.
        #[serde(default)]
        blocked_tools: Vec<String>,
        /// Custom deny message returned when a tool is blocked.
        #[serde(default = "default_tool_deny_message")]
        deny_message: String,
    },
}

/// Which external guardrail vendor to call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalVendor {
    /// Azure AI Content Safety — https://azure.microsoft.com/en-us/products/ai-services/ai-content-safety
    AzureContentSafety,
    /// AWS Comprehend toxic language / PII detection
    AwsComprehend,
    /// Self-hosted LlamaGuard via Ollama or vLLM (OpenAI-compatible chat endpoint)
    LlamaGuard,
    /// Palo Alto AIRS (AI Runtime Security) — enterprise prompt scanning
    PaloAltoAirs,
    /// Prompt Security — prompt injection & data leakage detection
    PromptSecurity,
}

// ── ConditionalRoute Sub-types ────────────────────────────────

/// A single condition→target branch in a `ConditionalRoute` action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBranch {
    /// The condition to evaluate against the request body / headers / metadata.
    pub condition: RouteCondition,
    /// The upstream target to route to when the condition matches.
    pub target: RouteTarget,
}

/// A simple boolean condition evaluated against the request context.
///
/// Supports: `eq`, `neq`, `contains`, `starts_with`, `ends_with`, `exists`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCondition {
    /// JSON pointer or named field:
    ///   - `"body.model"` — request body field `model`
    ///   - `"body.messages.0.content"` — first message content
    ///   - `"header.x-user-tier"` — request header value
    ///   - `"metadata.env"` — `x-trueflow-metadata` JSON field
    pub field: String,
    /// Comparison operator: `eq`, `neq`, `contains`, `starts_with`, `ends_with`, `exists`, `regex`
    pub op: String,
    /// The value to compare against. Use `null` for `exists` checks.
    #[serde(default)]
    pub value: serde_json::Value,
}

// ── Action Sub-types ─────────────────────────────────────────

/// A single weighted variant in a Split action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitVariant {
    /// Relative weight (does not need to sum to 100; e.g., 70+30 or 1+1 both mean 50/50).
    pub weight: u32,
    /// Body fields to override when this variant is selected.
    /// Typically `{"model": "claude-3-5-sonnet-20241022"}` to redirect to a different model.
    pub set_body_fields: std::collections::HashMap<String, serde_json::Value>,
    /// Optional variant label shown in experiment analytics (e.g., "control", "experiment").
    #[serde(default)]
    pub name: Option<String>,
}

/// Routing strategy for `DynamicRoute` actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    /// Pick the model with the lowest blended cost from the pricing cache.
    LowestCost,
    /// Pick the model with the lowest p50 latency from the last 24h.
    LowestLatency,
    /// Rotate through the pool sequentially (per-token counter).
    RoundRobin,
    /// Pick the model with the fewest in-flight requests right now.
    LeastBusy,
    /// Randomly select from the pool, weighted by each target's weight field.
    WeightedRandom,
}

/// A single entry in a `DynamicRoute` pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTarget {
    /// Model name to use (e.g. `"gpt-4o-mini"`).
    pub model: String,
    /// Upstream base URL. Provider is auto-detected by the Universal Translator.
    pub upstream_url: String,
    /// Optional credential override for this target.
    #[serde(default)]
    pub credential_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitKey {
    #[default]
    PerToken,
    PerAgent,
    PerIp,
    PerUser,
    Global,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedactDirection {
    Request,
    Response,
    #[default]
    Both,
}

/// What to do when PII is detected in a Redact action.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RedactOnMatch {
    /// Replace the PII in-place with a redaction token (e.g. `[REDACTED_SSN]`). Default.
    #[default]
    Redact,
    /// Deny the request entirely and return a structured error listing detected PII types.
    /// Use this in healthcare/finance contexts where sending PII is a hard policy violation.
    Block,
    /// Replace PII with a vault-backed reversible token (e.g. `tok_pii_cc_a3f1b2...`).
    /// Authorized callers with `pii:rehydrate` scope can recover the original value.
    Tokenize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformOp {
    /// Set a request or response header.
    SetHeader { name: String, value: String },
    /// Remove a request or response header.
    RemoveHeader { name: String },
    /// Append text to the system prompt (creates one if absent).
    AppendSystemPrompt { text: String },
    /// Prepend text to the system prompt (creates one if absent).
    PrependSystemPrompt { text: String },
    /// Find and replace using a regex pattern on the request/response body text.
    /// Equivalent to Portkey's `regex` mutator.
    RegexReplace {
        pattern: String,
        replacement: String,
        /// If true, replace all occurrences. Default: true.
        #[serde(default = "default_true")]
        global: bool,
    },
    /// Set a JSON field by dot-path on the body (`model`, `temperature`, `user.id`, etc.).
    /// Creates intermediate objects as needed.
    SetBodyField {
        path: String,
        value: serde_json::Value,
    },
    /// Remove a JSON field by dot-path from the body.
    RemoveBodyField { path: String },
    /// Inject a synthetic message into `messages` array (request-side only).
    /// Equivalent to Portkey's `addStringBeforeInput`/`addStringAfterInput` in structured form.
    AddToMessageList {
        role: String,
        content: String,
        /// Where to insert: "first" | "last" | "before_last" (default)
        #[serde(default = "default_message_position")]
        position: String,
    },
}

fn default_message_position() -> String {
    "before_last".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyConfig {
    #[serde(rename = "type")]
    pub notify_type: String, // "slack", "webhook"
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFail {
    #[default]
    Allow,
    Deny,
    Log,
}

// ── Defaults ─────────────────────────────────────────────────

fn default_deny_status() -> u16 {
    403
}
fn default_deny_message() -> String {
    "request blocked by policy".to_string()
}
fn default_timeout() -> String {
    "30m".to_string()
}
fn default_fallback() -> String {
    "deny".to_string()
}
fn default_log_level() -> String {
    "warn".to_string()
}
fn default_webhook_timeout() -> u64 {
    5000
}
fn default_risk_threshold() -> f32 {
    0.5
}
fn default_tool_deny_message() -> String {
    "tool not authorized for this agent".to_string()
}

// ── Serde Helpers ────────────────────────────────────────────

/// Deserialize `then` as either a single action or an array of actions.
fn deserialize_actions<'de, D>(deserializer: D) -> Result<Vec<Action>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(Action),
        Many(Vec<Action>),
    }

    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(a) => Ok(vec![a]),
        OneOrMany::Many(v) => Ok(v),
    }
}

// ── Result Types ─────────────────────────────────────────────

/// The result of evaluating all policies against a request.
#[derive(Debug, Default)]
pub struct EvalOutcome {
    /// Blocking actions to execute (in order) before responding.
    pub actions: Vec<TriggeredAction>,
    /// Shadow-mode violations (logged but not enforced).
    pub shadow_violations: Vec<String>,
    /// Async rules that matched — evaluated in a background task after
    /// the response has been sent. Violations are logged but cannot block.
    pub async_triggered: Vec<TriggeredAction>,
}

/// An action that was triggered by a specific policy+rule.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TriggeredAction {
    pub policy_id: Uuid,
    pub policy_name: String,
    pub rule_index: usize,
    pub action: Action,
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Deserialization: Actions ──────────────────────────────

    #[test]
    fn test_deserialize_deny_action() {
        let json = r#"{ "action": "deny", "status": 429, "message": "slow down" }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Deny { status, message } => {
                assert_eq!(status, 429);
                assert_eq!(message, "slow down");
            }
            _ => panic!("Expected Deny, got {:?}", action),
        }
    }

    #[test]
    fn test_deserialize_deny_default_status() {
        let json = r#"{ "action": "deny", "message": "blocked" }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Deny { status, .. } => assert_eq!(status, 403), // default
            _ => panic!("Expected Deny"),
        }
    }

    #[test]
    fn test_deserialize_require_approval() {
        let json = r#"{ "action": "require_approval", "timeout": "30m", "fallback": "deny" }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::RequireApproval {
                timeout,
                fallback,
                notify,
            } => {
                assert_eq!(timeout, "30m");
                assert_eq!(fallback, "deny");
                assert!(notify.is_none());
            }
            _ => panic!("Expected RequireApproval"),
        }
    }

    #[test]
    fn test_deserialize_require_approval_with_notify() {
        let json = r##"{
            "action": "require_approval",
            "timeout": "1h",
            "fallback": "allow",
            "notify": { "type": "slack", "channel": "#alerts" }
        }"##;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::RequireApproval { notify, .. } => {
                let n = notify.unwrap();
                assert_eq!(n.notify_type, "slack");
                assert_eq!(n.channel.as_deref().unwrap(), "#alerts");
            }
            _ => panic!("Expected RequireApproval"),
        }
    }

    #[test]
    fn test_deserialize_rate_limit_default_key() {
        let json = r#"{ "action": "rate_limit", "window": "5m", "max_requests": 50 }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::RateLimit {
                window,
                max_requests,
                key,
            } => {
                assert_eq!(window, "5m");
                assert_eq!(max_requests, 50);
                assert!(matches!(key, RateLimitKey::PerToken)); // default
            }
            _ => panic!("Expected RateLimit"),
        }
    }

    #[test]
    fn test_deserialize_rate_limit_all_keys() {
        for (key_str, expected) in [
            ("per_token", RateLimitKey::PerToken),
            ("per_agent", RateLimitKey::PerAgent),
            ("per_ip", RateLimitKey::PerIp),
            ("per_user", RateLimitKey::PerUser),
            ("global", RateLimitKey::Global),
        ] {
            let json = format!(
                r#"{{ "action": "rate_limit", "window": "1m", "max_requests": 10, "key": "{}" }}"#,
                key_str
            );
            let action: Action = serde_json::from_str(&json).unwrap();
            match action {
                Action::RateLimit { key, .. } => assert_eq!(
                    std::mem::discriminant(&key),
                    std::mem::discriminant(&expected),
                    "Key mismatch for {}",
                    key_str
                ),
                _ => panic!("Expected RateLimit for key={}", key_str),
            }
        }
    }

    #[test]
    fn test_deserialize_throttle_action() {
        let json = r#"{ "action": "throttle", "delay_ms": 2000 }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Throttle { delay_ms } => assert_eq!(delay_ms, 2000),
            _ => panic!("Expected Throttle"),
        }
    }

    #[test]
    fn test_deserialize_override_multiple_fields() {
        let json = r#"{
            "action": "override",
            "set_body_fields": {
                "model": "gpt-3.5-turbo",
                "max_tokens": 512,
                "temperature": 0.5
            }
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Override { set_body_fields } => {
                assert_eq!(set_body_fields.len(), 3);
                assert_eq!(set_body_fields["model"], "gpt-3.5-turbo");
                assert_eq!(set_body_fields["max_tokens"], 512);
                assert_eq!(set_body_fields["temperature"], 0.5);
            }
            _ => panic!("Expected Override"),
        }
    }

    #[test]
    fn test_deserialize_log_action() {
        let json = r#"{ "action": "log", "level": "error" }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Log { level, tags } => {
                assert_eq!(level, "error");
                assert!(tags.is_empty()); // default
            }
            _ => panic!("Expected Log"),
        }
    }

    #[test]
    fn test_deserialize_log_with_tags() {
        let json = r#"{ "action": "log", "level": "info", "tags": {"env": "prod", "team": "ml"} }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Log { tags, .. } => {
                assert_eq!(tags.get("env").unwrap(), "prod");
                assert_eq!(tags.get("team").unwrap(), "ml");
            }
            _ => panic!("Expected Log"),
        }
    }

    #[test]
    fn test_deserialize_tag_action() {
        let json = r#"{ "action": "tag", "key": "risk", "value": "high" }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Tag { key, value } => {
                assert_eq!(key, "risk");
                assert_eq!(value, "high");
            }
            _ => panic!("Expected Tag"),
        }
    }

    #[test]
    fn test_deserialize_webhook_action() {
        let json = r#"{
            "action": "webhook",
            "url": "https://hooks.example.com/alert",
            "timeout_ms": 5000
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Webhook {
                url, timeout_ms, ..
            } => {
                assert_eq!(url, "https://hooks.example.com/alert");
                assert_eq!(timeout_ms, 5000);
            }
            _ => panic!("Expected Webhook"),
        }
    }

    #[test]
    fn test_deserialize_redact_action() {
        let json = r#"{
            "action": "redact",
            "direction": "request",
            "patterns": ["ssn", "email"]
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Redact {
                direction,
                patterns,
                ..
            } => {
                assert!(matches!(direction, RedactDirection::Request));
                assert_eq!(patterns.len(), 2);
            }
            _ => panic!("Expected Redact"),
        }
    }

    #[test]
    fn test_deserialize_transform_action() {
        let json = r#"{
            "action": "transform",
            "operations": [
                {"type": "set_header", "name": "X-Custom", "value": "true"},
                {"type": "append_system_prompt", "text": "Be safe"}
            ]
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::Transform { operations } => {
                assert_eq!(operations.len(), 2);
                assert!(matches!(&operations[0], TransformOp::SetHeader { .. }));
                assert!(matches!(
                    &operations[1],
                    TransformOp::AppendSystemPrompt { .. }
                ));
            }
            _ => panic!("Expected Transform"),
        }
    }

    // ── Deserialization: Conditions ───────────────────────────

    #[test]
    fn test_deserialize_check_condition() {
        let json = r#"{ "field": "request.method", "op": "eq", "value": "POST" }"#;
        let cond: Condition = serde_json::from_str(json).unwrap();
        match cond {
            Condition::Check { field, op, value } => {
                assert_eq!(field, "request.method");
                assert!(matches!(op, Operator::Eq));
                assert_eq!(value, "POST");
            }
            _ => panic!("Expected Check"),
        }
    }

    #[test]
    fn test_deserialize_always_condition() {
        let json = r#"{ "always": true }"#;
        let cond: Condition = serde_json::from_str(json).unwrap();
        match cond {
            Condition::Always { always } => assert!(always),
            _ => panic!("Expected Always"),
        }
    }

    #[test]
    fn test_deserialize_not_condition() {
        let json = r#"{
            "not": { "field": "request.method", "op": "eq", "value": "GET" }
        }"#;
        let cond: Condition = serde_json::from_str(json).unwrap();
        match cond {
            Condition::Not { not } => {
                assert!(matches!(*not, Condition::Check { .. }));
            }
            _ => panic!("Expected Not"),
        }
    }

    #[test]
    fn test_deserialize_nested_any_all() {
        let json = r#"{
            "any": [
                { "field": "request.method", "op": "eq", "value": "DELETE" },
                {
                    "all": [
                        { "field": "request.path", "op": "glob", "value": "/v1/charges*" },
                        { "field": "request.body.amount", "op": "gt", "value": 5000 }
                    ]
                }
            ]
        }"#;
        let cond: Condition = serde_json::from_str(json).unwrap();
        if let Condition::Any { any } = cond {
            assert_eq!(any.len(), 2);
            assert!(matches!(any[1], Condition::All { .. }));
        } else {
            panic!("Expected Any");
        }
    }

    // ── Deserialization: All Operators ────────────────────────

    #[test]
    fn test_deserialize_all_operators() {
        let operators = vec![
            ("eq", "Eq"),
            ("neq", "Neq"),
            ("gt", "Gt"),
            ("gte", "Gte"),
            ("lt", "Lt"),
            ("lte", "Lte"),
            ("in", "In"),
            ("glob", "Glob"),
            ("regex", "Regex"),
            ("contains", "Contains"),
            ("exists", "Exists"),
            ("starts_with", "StartsWith"),
            ("ends_with", "EndsWith"),
        ];
        for (op_str, _label) in operators {
            let json = format!(
                r#"{{ "field": "request.method", "op": "{}", "value": "test" }}"#,
                op_str
            );
            let result: Result<Condition, _> = serde_json::from_str(&json);
            assert!(result.is_ok(), "Failed to deserialize operator: {}", op_str);
        }
    }

    // ── Deserialization: Policy-level ─────────────────────────

    #[test]
    fn test_policy_defaults() {
        // phase defaults to "pre", mode defaults based on explicit field
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "minimal",
            "mode": "enforce",
            "rules": []
        }"#;
        let policy: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.phase, Phase::Pre);
        assert_eq!(policy.mode, PolicyMode::Enforce);
        assert!(policy.rules.is_empty());
        assert!(policy.retry.is_none());
    }

    #[test]
    fn test_policy_post_phase() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000003",
            "name": "response-checker",
            "phase": "post",
            "mode": "enforce",
            "rules": [{
                "when": { "field": "response.status", "op": "gte", "value": 500 },
                "then": { "action": "log", "level": "error" }
            }]
        }"#;
        let policy: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.phase, Phase::Post);
    }

    #[test]
    fn test_policy_single_action_desugars_to_vec() {
        // "then" can be a single action or an array
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000004",
            "name": "single",
            "mode": "enforce",
            "rules": [{
                "when": { "always": true },
                "then": { "action": "deny", "message": "nope" }
            }]
        }"#;
        let policy: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.rules[0].then.len(), 1);
    }

    #[test]
    fn test_policy_multiple_rules() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000005",
            "name": "multi-rule",
            "mode": "enforce",
            "rules": [
                {
                    "when": { "field": "request.method", "op": "eq", "value": "DELETE" },
                    "then": { "action": "deny", "message": "deletes blocked" }
                },
                {
                    "when": { "field": "request.body.model", "op": "eq", "value": "gpt-4" },
                    "then": { "action": "rate_limit", "window": "1m", "max_requests": 5 }
                },
                {
                    "when": { "always": true },
                    "then": { "action": "log", "level": "info" }
                }
            ]
        }"#;
        let policy: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.rules.len(), 3);
        assert!(matches!(policy.rules[0].then[0], Action::Deny { .. }));
        assert!(matches!(policy.rules[1].then[0], Action::RateLimit { .. }));
        assert!(matches!(policy.rules[2].then[0], Action::Log { .. }));
    }

    // ── Tests: Retry Config ──────────────────────────────────

    #[test]
    fn test_retry_config_serialization() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000099",
            "name": "retry-policy",
            "mode": "enforce",
            "rules": [],
            "retry": {
                "max_retries": 5,
                "base_backoff_ms": 100,
                "status_codes": [429, 503]
            }
        }"#;
        let policy: Policy = serde_json::from_str(json).unwrap();
        let retry = policy.retry.unwrap();
        assert_eq!(retry.max_retries, 5);
        assert_eq!(retry.base_backoff_ms, 100);
        assert_eq!(retry.max_backoff_ms, 10000); // default
        assert_eq!(retry.status_codes, vec![429, 503]);
    }

    // ── Full Scenario: Stripe HITL policy ────────────────────

    #[test]
    fn test_full_stripe_hitl_policy_deserialization() {
        let json = r##"{
            "id": "00000000-0000-0000-0000-000000000010",
            "name": "stripe-high-value-approval",
            "phase": "pre",
            "mode": "enforce",
            "rules": [{
                "when": {
                    "all": [
                        { "field": "request.path", "op": "glob", "value": "/v1/charges*" },
                        { "field": "request.method", "op": "eq", "value": "POST" },
                        { "field": "request.body.amount", "op": "gt", "value": 5000 }
                    ]
                },
                "then": [
                    { "action": "require_approval", "timeout": "30m", "fallback": "deny",
                      "notify": { "type": "slack", "channel": "#payments-review" }},
                    { "action": "tag", "key": "risk", "value": "high" }
                ]
            }]
        }"##;
        let policy: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.name, "stripe-high-value-approval");
        assert_eq!(policy.phase, Phase::Pre);
        assert_eq!(policy.mode, PolicyMode::Enforce);
        assert_eq!(policy.rules.len(), 1);

        let rule = &policy.rules[0];
        assert!(matches!(rule.when, Condition::All { .. }));
        assert_eq!(rule.then.len(), 2);
        assert!(matches!(rule.then[0], Action::RequireApproval { .. }));
        assert!(matches!(rule.then[1], Action::Tag { .. }));
    }

    // ── Full Scenario: Model governance policy ───────────────

    #[test]
    fn test_full_model_governance_deserialization() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000011",
            "name": "expensive-model-governance",
            "mode": "enforce",
            "rules": [
                {
                    "when": { "field": "request.body.model", "op": "in", "value": ["gpt-4", "gpt-4-turbo"] },
                    "then": { "action": "rate_limit", "window": "1m", "max_requests": 10, "key": "per_token" }
                },
                {
                    "when": {
                        "all": [
                            { "field": "request.body.model", "op": "eq", "value": "gpt-4" },
                            { "field": "usage.spend_today_usd", "op": "gt", "value": 50.0 }
                        ]
                    },
                    "then": { "action": "override", "set_body_fields": { "model": "gpt-3.5-turbo" } }
                }
            ]
        }"#;
        let policy: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.rules.len(), 2);

        // First rule: rate limit for expensive models
        match &policy.rules[0].then[0] {
            Action::RateLimit {
                window,
                max_requests,
                key,
            } => {
                assert_eq!(window, "1m");
                assert_eq!(*max_requests, 10);
                assert!(matches!(key, RateLimitKey::PerToken));
            }
            _ => panic!("Expected RateLimit"),
        }

        // Second rule: override to cheaper model
        match &policy.rules[1].then[0] {
            Action::Override { set_body_fields } => {
                assert_eq!(set_body_fields["model"], "gpt-3.5-turbo");
            }
            _ => panic!("Expected Override"),
        }
    }

    // ── Feature 9: ExternalGuardrail Deserialization ─────────────

    #[test]
    fn test_deserialize_external_guardrail_azure() {
        let json = r#"{
            "action": "external_guardrail",
            "vendor": "azure_content_safety",
            "endpoint": "https://my-resource.cognitiveservices.azure.com",
            "api_key_env": "AZURE_KEY",
            "threshold": 4.0,
            "on_fail": "deny"
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::ExternalGuardrail {
                vendor,
                endpoint,
                api_key_env,
                threshold,
                on_fail,
            } => {
                assert_eq!(vendor, ExternalVendor::AzureContentSafety);
                assert_eq!(endpoint, "https://my-resource.cognitiveservices.azure.com");
                assert_eq!(api_key_env, Some("AZURE_KEY".to_string()));
                assert!((threshold - 4.0).abs() < 0.01);
                assert_eq!(on_fail, "deny");
            }
            _ => panic!("Expected ExternalGuardrail, got {:?}", action),
        }
    }

    #[test]
    fn test_deserialize_external_guardrail_aws() {
        let json = r#"{
            "action": "external_guardrail",
            "vendor": "aws_comprehend",
            "endpoint": "https://comprehend-proxy.example.com/detect-toxic",
            "api_key_env": "AWS_TOKEN",
            "threshold": 0.8
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::ExternalGuardrail {
                vendor,
                threshold,
                on_fail,
                ..
            } => {
                assert_eq!(vendor, ExternalVendor::AwsComprehend);
                assert!((threshold - 0.8).abs() < 0.01);
                assert_eq!(on_fail, "deny"); // default fallback
            }
            _ => panic!("Expected ExternalGuardrail"),
        }
    }

    #[test]
    fn test_deserialize_external_guardrail_llama_guard() {
        let json = r#"{
            "action": "external_guardrail",
            "vendor": "llama_guard",
            "endpoint": "http://localhost:11434",
            "on_fail": "log"
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::ExternalGuardrail {
                vendor,
                endpoint,
                api_key_env,
                threshold,
                on_fail,
            } => {
                assert_eq!(vendor, ExternalVendor::LlamaGuard);
                assert_eq!(endpoint, "http://localhost:11434");
                assert!(
                    api_key_env.is_none(),
                    "LlamaGuard should not require api_key_env"
                );
                assert!(
                    (threshold - 0.5).abs() < 0.01,
                    "default threshold should be 0.5"
                );
                assert_eq!(on_fail, "log");
            }
            _ => panic!("Expected ExternalGuardrail"),
        }
    }

    #[test]
    fn test_deserialize_external_guardrail_defaults() {
        // Minimal config — threshold and on_fail should use defaults
        let json = r#"{
            "action": "external_guardrail",
            "vendor": "azure_content_safety",
            "endpoint": "https://example.com"
        }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        match action {
            Action::ExternalGuardrail {
                threshold,
                on_fail,
                api_key_env,
                ..
            } => {
                assert!(
                    (threshold - 0.5).abs() < 0.01,
                    "default threshold should be 0.5"
                );
                assert_eq!(on_fail, "deny", "default on_fail should be 'deny'");
                assert!(api_key_env.is_none(), "api_key_env should default to None");
            }
            _ => panic!("Expected ExternalGuardrail"),
        }
    }

    #[test]
    fn test_external_vendor_round_trip() {
        // Verify ExternalVendor serializes and deserializes correctly
        let vendors = vec![
            (
                ExternalVendor::AzureContentSafety,
                "\"azure_content_safety\"",
            ),
            (ExternalVendor::AwsComprehend, "\"aws_comprehend\""),
            (ExternalVendor::LlamaGuard, "\"llama_guard\""),
        ];
        for (variant, expected_json) in vendors {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                serialized, expected_json,
                "serialization mismatch for {:?}",
                variant
            );
            let deserialized: ExternalVendor = serde_json::from_str(&serialized).unwrap();
            assert_eq!(
                deserialized, variant,
                "round-trip mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn test_external_guardrail_in_rule_with_async_check() {
        // ExternalGuardrail should work as an async_check rule
        let json = r#"{
            "when": { "field": "request.method", "op": "eq", "value": "POST" },
            "then": {
                "action": "external_guardrail",
                "vendor": "azure_content_safety",
                "endpoint": "https://example.com",
                "threshold": 3.0
            },
            "async_check": true
        }"#;
        let rule: Rule = serde_json::from_str(json).unwrap();
        assert!(rule.async_check);
        match &rule.then[0] {
            Action::ExternalGuardrail {
                vendor, threshold, ..
            } => {
                assert_eq!(*vendor, ExternalVendor::AzureContentSafety);
                assert!((threshold - 3.0).abs() < 0.01);
            }
            _ => panic!("Expected ExternalGuardrail action in rule"),
        }
    }

    #[test]
    fn test_action_name_includes_external_guardrail() {
        // Verify the action name mapper recognizes ExternalGuardrail
        let action = Action::ExternalGuardrail {
            vendor: ExternalVendor::LlamaGuard,
            endpoint: "http://localhost:11434".to_string(),
            api_key_env: None,
            threshold: 0.5,
            on_fail: "deny".to_string(),
        };
        // Just verify it doesn't panic — the actual name is tested in engine::tests
        let _ = format!("{:?}", action);
    }
}
