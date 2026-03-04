/**
 * Low-level HTTP client for the TrueFlow gateway.
 *
 * Built on native `fetch` — zero dependencies. Handles:
 * - Automatic retries with exponential backoff on 429 / 5xx
 * - Configurable timeout via `AbortController`
 * - Request/response logging hooks
 * - SDK version header injection
 *
 * @module
 */

import { VERSION } from "./version.js";
import { raiseForStatus, RateLimitError } from "./error.js";

/** Options for configuring the internal HTTP client. */
export interface HttpClientOptions {
    /** Gateway base URL (e.g. `"http://localhost:8443"`). */
    baseUrl: string;
    /** Default headers sent with every request. */
    headers: Record<string, string>;
    /** Request timeout in milliseconds (default: 30_000). */
    timeoutMs?: number;
    /** Number of retries on 429 / 5xx (default: 2). */
    maxRetries?: number;
    /** Initial backoff in ms for retries (default: 500). */
    initialBackoffMs?: number;
}

/** Shape of a request passed to the HTTP client. */
export interface FetchOptions {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
    params?: Record<string, string | number | boolean | undefined>;
    signal?: AbortSignal;
}

/**
 * Internal HTTP client used by `TrueFlowClient`. Thin wrapper over `fetch`
 * with retry logic, timeout, and automatic error handling.
 */
export class HttpClient {
    readonly baseUrl: string;
    private readonly defaultHeaders: Record<string, string>;
    private readonly timeoutMs: number;
    private readonly maxRetries: number;
    private readonly initialBackoffMs: number;

    constructor(options: HttpClientOptions) {
        this.baseUrl = options.baseUrl.replace(/\/+$/, "");
        this.defaultHeaders = {
            ...options.headers,
            "X-TrueFlow-SDK-Version": VERSION,
            "Content-Type": "application/json",
        };
        this.timeoutMs = options.timeoutMs ?? 30_000;
        this.maxRetries = options.maxRetries ?? 2;
        this.initialBackoffMs = options.initialBackoffMs ?? 500;
    }

    /**
     * Execute a request with retry + timeout.
     *
     * @returns The raw `Response` (already checked for errors via `raiseForStatus`).
     */
    async request(path: string, options: FetchOptions = {}): Promise<Response> {
        const url = this.buildUrl(path, options.params);
        const headers: Record<string, string> = {
            ...this.defaultHeaders,
            ...options.headers,
        };
        const method = options.method ?? "GET";
        let lastError: unknown;

        for (let attempt = 0; attempt <= this.maxRetries; attempt++) {
            const controller = new AbortController();
            const timeout = setTimeout(() => controller.abort(), this.timeoutMs);

            try {
                const response = await fetch(url, {
                    method,
                    headers,
                    body: options.body,
                    signal: options.signal ?? controller.signal,
                });

                // Don't consume the body for retryable statuses — just check status
                if (this.isRetryable(response.status) && attempt < this.maxRetries) {
                    lastError = new Error(`HTTP ${response.status}`);
                    const backoff = this.computeBackoff(attempt, response);
                    await sleep(backoff);
                    continue;
                }

                await raiseForStatus(response);
                return response;
            } catch (error) {
                lastError = error;
                // Retry on network errors and rate limits, but not on typed TrueFlow errors
                // (those were already thrown by raiseForStatus)
                if (error instanceof RateLimitError && attempt < this.maxRetries) {
                    const backoff = error.retryAfter
                        ? error.retryAfter * 1000
                        : this.computeBackoff(attempt);
                    await sleep(backoff);
                    continue;
                }
                if (isNetworkError(error) && attempt < this.maxRetries) {
                    const backoff = this.computeBackoff(attempt);
                    await sleep(backoff);
                    continue;
                }
                throw error;
            } finally {
                clearTimeout(timeout);
            }
        }

        throw lastError;
    }

    // ── Convenience methods ─────────────────────────────────────────────────

    /** Send a GET request. */
    async get(path: string, options: Omit<FetchOptions, "method" | "body"> = {}): Promise<Response> {
        return this.request(path, { ...options, method: "GET" });
    }

    /** Send a POST request with a JSON body. */
    async post(path: string, body?: unknown, options: Omit<FetchOptions, "method" | "body"> = {}): Promise<Response> {
        return this.request(path, {
            ...options,
            method: "POST",
            body: body !== undefined ? JSON.stringify(body) : undefined,
        });
    }

    /** Send a PUT request with a JSON body. */
    async put(path: string, body?: unknown, options: Omit<FetchOptions, "method" | "body"> = {}): Promise<Response> {
        return this.request(path, {
            ...options,
            method: "PUT",
            body: body !== undefined ? JSON.stringify(body) : undefined,
        });
    }

    /** Send a PATCH request with a JSON body. */
    async patch(path: string, body?: unknown, options: Omit<FetchOptions, "method" | "body"> = {}): Promise<Response> {
        return this.request(path, {
            ...options,
            method: "PATCH",
            body: body !== undefined ? JSON.stringify(body) : undefined,
        });
    }

    /** Send a DELETE request. */
    async delete(path: string, options: Omit<FetchOptions, "method"> = {}): Promise<Response> {
        return this.request(path, { ...options, method: "DELETE" });
    }

    /**
     * Send a raw request (custom Content-Type, binary body, etc.).
     * Skips JSON serialization.
     */
    async raw(
        path: string,
        options: { method: string; body?: string | Uint8Array; headers?: Record<string, string>; params?: FetchOptions["params"] },
    ): Promise<Response> {
        const url = this.buildUrl(path, options.params);
        const headers: Record<string, string> = {
            ...this.defaultHeaders,
            ...options.headers,
        };
        // Remove default Content-Type if caller provides one
        if (options.headers?.["Content-Type"]) {
            headers["Content-Type"] = options.headers["Content-Type"];
        }
        const response = await fetch(url, {
            method: options.method,
            headers,
            body: options.body,
        });
        await raiseForStatus(response);
        return response;
    }

    // ── Internals ───────────────────────────────────────────────────────────

    private buildUrl(path: string, params?: Record<string, string | number | boolean | undefined>): string {
        const url = new URL(path, this.baseUrl);
        if (params) {
            for (const [key, value] of Object.entries(params)) {
                if (value !== undefined) {
                    url.searchParams.set(key, String(value));
                }
            }
        }
        return url.toString();
    }

    private isRetryable(status: number): boolean {
        return status === 429 || status >= 500;
    }

    private computeBackoff(attempt: number, response?: Response): number {
        // Respect Retry-After header if present
        if (response) {
            const retryAfter = response.headers.get("retry-after");
            if (retryAfter) {
                const seconds = parseFloat(retryAfter);
                if (!isNaN(seconds)) return seconds * 1000;
            }
        }
        // Exponential backoff with jitter
        const base = this.initialBackoffMs * Math.pow(2, attempt);
        const jitter = base * 0.1 * Math.random();
        return base + jitter;
    }
}

function isNetworkError(error: unknown): boolean {
    if (error instanceof TypeError) return true; // fetch network errors
    if (error instanceof DOMException && error.name === "AbortError") return false; // timeout, don't retry
    return false;
}

function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
