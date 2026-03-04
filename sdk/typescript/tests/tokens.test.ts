import { describe, it, expect, vi, beforeEach } from "vitest";
import { TokensResource } from "../src/resources/tokens.js";
import { HttpClient } from "../src/http.js";

// ── Helpers ──────────────────────────────────────────────────────────────

function okResponse(body: unknown = {}): Response {
    return new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });
}

function createMockHttp(): HttpClient & { post: ReturnType<typeof vi.fn>; get: ReturnType<typeof vi.fn>; put: ReturnType<typeof vi.fn>; delete: ReturnType<typeof vi.fn> } {
    return {
        post: vi.fn().mockResolvedValue(okResponse({})),
        get: vi.fn().mockResolvedValue(okResponse([])),
        put: vi.fn().mockResolvedValue(okResponse({})),
        delete: vi.fn().mockResolvedValue(okResponse({})),
        patch: vi.fn(),
        raw: vi.fn(),
        request: vi.fn(),
        baseUrl: "https://gw.test",
    } as unknown as HttpClient & { post: ReturnType<typeof vi.fn>; get: ReturnType<typeof vi.fn>; put: ReturnType<typeof vi.fn>; delete: ReturnType<typeof vi.fn> };
}

// ── Tests ────────────────────────────────────────────────────────────────

describe("TokensResource", () => {
    let tokens: TokensResource;
    let http: ReturnType<typeof createMockHttp>;

    beforeEach(() => {
        http = createMockHttp();
        tokens = new TokensResource(http);
    });

    it("create sends correct body with camelCase→snake_case mapping", async () => {
        http.post.mockResolvedValueOnce(okResponse({ token_id: "tf_v1_test", id: "uuid" }));
        const result = await tokens.create({
            name: "my-agent",
            upstreamUrl: "https://api.openai.com",
            credentialId: "cred_123",
            projectId: "proj_1",
            policyIds: ["pol_a"],
            logLevel: "redacted",
        });
        expect(http.post).toHaveBeenCalledWith("/api/v1/tokens", expect.objectContaining({
            name: "my-agent",
            upstream_url: "https://api.openai.com",
            credential_id: "cred_123",
            project_id: "proj_1",
            policy_ids: ["pol_a"],
            log_level_name: "redacted",
        }));
        expect(result.tokenId).toBe("tf_v1_test");
    });

    it("create handles upstream load balancing config", async () => {
        http.post.mockResolvedValueOnce(okResponse({ id: "uuid" }));
        await tokens.create({
            name: "lb-agent",
            upstreamUrl: "https://api.openai.com",
            upstreams: [
                { url: "https://api.openai.com", weight: 80 },
                { url: "https://backup.openai.com", weight: 20, priority: 2 },
            ],
        });
        const body = http.post.mock.calls[0]?.[1] as Record<string, unknown>;
        const upstreams = body["upstreams"] as Array<Record<string, unknown>>;
        expect(upstreams).toHaveLength(2);
        expect(upstreams[0]?.["url"]).toBe("https://api.openai.com");
        expect(upstreams[0]?.["weight"]).toBe(80);
        expect(upstreams[1]?.["priority"]).toBe(2);
    });

    it("list calls GET /api/v1/tokens with pagination", async () => {
        http.get.mockResolvedValueOnce(okResponse([{ id: "1", name: "tok" }]));
        const result = await tokens.list({ limit: 10, offset: 5 });
        expect(http.get).toHaveBeenCalledWith("/api/v1/tokens", { params: { limit: 10, offset: 5 } });
        expect(result).toHaveLength(1);
    });

    it("get calls GET /api/v1/tokens/:id", async () => {
        http.get.mockResolvedValueOnce(okResponse({ id: "tok_1", name: "my-token" }));
        const result = await tokens.get("tok_1");
        expect(http.get).toHaveBeenCalledWith("/api/v1/tokens/tok_1");
        expect(result.name).toBe("my-token");
    });

    it("update calls PUT with mapped body", async () => {
        http.put.mockResolvedValueOnce(okResponse({ updated: true }));
        await tokens.update("tok_1", { name: "renamed" });
        expect(http.put).toHaveBeenCalledWith("/api/v1/tokens/tok_1", expect.objectContaining({ name: "renamed" }));
    });

    it("revoke calls DELETE", async () => {
        http.delete.mockResolvedValueOnce(okResponse({ revoked: true }));
        const result = await tokens.revoke("tok_1");
        expect(http.delete).toHaveBeenCalledWith("/api/v1/tokens/tok_1");
        expect(result["revoked"]).toBe(true);
    });

    it("enableGuardrail calls POST with guardrail name", async () => {
        http.post.mockResolvedValueOnce(okResponse({ enabled: true }));
        await tokens.enableGuardrail("tok_1", "prompt_injection");
        expect(http.post).toHaveBeenCalledWith("/api/v1/tokens/tok_1/guardrails", { guardrail: "prompt_injection" });
    });

    it("disableGuardrail calls DELETE with guardrail name", async () => {
        http.delete.mockResolvedValueOnce(okResponse({ disabled: true }));
        await tokens.disableGuardrail("tok_1", "pii_redaction");
        expect(http.delete).toHaveBeenCalledWith("/api/v1/tokens/tok_1/guardrails/pii_redaction");
    });
});
