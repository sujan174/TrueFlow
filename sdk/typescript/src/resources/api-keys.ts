import type { HttpClient } from "../http.js";
import type { JsonObject } from "../types.js";

export class ApiKeysResource {
    constructor(private readonly http: HttpClient) { }

    /** Create a new API key. */
    async create(options: { name: string; role?: string; scopes?: string[]; userId?: string }): Promise<JsonObject> {
        const body: Record<string, unknown> = { name: options.name };
        if (options.role) body["role"] = options.role;
        if (options.scopes) body["scopes"] = options.scopes;
        if (options.userId) body["user_id"] = options.userId;
        const res = await this.http.post("/api/v1/auth/keys", body);
        return (await res.json()) as JsonObject;
    }

    /** List API keys. */
    async list(options: { limit?: number; offset?: number } = {}): Promise<JsonObject[]> {
        const res = await this.http.get("/api/v1/auth/keys", { params: { limit: options.limit, offset: options.offset } });
        return (await res.json()) as JsonObject[];
    }

    /** Revoke an API key. */
    async revoke(keyId: string): Promise<JsonObject> {
        const res = await this.http.delete(`/api/v1/auth/keys/${keyId}`);
        return (await res.json()) as JsonObject;
    }

    /** Update an API key's name and/or scopes. */
    async update(keyId: string, options: { name?: string; scopes?: string[] }): Promise<JsonObject> {
        const body: Record<string, unknown> = {};
        if (options.name !== undefined) body["name"] = options.name;
        if (options.scopes !== undefined) body["scopes"] = options.scopes;
        const res = await this.http.put(`/api/v1/auth/keys/${keyId}`, body);
        return (await res.json()) as JsonObject;
    }

    /** Get information about the current authentication context. */
    async whoami(): Promise<JsonObject> {
        const res = await this.http.get("/api/v1/auth/whoami");
        return (await res.json()) as JsonObject;
    }
}
