import type { HttpClient } from "../http.js";
import type { JsonObject } from "../types.js";

/** @internal */
interface CacheEntry {
    data: JsonObject;
    timestamp: number;
}

/** Simple hash for cache keys (FNV-1a inspired). */
function hashCode(s: string): string {
    let h = 0x811c9dc5;
    for (let i = 0; i < s.length; i++) {
        h ^= s.charCodeAt(i);
        h = (h * 0x01000193) >>> 0;
    }
    return h.toString(36);
}

function cacheKey(
    slug: string,
    label: string | undefined,
    version: number | undefined,
    variables: Record<string, unknown> | undefined,
): string {
    const varHash = variables ? hashCode(JSON.stringify(variables, Object.keys(variables).sort())) : "";
    return `${slug}:${label ?? ""}:${version ?? ""}:${varHash}`;
}

/**
 * Prompt management — CRUD, versioning, label-based deployment, rendering.
 *
 * Includes client-side TTL caching for `render()` (default 60s).
 *
 * @example
 * ```ts
 * const admin = TrueFlowClient.admin({ adminKey: "..." });
 *
 * // Create a prompt
 * const prompt = await admin.prompts.create({ name: "Customer Support Agent" });
 *
 * // Publish a version
 * await admin.prompts.createVersion(prompt.id, {
 *   model: "gpt-4o",
 *   messages: [
 *     { role: "system", content: "You help {{user_name}} with {{topic}}." },
 *     { role: "user", content: "{{question}}" },
 *   ],
 * });
 *
 * // Deploy v1 to production
 * await admin.prompts.deploy(prompt.id, { version: 1, label: "production" });
 *
 * // Render for use with OpenAI (cached for 60s by default)
 * const payload = await admin.prompts.render("customer-support-agent", {
 *   variables: { user_name: "Alice", topic: "billing", question: "Where is my invoice?" },
 *   label: "production",
 * });
 * ```
 */
export class PromptsResource {
    private readonly cache = new Map<string, CacheEntry>();
    private readonly cacheTtlMs: number;

    constructor(
        private readonly http: HttpClient,
        options?: { cacheTtl?: number },
    ) {
        this.cacheTtlMs = (options?.cacheTtl ?? 60) * 1000;
    }

    // ── Prompt CRUD ────────────────────────────────────────────

    /** List all prompts, optionally filtered by folder. */
    async list(options: { folder?: string } = {}): Promise<JsonObject[]> {
        const res = await this.http.get("/api/v1/prompts", {
            params: { folder: options.folder },
        });
        return (await res.json()) as JsonObject[];
    }

    /** Create a new prompt. */
    async create(options: {
        name: string;
        slug?: string;
        description?: string;
        folder?: string;
        tags?: Record<string, unknown>;
    }): Promise<JsonObject> {
        const body: Record<string, unknown> = { name: options.name };
        if (options.slug !== undefined) body["slug"] = options.slug;
        if (options.description !== undefined) body["description"] = options.description;
        if (options.folder !== undefined) body["folder"] = options.folder;
        if (options.tags !== undefined) body["tags"] = options.tags;
        const res = await this.http.post("/api/v1/prompts", body);
        return (await res.json()) as JsonObject;
    }

    /** Get a prompt and its versions. */
    async get(promptId: string): Promise<JsonObject> {
        const res = await this.http.get(`/api/v1/prompts/${promptId}`);
        return (await res.json()) as JsonObject;
    }

    /** Update prompt metadata. */
    async update(
        promptId: string,
        options: {
            name: string;
            description?: string;
            folder?: string;
            tags?: Record<string, unknown>;
        },
    ): Promise<JsonObject> {
        const body: Record<string, unknown> = { name: options.name };
        if (options.description !== undefined) body["description"] = options.description;
        if (options.folder !== undefined) body["folder"] = options.folder;
        if (options.tags !== undefined) body["tags"] = options.tags;
        const res = await this.http.put(`/api/v1/prompts/${promptId}`, body);
        return (await res.json()) as JsonObject;
    }

