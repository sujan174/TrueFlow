/**
 * OpenAI drop-in wrapper — point your existing OpenAI client at the TrueFlow gateway.
 *
 * @example
 * ```ts
 * import { TrueFlowClient } from "@trueflow/sdk";
 *
 * const client = new TrueFlowClient({ apiKey: "tf_v1_..." });
 * const openai = client.openai();
 *
 * // Now use the standard OpenAI SDK — all requests route through TrueFlow
 * const response = await openai.chat.completions.create({
 *   model: "gpt-4o",
 *   messages: [{ role: "user", content: "Hello!" }],
 * });
 * ```
 *
 * @module
 */

import { VERSION } from "./version.js";

/**
 * Create a configured OpenAI client that routes through the TrueFlow gateway.
 *
 * Requires the `openai` package as a peer dependency.
 *
 * @param gatewayUrl - The TrueFlow gateway URL (e.g. `"http://localhost:8443"`).
 * @param apiKey - The TrueFlow virtual token.
 * @returns A configured `OpenAI` client instance.
 *
 * @example
 * ```ts
 * import { createOpenAIClient } from "@trueflow/sdk";
 *
 * const openai = createOpenAIClient("http://localhost:8443", "tf_v1_...");
 * const res = await openai.chat.completions.create({
 *   model: "gpt-4o",
 *   messages: [{ role: "user", content: "Hello!" }],
 * });
 * ```
 */
export function createOpenAIClient(gatewayUrl: string, apiKey: string): OpenAIClientLike {
    // Dynamic import to avoid hard dependency
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    let OpenAI: OpenAIConstructor;
    try {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        OpenAI = require("openai").default ?? require("openai");
    } catch {
        throw new Error(
            "The 'openai' package is required to use client.openai(). " +
            "Install it with: npm install openai",
        );
    }

    return new OpenAI({
        apiKey,
        baseURL: `${gatewayUrl.replace(/\/+$/, "")}/v1`,
        defaultHeaders: {
            "X-TrueFlow-SDK": `typescript/${VERSION}`,
        },
    });
}

// ── Minimal types so we don't depend on openai at compile time ──────────

interface OpenAIConstructor {
    new(opts: { apiKey: string; baseURL: string; defaultHeaders: Record<string, string> }): OpenAIClientLike;
}

/** Minimal type representing an OpenAI client. Use the real `OpenAI` type for full API. */
export interface OpenAIClientLike {
    chat: { completions: { create: (...args: unknown[]) => Promise<unknown> } };
    [key: string]: unknown;
}
