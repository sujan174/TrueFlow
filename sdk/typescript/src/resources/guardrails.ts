import type { HttpClient } from "../http.js";
import type { JsonObject } from "../types.js";
import type { GuardrailPresetInfo } from "../guardrail-presets.js";

export class GuardrailsResource {
    constructor(private readonly http: HttpClient) { }

    /** List available guardrail presets from the gateway. */
    async listPresets(): Promise<GuardrailPresetInfo[]> {
        const res = await this.http.get("/api/v1/guardrails/presets");
        const body = (await res.json()) as Record<string, unknown>;
        return (body["presets"] as GuardrailPresetInfo[]) ?? [];
    }

    /** Check current guardrails state for a token. */
    async status(tokenId: string): Promise<JsonObject> {
        const res = await this.http.get("/api/v1/guardrails/status", { params: { token_id: tokenId } });
        return (await res.json()) as JsonObject;
    }

    /** Attach guardrail presets to a token. */
    async enable(tokenId: string, presets: string[], options: { topicAllowlist?: string[]; topicDenylist?: string[] } = {}): Promise<JsonObject> {
        const body: Record<string, unknown> = { token_id: tokenId, presets, source: "sdk" };
        if (options.topicAllowlist) body["topic_allowlist"] = options.topicAllowlist;
        if (options.topicDenylist) body["topic_denylist"] = options.topicDenylist;
        const res = await this.http.post("/api/v1/guardrails/enable", body);
        return (await res.json()) as JsonObject;
    }

    /** Remove guardrail policies from a token. */
    async disable(tokenId: string, options: { policyNamePrefix?: string } = {}): Promise<JsonObject> {
        const body: Record<string, unknown> = { token_id: tokenId };
        if (options.policyNamePrefix) body["policy_name_prefix"] = options.policyNamePrefix;
        const res = await this.http.raw("/api/v1/guardrails/disable", { method: "DELETE", body: JSON.stringify(body), headers: { "Content-Type": "application/json" } });
        return (await res.json()) as JsonObject;
    }
}