    /** Soft-delete a prompt. */
    async delete(promptId: string): Promise<JsonObject> {
        const res = await this.http.delete(`/api/v1/prompts/${promptId}`);
        return (await res.json()) as JsonObject;
    }

    // ── Versions ───────────────────────────────────────────────

    /** List all versions for a prompt (newest first). */
    async listVersions(promptId: string): Promise<JsonObject[]> {
        const res = await this.http.get(`/api/v1/prompts/${promptId}/versions`);
        return (await res.json()) as JsonObject[];
    }

    /** Publish a new version of a prompt. */
    async createVersion(
        promptId: string,
        options: {
            model: string;
            messages: unknown[];
            temperature?: number;
            maxTokens?: number;
            topP?: number;
            tools?: unknown[];
            commitMessage?: string;
        },
    ): Promise<JsonObject> {
        const body: Record<string, unknown> = {
            model: options.model,
            messages: options.messages,
        };
        if (options.temperature !== undefined) body["temperature"] = options.temperature;
        if (options.maxTokens !== undefined) body["max_tokens"] = options.maxTokens;
        if (options.topP !== undefined) body["top_p"] = options.topP;
        if (options.tools !== undefined) body["tools"] = options.tools;
        if (options.commitMessage !== undefined) body["commit_message"] = options.commitMessage;
        const res = await this.http.post(`/api/v1/prompts/${promptId}/versions`, body);
        return (await res.json()) as JsonObject;
    }

    /** Get a specific version of a prompt. */
    async getVersion(promptId: string, version: number): Promise<JsonObject> {
        const res = await this.http.get(`/api/v1/prompts/${promptId}/versions/${version}`);
        return (await res.json()) as JsonObject;
    }

    // ── Deployment ─────────────────────────────────────────────

    /**
     * Deploy a version to a label (e.g. "production", "staging").
     * Atomically promotes a version — the previous holder of the label is demoted.
     */
    async deploy(
        promptId: string,
        options: { version: number; label: string },
    ): Promise<JsonObject> {
        const res = await this.http.post(`/api/v1/prompts/${promptId}/deploy`, {
            version: options.version,
            label: options.label,
        });
        return (await res.json()) as JsonObject;
    }

    // ── Rendering (with client-side TTL cache) ─────────────────

    /**
     * Render a prompt with variable substitution.
     *
     * Returns an OpenAI-compatible payload (`model`, `messages`, etc.)
     * ready to spread into `openai.chat.completions.create()`.
     *
     * Results are cached client-side for `cacheTtl` seconds (default 60).
     * Resolution order: exact `version` → matching `label` → latest.
     */
    async render(
        slug: string,
        options: {
            variables?: Record<string, unknown>;
            label?: string;
            version?: number;
        } = {},
    ): Promise<JsonObject> {
        const key = cacheKey(slug, options.label, options.version, options.variables);
        const cached = this.cache.get(key);
        if (cached && Date.now() - cached.timestamp < this.cacheTtlMs) {
            return cached.data;
        }

        const body: Record<string, unknown> = {};
        if (options.variables) body["variables"] = options.variables;
        if (options.label !== undefined) body["label"] = options.label;
        if (options.version !== undefined) body["version"] = options.version;
        const res = await this.http.post(`/api/v1/prompts/by-slug/${slug}/render`, body);
        const data = (await res.json()) as JsonObject;

        this.cache.set(key, { data, timestamp: Date.now() });
        return data;
    }

    // ── Cache Management ──────────────────────────────────────

    /** Clear all cached rendered prompts. */
    clearCache(): void {
        this.cache.clear();
    }

    /** Invalidate all cache entries for a specific prompt slug. */
    invalidate(slug: string): void {
        for (const key of this.cache.keys()) {
            if (key.startsWith(`${slug}:`)) {
                this.cache.delete(key);
            }
        }
    }

    // ── Folders ────────────────────────────────────────────────

    /** List all unique folder paths across prompts. */
    async listFolders(): Promise<string[]> {
        const res = await this.http.get("/api/v1/prompts/folders");
        return (await res.json()) as string[];
    }
}

