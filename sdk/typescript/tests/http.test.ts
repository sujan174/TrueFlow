import { describe, it, expect, vi, beforeEach } from "vitest";
import { HttpClient } from "../src/http.js";
import { TrueFlowError, GatewayError, RateLimitError } from "../src/error.js";

// ── Helpers ──────────────────────────────────────────────────────────────

function okResponse(body: unknown = {}, status = 200): Response {
    return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

function errorResponse(status: number, body: unknown = {}): Response {
    return new Response(JSON.stringify({ error: body }), { status, headers: { "Content-Type": "application/json" } });
}

// ── Tests ────────────────────────────────────────────────────────────────

describe("HttpClient", () => {
    let client: HttpClient;
    let fetchSpy: ReturnType<typeof vi.fn>;

    beforeEach(() => {
        client = new HttpClient({
            baseUrl: "https://gateway.test",
            headers: { Authorization: "Bearer tok_test" },
            timeoutMs: 5000,
            maxRetries: 2,
            initialBackoffMs: 10, // fast for tests
        });
        fetchSpy = vi.fn();
        vi.stubGlobal("fetch", fetchSpy);
    });

    it("makes a successful GET", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse({ ok: true }));
        const res = await client.get("/api/test");
        const data: Record<string, unknown> = await res.json();
        expect(data["ok"]).toBe(true);
        expect(fetchSpy).toHaveBeenCalledOnce();
        const callUrl = fetchSpy.mock.calls[0]?.[0] as string;
        expect(callUrl).toBe("https://gateway.test/api/test");
    });

    it("sends JSON body on POST", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse({ id: "123" }));
        await client.post("/api/create", { name: "test" });
        const callOpts = fetchSpy.mock.calls[0]?.[1] as RequestInit;
        expect(callOpts.method).toBe("POST");
        expect(callOpts.body).toBe('{"name":"test"}');
    });

    it("sends default headers", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse());
        await client.get("/api/test");
        const callOpts = fetchSpy.mock.calls[0]?.[1] as RequestInit;
        const headers = callOpts.headers as Record<string, string>;
        expect(headers["Authorization"]).toBe("Bearer tok_test");
        expect(headers["X-TrueFlow-SDK-Version"]).toBeDefined();
        expect(headers["Content-Type"]).toBe("application/json");
    });

    it("includes query params in URL", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse());
        await client.get("/api/test", { params: { limit: 10, offset: 0, unused: undefined } });
        const callUrl = fetchSpy.mock.calls[0]?.[0] as string;
        expect(callUrl).toContain("limit=10");
        expect(callUrl).toContain("offset=0");
        expect(callUrl).not.toContain("unused");
    });

    it("throws typed error for 4xx responses", async () => {
        fetchSpy.mockResolvedValueOnce(errorResponse(404, { message: "not found" }));
        await expect(client.get("/api/missing")).rejects.toThrow(TrueFlowError);
    });

    it("retries on 5xx and eventually throws", async () => {
        fetchSpy.mockResolvedValue(errorResponse(500, { message: "boom" }));
        await expect(client.get("/api/flaky")).rejects.toThrow(GatewayError);
        // Should have retried: initial + 2 retries = 3 calls
        expect(fetchSpy).toHaveBeenCalledTimes(3);
    });

    it("retries on network errors", async () => {
        fetchSpy
            .mockRejectedValueOnce(new TypeError("fetch failed"))
            .mockRejectedValueOnce(new TypeError("fetch failed"))
            .mockResolvedValueOnce(okResponse({ ok: true }));
        const res = await client.get("/api/flaky");
        const data: Record<string, unknown> = await res.json();
        expect(data["ok"]).toBe(true);
        expect(fetchSpy).toHaveBeenCalledTimes(3);
    });

    it("retries on RateLimitError and respects retry-after", async () => {
        fetchSpy
            .mockResolvedValueOnce(new Response(JSON.stringify({ error: { message: "slow" } }), { status: 429, headers: { "retry-after": "0.01" } }))
            .mockResolvedValueOnce(okResponse({ ok: true }));
        const res = await client.get("/api/rated");
        const data: Record<string, unknown> = await res.json();
        expect(data["ok"]).toBe(true);
        expect(fetchSpy).toHaveBeenCalledTimes(2);
    });

    it("DELETE returns response", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse({ deleted: true }));
        const res = await client.delete("/api/tokens/tok_123");
        const data: Record<string, unknown> = await res.json();
        expect(data["deleted"]).toBe(true);
        const callOpts = fetchSpy.mock.calls[0]?.[1] as RequestInit;
        expect(callOpts.method).toBe("DELETE");
    });

    it("PUT sends body", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse({ updated: true }));
        await client.put("/api/tokens/tok_123", { name: "new" });
        const callOpts = fetchSpy.mock.calls[0]?.[1] as RequestInit;
        expect(callOpts.method).toBe("PUT");
        expect(callOpts.body).toBe('{"name":"new"}');
    });

    it("PATCH sends body", async () => {
        fetchSpy.mockResolvedValueOnce(okResponse({ patched: true }));
        await client.patch("/api/tokens/tok_123", { name: "patched" });
        const callOpts = fetchSpy.mock.calls[0]?.[1] as RequestInit;
        expect(callOpts.method).toBe("PATCH");
    });
});
