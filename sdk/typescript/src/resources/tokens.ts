/**
 * Virtual token management — create, list, update, and revoke tokens.
 *
 * @example
 * ```ts
 * const token = await client.tokens.create({
 *   name: "my-agent",
 *   upstreamUrl: "https://api.openai.com",
 * });
 * console.log(token.tokenId); // tf_v1_...
 * ```
 *
 * @module
 */

import type { HttpClient } from "../http.js";
import type {
    Token,
    TokenCreateOptions,
    TokenCreateResponse,
    PaginationOptions,
    JsonObject,
} from "../types.js";

export class TokensResource {
    constructor(private readonly http: HttpClient) { }

    /**
     * Create a new virtual token.
     *
     * @example
     * ```ts
     * const token = await client.tokens.create({
     *   name: "research-agent",
     *   upstreamUrl: "https://api.openai.com",
     *   policyIds: ["pol_abc"],
     *   logLevel: "redacted",
     * });
     * ```
     */
    async create(options: TokenCreateOptions): Promise<TokenCreateResponse> {
        const body: Record<string, unknown> = {
            name: options.name,
            upstream_url: options.upstreamUrl,
        };
        if (options.credentialId) body["credential_id"] = options.credentialId;
        if (options.projectId) body["project_id"] = options.projectId;
        if (options.policyIds) body["policy_ids"] = options.policyIds;
        if (options.circuitBreaker !== undefined) body["circuit_breaker"] = options.circuitBreaker;
        if (options.fallbackUrl) body["fallback_url"] = options.fallbackUrl;
        if (options.upstreams) {
            body["upstreams"] = options.upstreams.map((u) => ({
                url: u.url,
                weight: u.weight ?? 100,
                priority: u.priority ?? 1,
                ...(u.credentialId ? { credential_id: u.credentialId } : {}),
            }));
        }
        if (options.logLevel) body["log_level_name"] = options.logLevel;
        if (options.expiresAt) body["expires_at"] = options.expiresAt;

        const res = await this.http.post("/api/v1/tokens", body);
        const raw = (await res.json()) as Record<string, unknown>;
        return {
            tokenId: (raw.token_id ?? raw.tokenId) as string | undefined,
            id: raw.id as string | undefined,
            name: raw.name as string | undefined,
            upstreamUrl: (raw.upstream_url ?? raw.upstreamUrl) as string | undefined,
            credentialId: (raw.credential_id ?? raw.credentialId) as string | undefined,
            projectId: (raw.project_id ?? raw.projectId) as string | undefined,
        };
    }

    /**
     * List all tokens with pagination.
     *
     * @example
     * ```ts
     * const tokens = await client.tokens.list({ limit: 10 });
     * ```
     */
    async list(options: PaginationOptions = {}): Promise<Token[]> {
        const res = await this.http.get("/api/v1/tokens", {
            params: {
                limit: options.limit,
                offset: options.offset,
            },
        });
        return (await res.json()) as Token[];
    }

    /**
     * Get a single token by ID.
     *
     * @example
     * ```ts
     * const token = await client.tokens.get("tok_abc123");
     * ```
     */
    async get(tokenId: string): Promise<Token> {
        const res = await this.http.get(`/api/v1/tokens/${tokenId}`);
        return (await res.json()) as Token;
    }

    /**
     * Update a token's properties.
     *
     * @example
     * ```ts
     * await client.tokens.update("tok_abc", { name: "renamed-agent" });
     * ```
     */
    async update(tokenId: string, updates: Partial<TokenCreateOptions>): Promise<JsonObject> {
        const body: Record<string, unknown> = {};
        if (updates.name) body["name"] = updates.name;
        if (updates.upstreamUrl) body["upstream_url"] = updates.upstreamUrl;
        if (updates.policyIds) body["policy_ids"] = updates.policyIds;
        if (updates.circuitBreaker !== undefined) body["circuit_breaker"] = updates.circuitBreaker;
        const res = await this.http.put(`/api/v1/tokens/${tokenId}`, body);
        return (await res.json()) as JsonObject;
    }

    /**
     * Revoke (deactivate) a token.
     *
     * @example
     * ```ts
     * await client.tokens.revoke("tok_abc123");
     * ```
     */
    async revoke(tokenId: string): Promise<JsonObject> {
        const res = await this.http.delete(`/api/v1/tokens/${tokenId}`);
        return (await res.json()) as JsonObject;
    }

    /**
     * Enable a guardrail on a token.
     *
     * @example
     * ```ts
     * await client.tokens.enableGuardrail("tok_abc", "prompt_injection");
     * ```
     */
    async enableGuardrail(tokenId: string, guardrail: string): Promise<JsonObject> {
        const res = await this.http.post(`/api/v1/tokens/${tokenId}/guardrails`, { guardrail });
        return (await res.json()) as JsonObject;
    }

    /**
     * Disable a guardrail on a token.
     *
     * @example
     * ```ts
     * await client.tokens.disableGuardrail("tok_abc", "prompt_injection");
     * ```
     */
    async disableGuardrail(tokenId: string, guardrail: string): Promise<JsonObject> {
        const res = await this.http.delete(`/api/v1/tokens/${tokenId}/guardrails/${guardrail}`);
        return (await res.json()) as JsonObject;
    }
}
