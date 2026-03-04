/**
 * Main TrueFlow client — the entry point for all SDK interactions.
 *
 * @example
 * ```ts
 * import { TrueFlowClient } from "@trueflow/sdk";
 *
 * const client = new TrueFlowClient({ apiKey: "tf_v1_..." });
 *
 * // Use the management API
 * const tokens = await client.tokens.list();
 *
 * // Use the OpenAI drop-in wrapper
 * const openai = client.openai();
 * const res = await openai.chat.completions.create({
 *   model: "gpt-4o",
 *   messages: [{ role: "user", content: "Hello!" }],
 * });
 *
 * // Admin operations
 * const admin = TrueFlowClient.admin({ adminKey: "your-admin-key" });
 * await admin.policies.list();
 * ```
 *
 * @module
 */

import { HttpClient } from "./http.js";
import { GatewayError } from "./error.js";
import { createOpenAIClient, type OpenAIClientLike } from "./openai.js";
import { createAnthropicClient, type AnthropicClientLike } from "./anthropic.js";

// Resources
import { TokensResource } from "./resources/tokens.js";
import { CredentialsResource } from "./resources/credentials.js";
import { PoliciesResource } from "./resources/policies.js";
import { ApprovalsResource } from "./resources/approvals.js";
import { AuditResource } from "./resources/audit.js";
import { ServicesResource } from "./resources/services.js";
import { ApiKeysResource } from "./resources/api-keys.js";
import { WebhooksResource } from "./resources/webhooks.js";
import { GuardrailsResource } from "./resources/guardrails.js";
import { ModelAliasesResource } from "./resources/model-aliases.js";
import { AnalyticsResource } from "./resources/analytics.js";
import { ConfigResource } from "./resources/config.js";
import { BatchesResource } from "./resources/batches.js";
import { FineTuningResource } from "./resources/fine-tuning.js";
import { RealtimeResource } from "./resources/realtime.js";
import { BillingResource } from "./resources/billing.js";
import { ProjectsResource } from "./resources/projects.js";
import { ExperimentsResource } from "./resources/experiments.js";
import { PromptsResource } from "./resources/prompts.js";

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/** Generate a UUID v4 — works in Node 18+, Deno, Bun, and all modern browsers. */
function generateUUID(): string {
    // crypto.randomUUID is available in Node 19+, all browsers, Deno, Bun
    // For Node 18, fall back to crypto.getRandomValues
    try {
        return crypto.randomUUID();
    } catch {
        // Fallback: manual UUID v4 generation using getRandomValues
        const bytes = new Uint8Array(16);
        crypto.getRandomValues(bytes);
        bytes[6] = (bytes[6]! & 0x0f) | 0x40; // version 4
        bytes[8] = (bytes[8]! & 0x3f) | 0x80; // variant 1
        const hex = [...bytes].map(b => b.toString(16).padStart(2, "0")).join("");
        return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Client options
// ────────────────────────────────────────────────────────────────────────────

/** Options for constructing an `TrueFlowClient`. */
export interface TrueFlowClientOptions {
    /**
     * TrueFlow virtual token.
     *
     * Falls back to `process.env.TRUEFLOW_API_KEY` if not provided.
     */
    apiKey?: string;
    /**
     * Gateway URL.
     *
     * Falls back to `process.env.TRUEFLOW_GATEWAY_URL`, then `"http://localhost:8443"`.
     */
    gatewayUrl?: string;
    /** Optional name for this agent (appears in audit logs). */
    agentName?: string;
    /** Idempotency key for request deduplication. */
    idempotencyKey?: string;
    /** Request timeout in milliseconds (default: 30000). */
    timeoutMs?: number;
    /** Number of retries on transient failures (default: 2). */
    maxRetries?: number;
}

/** Options for creating an admin client. */
export interface AdminOptions {
    /**
     * Admin key (`X-Admin-Key` header value).
     *
     * Falls back to `process.env.TRUEFLOW_ADMIN_KEY`.
     */
    adminKey?: string;
    /** Gateway URL (same fallback behavior as `TrueFlowClientOptions.gatewayUrl`). */
    gatewayUrl?: string;
    /** Request timeout in milliseconds. */
    timeoutMs?: number;
    /** Number of retries. */
    maxRetries?: number;
}

// ────────────────────────────────────────────────────────────────────────────
// ScopedClient — lightweight wrapper for BYOK / tracing / guardrails
// ────────────────────────────────────────────────────────────────────────────

/**
 * A scoped HTTP client that merges extra headers into every request.
 * Returned by `withUpstreamKey()`, `trace()`, and `withGuardrails()`.
 */
export class ScopedClient {
    /** @internal */
    constructor(
        private readonly http: HttpClient,
        private readonly extraHeaders: Record<string, string>,
    ) { }

    private mergeHeaders(headers?: Record<string, string>): Record<string, string> {
        return { ...headers, ...this.extraHeaders };
    }

    /** Send a GET request through the gateway with scoped headers. */
    async get(path: string, options: { headers?: Record<string, string>; params?: Record<string, string | number | boolean | undefined> } = {}): Promise<Response> {
        return this.http.get(path, { ...options, headers: this.mergeHeaders(options.headers) });
    }

    /** Send a POST request with a JSON body through the gateway with scoped headers. */
    async post(path: string, body?: unknown, options: { headers?: Record<string, string> } = {}): Promise<Response> {
        return this.http.post(path, body, { headers: this.mergeHeaders(options.headers) });
    }

    /** Send a PUT request with a JSON body through the gateway with scoped headers. */
    async put(path: string, body?: unknown, options: { headers?: Record<string, string> } = {}): Promise<Response> {
        return this.http.put(path, body, { headers: this.mergeHeaders(options.headers) });
    }

    /** Send a PATCH request through the gateway with scoped headers. */
    async patch(path: string, body?: unknown, options: { headers?: Record<string, string> } = {}): Promise<Response> {
        return this.http.patch(path, body, { headers: this.mergeHeaders(options.headers) });
    }

    /** Send a DELETE request through the gateway with scoped headers. */
    async delete(path: string, options: { headers?: Record<string, string> } = {}): Promise<Response> {
        return this.http.delete(path, { headers: this.mergeHeaders(options.headers) });
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Main client
// ────────────────────────────────────────────────────────────────────────────

/**
 * The official TrueFlow TypeScript client.
 *
 * @example
 * ```ts
 * import { TrueFlowClient } from "@trueflow/sdk";
 *
 * const client = new TrueFlowClient({ apiKey: "tf_v1_..." });
 * const tokens = await client.tokens.list();
 * ```
 */
export class TrueFlowClient {
    /** The gateway URL this client is connected to. */
    readonly gatewayUrl: string;
    /** The API key used for authentication. */
    readonly apiKey: string;

    /** @internal */
    readonly _http: HttpClient;

    // ── Lazy resource accessors (cached after first access) ────────────────

    private _tokens?: TokensResource;
    private _credentials?: CredentialsResource;
    private _policies?: PoliciesResource;
    private _approvals?: ApprovalsResource;
    private _audit?: AuditResource;
    private _services?: ServicesResource;
    private _apiKeys?: ApiKeysResource;
    private _webhooks?: WebhooksResource;
    private _guardrails?: GuardrailsResource;
    private _modelAliases?: ModelAliasesResource;
    private _analytics?: AnalyticsResource;
    private _config?: ConfigResource;
    private _batches?: BatchesResource;
    private _fineTuning?: FineTuningResource;
    private _realtime?: RealtimeResource;
    private _billing?: BillingResource;
    private _projects?: ProjectsResource;
    private _experiments?: ExperimentsResource;
    private _prompts?: PromptsResource;

    /**
     * Create a new TrueFlow client.
     *
     * @example
     * ```ts
     * const client = new TrueFlowClient({
     *   apiKey: "tf_v1_...",
     *   gatewayUrl: "https://gateway.mycompany.com",
     *   agentName: "research-bot",
     * });
     * ```
     */
    constructor(options: TrueFlowClientOptions = {}) {
        const env = typeof process !== "undefined" ? process.env : ({} as Record<string, string | undefined>);
        this.apiKey = options.apiKey ?? env["TRUEFLOW_API_KEY"] ?? "";
        this.gatewayUrl = (options.gatewayUrl ?? env["TRUEFLOW_GATEWAY_URL"] ?? "http://localhost:8443").replace(/\/+$/, "");

        const headers: Record<string, string> = {
            Authorization: `Bearer ${this.apiKey}`,
        };
        if (options.agentName) {
            headers["X-Agent-Name"] = options.agentName;
        }
        if (options.idempotencyKey) {
            headers["Idempotency-Key"] = options.idempotencyKey;
        }

        this._http = new HttpClient({
            baseUrl: this.gatewayUrl,
            headers,
            timeoutMs: options.timeoutMs,
            maxRetries: options.maxRetries,
        });
    }

    // ── Static factories ────────────────────────────────────────────────────

    /**
     * Create an admin client for Management API operations.
     *
     * @example
     * ```ts
     * const admin = TrueFlowClient.admin({ adminKey: "your-admin-key" });
     * const policies = await admin.policies.list();
     * ```
     */
    static admin(options: AdminOptions = {}): TrueFlowClient {
        const env = typeof process !== "undefined" ? process.env : ({} as Record<string, string | undefined>);
        const adminKey = options.adminKey ?? env["TRUEFLOW_ADMIN_KEY"] ?? "";
        const gatewayUrl = options.gatewayUrl ?? env["TRUEFLOW_GATEWAY_URL"] ?? "http://localhost:8443";

        // Admin clients use X-Admin-Key instead of Bearer token
        const client = new TrueFlowClient({ gatewayUrl, timeoutMs: options.timeoutMs, maxRetries: options.maxRetries });
        // Override the HTTP client to use admin auth
        (client as { _http: HttpClient })._http = new HttpClient({
            baseUrl: gatewayUrl,
            headers: { "X-Admin-Key": adminKey },
            timeoutMs: options.timeoutMs,
            maxRetries: options.maxRetries,
        });
        (client as { apiKey: string }).apiKey = adminKey;
        return client;
    }

    // ── Provider wrappers ───────────────────────────────────────────────────

    /**
     * Returns a configured OpenAI client that routes through the TrueFlow gateway.
     *
     * Requires the `openai` package: `npm install openai`
     *
     * @example
     * ```ts
     * const openai = client.openai();
     * const res = await openai.chat.completions.create({
     *   model: "gpt-4o",
     *   messages: [{ role: "user", content: "Hello!" }],
     * });
     * ```
     */
    openai(): OpenAIClientLike {
        return createOpenAIClient(this.gatewayUrl, this.apiKey);
    }

    /**
     * Returns a configured Anthropic client that routes through the TrueFlow gateway.
     *
     * Requires the `@anthropic-ai/sdk` package: `npm install @anthropic-ai/sdk`
     *
     * @example
     * ```ts
     * const anthropic = client.anthropic();
     * const msg = await anthropic.messages.create({
     *   model: "claude-sonnet-4-20250514",
     *   max_tokens: 1024,
     *   messages: [{ role: "user", content: "Hello!" }],
     * });
     * ```
     */
    anthropic(): AnthropicClientLike {
        return createAnthropicClient(this.gatewayUrl, this.apiKey);
    }

    // ── Scoped clients ─────────────────────────────────────────────────────

    /**
     * Create a scoped client for Passthrough (BYOK) mode.
     *
     * When the token has no stored credential, the gateway forwards the key
     * you supply here directly to the upstream.
     *
     * @example
     * ```ts
     * const byok = client.withUpstreamKey("sk-my-openai-key");
     * await byok.post("/v1/chat/completions", { model: "gpt-4o", messages: [...] });
     * ```
     */
    withUpstreamKey(key: string, header = "Bearer"): ScopedClient {
        const authValue = header ? `${header} ${key}` : key;
        return new ScopedClient(this._http, { "X-Real-Authorization": authValue });
    }

    /**
     * Create a scoped client that injects distributed tracing headers.
     *
     * All requests are tagged with the given session and span IDs,
     * which appear in audit logs for correlating multi-step agent workflows.
     *
     * @example
     * ```ts
     * const traced = client.trace({
     *   sessionId: "agent-run-42",
     *   properties: { env: "prod", customer: "acme" },
     * });
     * await traced.post("/v1/chat/completions", { ... }); // step 1
     * await traced.post("/v1/chat/completions", { ... }); // step 2
     * ```
     */
    trace(options: { sessionId?: string; parentSpanId?: string; properties?: Record<string, unknown> } = {}): ScopedClient {
        const headers: Record<string, string> = {};
        headers["x-session-id"] = options.sessionId ?? generateUUID();
        if (options.parentSpanId) {
            headers["x-parent-span-id"] = options.parentSpanId;
        }
        if (options.properties) {
            headers["x-properties"] = JSON.stringify(options.properties);
        }
        return new ScopedClient(this._http, headers);
    }

    /**
     * Create a scoped client that attaches guardrails on a per-request basis.
     *
     * @example
     * ```ts
     * const guarded = client.withGuardrails(["pii_redaction", "prompt_injection"]);
     * await guarded.post("/v1/chat/completions", { ... });
     * ```
     */
    withGuardrails(presets: string[]): ScopedClient {
        if (presets.length === 0) return new ScopedClient(this._http, {});
        return new ScopedClient(this._http, { "X-TrueFlow-Guardrails": presets.join(",") });
    }

    // ── Health check ───────────────────────────────────────────────────────

    /**
     * Returns `true` if the gateway is reachable and healthy, `false` otherwise.
     *
     * @example
     * ```ts
     * if (await client.isHealthy()) {
     *   const openai = client.openai(); // use gateway
     * } else {
     *   // use fallback
     * }
     * ```
     */
    async isHealthy(options: { timeoutMs?: number } = {}): Promise<boolean> {
        const timeout = options.timeoutMs ?? 3000;
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeout);
        try {
            const url = `${this.gatewayUrl}/healthz`;
            const res = await fetch(url, { signal: controller.signal });
            return res.status < 500;
        } catch {
            return false;
        } finally {
            clearTimeout(timer);
        }
    }

    /**
     * Check gateway health. Returns a status object or throws `GatewayError`.
     *
     * @example
     * ```ts
     * const health = await client.health();
     * console.log(health.status); // "ok"
     * ```
     */
    async health(options: { timeoutMs?: number } = {}): Promise<{ status: string; gatewayUrl: string; httpStatus: number }> {
        const timeout = options.timeoutMs ?? 5000;
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeout);
        try {
            const url = `${this.gatewayUrl}/healthz`;
            const res = await fetch(url, { signal: controller.signal });
            return { status: "ok", gatewayUrl: this.gatewayUrl, httpStatus: res.status };
        } catch (error) {
            if (error instanceof DOMException && error.name === "AbortError") {
                throw new GatewayError(`Gateway health check timed out after ${timeout}ms`);
            }
            throw new GatewayError(`Gateway unreachable at ${this.gatewayUrl}`);
        } finally {
            clearTimeout(timer);
        }
    }

    /**
     * Automatic gateway fallback. Checks health and returns either a gateway-backed
     * OpenAI client or the provided fallback.
     *
     * @example
     * ```ts
     * import OpenAI from "openai";
     *
     * const fallback = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
     * const openai = await client.withFallback(fallback);
     * const res = await openai.chat.completions.create({
     *   model: "gpt-4o",
     *   messages: [{ role: "user", content: "Hello!" }],
     * });
     * ```
     */
    async withFallback<T>(fallback: T, options: { healthTimeoutMs?: number } = {}): Promise<OpenAIClientLike | T> {
        const healthy = await this.isHealthy({ timeoutMs: options.healthTimeoutMs ?? 3000 });
        if (healthy) {
            return this.openai();
        }
        console.warn(
            `TrueFlow gateway at ${this.gatewayUrl} is unreachable — ` +
            "using fallback client. Requests will bypass policy enforcement and audit logging.",
        );
        return fallback;
    }

    // ── Raw HTTP methods ────────────────────────────────────────────────────

    /** Send a raw GET request through the gateway. */
    async get(path: string, options?: { headers?: Record<string, string>; params?: Record<string, string | number | boolean | undefined> }): Promise<Response> {
        return this._http.get(path, options);
    }

    /** Send a raw POST request through the gateway. */
    async post(path: string, body?: unknown, options?: { headers?: Record<string, string> }): Promise<Response> {
        return this._http.post(path, body, options);
    }

    /** Send a raw PUT request through the gateway. */
    async put(path: string, body?: unknown, options?: { headers?: Record<string, string> }): Promise<Response> {
        return this._http.put(path, body, options);
    }

    /** Send a raw PATCH request through the gateway. */
    async patch(path: string, body?: unknown, options?: { headers?: Record<string, string> }): Promise<Response> {
        return this._http.patch(path, body, options);
    }

    /** Send a raw DELETE request through the gateway. */
    async delete(path: string, options?: { headers?: Record<string, string> }): Promise<Response> {
        return this._http.delete(path, options);
    }

    // ── Resource accessors (lazy, cached) ──────────────────────────────────

    /** Virtual token management — create, list, update, revoke. */
    get tokens(): TokensResource {
        return (this._tokens ??= new TokensResource(this._http));
    }

    /** Encrypted credential management — create, list, delete, rotate. */
    get credentials(): CredentialsResource {
        return (this._credentials ??= new CredentialsResource(this._http));
    }

    /** Security policy management — create, list, update, delete. */
    get policies(): PoliciesResource {
        return (this._policies ??= new PoliciesResource(this._http));
    }

    /** HITL approval management — list, approve, reject. */
    get approvals(): ApprovalsResource {
        return (this._approvals ??= new ApprovalsResource(this._http));
    }

    /** Audit log querying — list, auto-paginate. */
    get audit(): AuditResource {
        return (this._audit ??= new AuditResource(this._http));
    }

    /** External service registration (Action Gateway). */
    get services(): ServicesResource {
        return (this._services ??= new ServicesResource(this._http));
    }

    /** API key management — create, list, revoke, whoami. */
    get apiKeys(): ApiKeysResource {
        return (this._apiKeys ??= new ApiKeysResource(this._http));
    }

    /** Webhook subscription management — create, list, delete, test. */
    get webhooks(): WebhooksResource {
        return (this._webhooks ??= new WebhooksResource(this._http));
    }

    /** Guardrail management — enable, disable, list presets per token. */
    get guardrails(): GuardrailsResource {
        return (this._guardrails ??= new GuardrailsResource(this._http));
    }

    /** Model alias management — map short names to real model identifiers. */
    get modelAliases(): ModelAliasesResource {
        return (this._modelAliases ??= new ModelAliasesResource(this._http));
    }

    /** Analytics — token summary, volume, latency, spend breakdown. */
    get analytics(): AnalyticsResource {
        return (this._analytics ??= new AnalyticsResource(this._http));
    }

    /** Config-as-Code — export/import policies and tokens as YAML or JSON. */
    get config(): ConfigResource {
        return (this._config ??= new ConfigResource(this._http));
    }

    /** OpenAI Batches API proxy. */
    get batches(): BatchesResource {
        return (this._batches ??= new BatchesResource(this._http));
    }

    /** OpenAI Fine-tuning API proxy. */
    get fineTuning(): FineTuningResource {
        return (this._fineTuning ??= new FineTuningResource(this._http));
    }

    /** Realtime WebSocket sessions through the gateway. */
    get realtime(): RealtimeResource {
        return (this._realtime ??= new RealtimeResource(this.gatewayUrl, this.apiKey));
    }

    /** Billing and usage information. */
    get billing(): BillingResource {
        return (this._billing ??= new BillingResource(this._http));
    }

    /** Project management — create, list, delete. */
    get projects(): ProjectsResource {
        return (this._projects ??= new ProjectsResource(this._http));
    }

    /** Experiment tracking (A/B testing) — create, monitor, and stop experiments. */
    get experiments(): ExperimentsResource {
        return (this._experiments ??= new ExperimentsResource(this._http));
    }

    /** Prompt management — CRUD, versioning, deployment, rendering. */
    get prompts(): PromptsResource {
        return (this._prompts ??= new PromptsResource(this._http));
    }
}
