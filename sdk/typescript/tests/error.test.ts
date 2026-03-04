import { describe, it, expect } from "vitest";
import {
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
} from "../src/error.js";

// ── Helpers ──────────────────────────────────────────────────────────────

function mockResponse(
    status: number,
    body: unknown,
    headers: Record<string, string> = {},
): Response {
    const text = typeof body === "string" ? body : JSON.stringify(body);
    return new Response(text, {
        status,
        statusText: status < 400 ? "OK" : "Error",
        headers: new Headers(headers),
    });
}

// ── Tests ────────────────────────────────────────────────────────────────

describe("Error hierarchy", () => {
    it("base TrueFlowError has all properties", () => {
        const e = new TrueFlowError("oops", { statusCode: 500, errorType: "t", code: "c", requestId: "r" });
        expect(e.message).toBe("oops");
        expect(e.statusCode).toBe(500);
        expect(e.errorType).toBe("t");
        expect(e.code).toBe("c");
        expect(e.requestId).toBe("r");
        expect(e.name).toBe("TrueFlowError");
        expect(e).toBeInstanceOf(Error);
    });

    it("subclasses are instanceof TrueFlowError", () => {
        expect(new AuthenticationError("x")).toBeInstanceOf(TrueFlowError);
        expect(new AccessDeniedError("x")).toBeInstanceOf(TrueFlowError);
        expect(new PolicyDeniedError("x")).toBeInstanceOf(AccessDeniedError);
        expect(new ContentBlockedError("x")).toBeInstanceOf(AccessDeniedError);
        expect(new NotFoundError("x")).toBeInstanceOf(TrueFlowError);
        expect(new RateLimitError("x")).toBeInstanceOf(TrueFlowError);
        expect(new ValidationError("x")).toBeInstanceOf(TrueFlowError);
        expect(new PayloadTooLargeError("x")).toBeInstanceOf(TrueFlowError);
        expect(new SpendCapError("x")).toBeInstanceOf(TrueFlowError);
        expect(new GatewayError("x")).toBeInstanceOf(TrueFlowError);
    });

    it("RateLimitError has retryAfter", () => {
        const e = new RateLimitError("wait", { retryAfter: 30 });
        expect(e.retryAfter).toBe(30);
        expect(e.name).toBe("RateLimitError");
    });

    it("ContentBlockedError has matchedPatterns and confidence", () => {
        const e = new ContentBlockedError("nope", { matchedPatterns: ["DAN"], confidence: 0.95 });
        expect(e.matchedPatterns).toEqual(["DAN"]);
        expect(e.confidence).toBe(0.95);
        expect(e.name).toBe("ContentBlockedError");
    });

    it("defaults optional fields to empty strings", () => {
        const e = new TrueFlowError("oops");
        expect(e.statusCode).toBeUndefined();
        expect(e.errorType).toBe("");
        expect(e.code).toBe("");
        expect(e.requestId).toBe("");
    });
});

describe("raiseForStatus", () => {
    it("does nothing for 200", async () => {
        const res = mockResponse(200, { ok: true });
        await expect(raiseForStatus(res)).resolves.toBeUndefined();
    });

    it("throws AuthenticationError for 401", async () => {
        const res = mockResponse(401, { error: { message: "bad key", type: "auth_error", code: "invalid_key" } });
        await expect(raiseForStatus(res)).rejects.toThrow(AuthenticationError);
    });

    it("throws SpendCapError for 402", async () => {
        const res = mockResponse(402, { error: { message: "cap reached" } });
        await expect(raiseForStatus(res)).rejects.toThrow(SpendCapError);
    });

    it("throws PolicyDeniedError for 403 with code=policy_denied", async () => {
        const res = mockResponse(403, { error: { message: "blocked", code: "policy_denied" } });
        await expect(raiseForStatus(res)).rejects.toThrow(PolicyDeniedError);
    });

    it("throws ContentBlockedError for 403 with code=content_blocked", async () => {
        const res = mockResponse(403, {
            error: { message: "harmful", code: "content_blocked", details: { matched_patterns: ["DAN"], confidence: 0.9 } },
        });
        try {
            await raiseForStatus(res);
            expect.fail("should throw");
        } catch (e) {
            expect(e).toBeInstanceOf(ContentBlockedError);
            const cbe = e as ContentBlockedError;
            expect(cbe.matchedPatterns).toEqual(["DAN"]);
            expect(cbe.confidence).toBe(0.9);
        }
    });

    it("throws AccessDeniedError for generic 403", async () => {
        const res = mockResponse(403, { error: { message: "nope" } });
        await expect(raiseForStatus(res)).rejects.toThrow(AccessDeniedError);
    });

    it("throws NotFoundError for 404", async () => {
        const res = mockResponse(404, { error: { message: "not found" } });
        await expect(raiseForStatus(res)).rejects.toThrow(NotFoundError);
    });

    it("throws PayloadTooLargeError for 413", async () => {
        const res = mockResponse(413, { error: { message: "too big" } });
        await expect(raiseForStatus(res)).rejects.toThrow(PayloadTooLargeError);
    });

    it("throws ValidationError for 422", async () => {
        const res = mockResponse(422, { error: { message: "invalid" } });
        await expect(raiseForStatus(res)).rejects.toThrow(ValidationError);
    });

    it("throws RateLimitError for 429 with Retry-After", async () => {
        const res = mockResponse(429, { error: { message: "slow down" } }, { "retry-after": "30" });
        try {
            await raiseForStatus(res);
            expect.fail("should throw");
        } catch (e) {
            expect(e).toBeInstanceOf(RateLimitError);
            expect((e as RateLimitError).retryAfter).toBe(30);
        }
    });

    it("throws GatewayError for 500", async () => {
        const res = mockResponse(500, { error: { message: "internal" } });
        await expect(raiseForStatus(res)).rejects.toThrow(GatewayError);
    });

    it("throws TrueFlowError for other 4xx codes", async () => {
        const res = mockResponse(418, "I'm a teapot");
        await expect(raiseForStatus(res)).rejects.toThrow(TrueFlowError);
    });

    it("uses x-request-id header when body has no request_id", async () => {
        const res = mockResponse(500, { error: { message: "boom" } }, { "x-request-id": "req_123" });
        try {
            await raiseForStatus(res);
        } catch (e) {
            expect((e as GatewayError).requestId).toBe("req_123");
        }
    });

    it("handles non-JSON error bodies gracefully", async () => {
        const res = mockResponse(500, "plain text error");
        await expect(raiseForStatus(res)).rejects.toThrow(GatewayError);
    });
});
