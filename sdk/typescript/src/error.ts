/**
 * Typed error hierarchy for the TrueFlow SDK.
 *
 * Every gateway failure mode maps to a specific error class with typed
 * properties. Users can catch specific classes and act on structured data
 * like `requestId`, `code`, or `retryAfter` — no string parsing needed.
 *
 * @example
 * ```ts
 * import { TrueFlowError, RateLimitError } from "@trueflow/sdk";
 *
 * try {
 *   await client.tokens.list();
 * } catch (e) {
 *   if (e instanceof RateLimitError) {
 *     console.log(`Retry after ${e.retryAfter}s`);
 *   }
 * }
 * ```
 *
 * @module
 */

// ────────────────────────────────────────────────────────────────────────────
// Base
// ────────────────────────────────────────────────────────────────────────────

/** Base error for all TrueFlow SDK errors. */
export class TrueFlowError extends Error {
    /** HTTP status code from the gateway (undefined for client-side errors). */
    readonly statusCode: number | undefined;
    /** Machine-readable error type from the gateway body (e.g. `"rate_limit_error"`). */
    readonly errorType: string;
    /** Specific error code from the gateway body (e.g. `"rate_limit_exceeded"`). */
    readonly code: string;
    /** Request ID for support correlation. */
    readonly requestId: string;

    constructor(
        message: string,
        options: {
            statusCode?: number;
            errorType?: string;
            code?: string;
            requestId?: string;
        } = {},
    ) {
        super(message);
        this.name = "TrueFlowError";
        this.statusCode = options.statusCode;
        this.errorType = options.errorType ?? "";
        this.code = options.code ?? "";
        this.requestId = options.requestId ?? "";
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Auth
// ────────────────────────────────────────────────────────────────────────────

/** Invalid or missing API key / admin key (HTTP 401). */
export class AuthenticationError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "AuthenticationError";
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Permission
// ────────────────────────────────────────────────────────────────────────────

/** Valid credentials but insufficient permissions (HTTP 403). */
export class AccessDeniedError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "AccessDeniedError";
    }
}

/** Request blocked by a gateway policy (HTTP 403, code=policy_denied). */
export class PolicyDeniedError extends AccessDeniedError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "PolicyDeniedError";
    }
}

/** Request blocked by a content filter — jailbreak, harmful content, etc. (HTTP 403, code=content_blocked). */
export class ContentBlockedError extends AccessDeniedError {
    /** Regex/NLP patterns that matched. */
    readonly matchedPatterns: string[];
    /** Detection confidence (0–1). */
    readonly confidence: number | undefined;

    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] & {
            matchedPatterns?: string[];
            confidence?: number;
        } = {},
    ) {
        super(message, options);
        this.name = "ContentBlockedError";
        this.matchedPatterns = options.matchedPatterns ?? [];
        this.confidence = options.confidence;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Client errors
// ────────────────────────────────────────────────────────────────────────────

/** Requested resource does not exist (HTTP 404). */
export class NotFoundError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "NotFoundError";
    }
}

/**
 * Rate limit exceeded (HTTP 429).
 *
 * Check `retryAfter` for the number of seconds to wait before retrying.
 */
export class RateLimitError extends TrueFlowError {
    /** Seconds to wait before retrying, from the `Retry-After` header. */
    readonly retryAfter: number | undefined;

    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] & {
            retryAfter?: number;
        } = {},
    ) {
        super(message, options);
        this.name = "RateLimitError";
        this.retryAfter = options.retryAfter;
    }
}

/** Request payload failed server-side validation (HTTP 422). */
export class ValidationError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "ValidationError";
    }
}

/** Request body exceeds the gateway's 25 MB size limit (HTTP 413). */
export class PayloadTooLargeError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "PayloadTooLargeError";
    }
}

/** Token spend cap reached (HTTP 402). */
export class SpendCapError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "SpendCapError";
    }
}

/** Gateway returned a 5xx error. */
export class GatewayError extends TrueFlowError {
    constructor(
        message: string,
        options: ConstructorParameters<typeof TrueFlowError>[1] = {},
    ) {
        super(message, options);
        this.name = "GatewayError";
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Error body parser
// ────────────────────────────────────────────────────────────────────────────

interface ParsedError {
    message: string;
    errorType: string;
    code: string;
    requestId: string;
    details: Record<string, unknown> | undefined;
}

function parseErrorBody(body: string): ParsedError {
    try {
        const json = JSON.parse(body) as Record<string, unknown>;
        const error = json["error"];
        if (typeof error === "object" && error !== null) {
            const e = error as Record<string, unknown>;
            return {
                message: (e["message"] as string) ?? body,
                errorType: (e["type"] as string) ?? "",
                code: (e["code"] as string) ?? "",
                requestId: (e["request_id"] as string) ?? "",
                details: e["details"] as Record<string, unknown> | undefined,
            };
        }
        if (typeof error === "string") {
            return { message: error, errorType: "", code: "", requestId: "", details: undefined };
        }
        return { message: body, errorType: "", code: "", requestId: "", details: undefined };
    } catch {
        return { message: body, errorType: "", code: "", requestId: "", details: undefined };
    }
}

// ────────────────────────────────────────────────────────────────────────────
// raiseForStatus — maps HTTP status to the correct error class
// ────────────────────────────────────────────────────────────────────────────

/**
 * Inspect a `Response` and throw a typed `TrueFlowError` subclass if it
 * indicates failure. Does nothing if the response is 2xx.
 *
 * @example
 * ```ts
 * const res = await fetch(url);
 * await raiseForStatus(res);
 * ```
 */
export async function raiseForStatus(response: Response): Promise<void> {
    if (response.ok) return;

    const status = response.status;
    const text = await response.text();
    const { message, errorType, code, requestId: bodyReqId, details } = parseErrorBody(text);
    const requestId = bodyReqId || response.headers.get("x-request-id") || "";

    const opts = { statusCode: status, errorType, code, requestId };

    if (status === 401) {
        throw new AuthenticationError(`Authentication failed: ${message}`, opts);
    }
    if (status === 402) {
        throw new SpendCapError(`Spend cap reached: ${message}`, opts);
    }
    if (status === 403) {
        if (code === "policy_denied") {
            throw new PolicyDeniedError(`Policy denied: ${message}`, opts);
        }
        if (code === "content_blocked") {
            const matched = (details?.["matched_patterns"] as string[]) ?? [];
            const confidence = details?.["confidence"] as number | undefined;
            throw new ContentBlockedError(`Content blocked: ${message}`, {
                ...opts,
                matchedPatterns: matched,
                confidence,
            });
        }
        throw new AccessDeniedError(`Permission denied: ${message}`, opts);
    }
    if (status === 404) {
        throw new NotFoundError(`Resource not found: ${message}`, opts);
    }
    if (status === 413) {
        throw new PayloadTooLargeError(`Payload too large: ${message}`, opts);
    }
    if (status === 422) {
        throw new ValidationError(`Validation error: ${message}`, opts);
    }
    if (status === 429) {
        const retryAfterRaw = response.headers.get("retry-after");
        throw new RateLimitError(`Rate limit exceeded: ${message}`, {
            ...opts,
            retryAfter: retryAfterRaw ? parseFloat(retryAfterRaw) : undefined,
        });
    }
    if (status >= 400 && status < 500) {
        throw new TrueFlowError(`Client error (${status}): ${message}`, opts);
    }
    if (status >= 500) {
        throw new GatewayError(`Gateway error (${status}): ${message}`, opts);
    }
}
