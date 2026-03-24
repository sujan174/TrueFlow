/**
 * @trueflow/sdk — Official TypeScript SDK for the TrueFlow Gateway.
 *
 * @example
 * ```ts
 * import { TrueFlowClient } from "@trueflow/sdk";
 *
 * const client = new TrueFlowClient({ apiKey: "tf_v1_..." });
 *
 * // Use the OpenAI drop-in wrapper
 * const openai = client.openai();
 * const res = await openai.chat.completions.create({
 *   model: "gpt-4o",
 *   messages: [{ role: "user", content: "Hello!" }],
 * });
 *
 * // Use the management API
 * const admin = TrueFlowClient.admin({ adminKey: "your-admin-key" });
 * const tokens = await admin.tokens.list();
 * ```
 *
 * @packageDocumentation
 */

// ── Client ────────────────────────────────────────────────────────────────
export { TrueFlowClient, ScopedClient } from "./client.js";
export type { TrueFlowClientOptions, AdminOptions } from "./client.js";

// ── Errors ────────────────────────────────────────────────────────────────
export {
    TrueFlowError,
    AuthenticationError,
    AccessDeniedError,
    PolicyDeniedError,
    ContentBlockedError,
    NotFoundError,
    RateLimitError,
    ValidationError,
    PayloadTooLargeError,
    SpendCapError,
    GatewayError,
    raiseForStatus,
} from "./error.js";

// ── Types ─────────────────────────────────────────────────────────────────
export type {
    Token,
    TokenCreateOptions,
    TokenCreateResponse,
    Credential,
    CredentialCreateResponse,
    Policy,
    PolicyRule,
    PolicyCreateResponse,
    AuditLog,
    RequestSummary,
    ApprovalRequest,
    ApprovalDecision,
    Service,
    Upstream,
    PaginationOptions,
    CursorPaginationOptions,
    JsonObject,
} from "./types.js";

// ── Resources ─────────────────────────────────────────────────────────────
export { TokensResource } from "./resources/tokens.js";
export { CredentialsResource } from "./resources/credentials.js";
export { PoliciesResource } from "./resources/policies.js";
export { ApprovalsResource } from "./resources/approvals.js";
export { AuditResource } from "./resources/audit.js";
export { ServicesResource } from "./resources/services.js";
export { ApiKeysResource } from "./resources/api-keys.js";
export { WebhooksResource } from "./resources/webhooks.js";
export { GuardrailsResource } from "./resources/guardrails.js";
export { ModelAliasesResource } from "./resources/model-aliases.js";
export { AnalyticsResource } from "./resources/analytics.js";
export { ConfigResource } from "./resources/config.js";
export { BatchesResource } from "./resources/batches.js";
export { FineTuningResource } from "./resources/fine-tuning.js";
export { RealtimeResource, RealtimeSession } from "./resources/realtime.js";
export type { RealtimeConnectOptions } from "./resources/realtime.js";
export { BillingResource } from "./resources/billing.js";
export { ProjectsResource } from "./resources/projects.js";
export { ExperimentsResource } from "./resources/experiments.js";

// ── Helpers ───────────────────────────────────────────────────────────────
export { createOpenAIClient } from "./openai.js";
export type { OpenAIClientLike } from "./openai.js";
export { createAnthropicClient } from "./anthropic.js";
export type { AnthropicClientLike } from "./anthropic.js";
export { HealthPoller } from "./health-poller.js";
export type { HealthPollerOptions } from "./health-poller.js";
export { streamSSE } from "./streaming.js";
export { VERSION } from "./version.js";

// ── Guardrail presets ─────────────────────────────────────────────────────
export {
    PRESET_PROMPT_INJECTION,
    PRESET_CODE_INJECTION,
    PRESET_PII_REDACTION,
    PRESET_PII_ENTERPRISE,
    PRESET_PII_BLOCK,
    PRESET_HIPAA,
    PRESET_PCI,
    PRESET_TOPIC_FENCE,
    PRESET_LENGTH_LIMIT,
} from "./guardrail-presets.js";
export type { GuardrailPreset, GuardrailPresetInfo, GuardrailPresetCategory } from "./guardrail-presets.js";
