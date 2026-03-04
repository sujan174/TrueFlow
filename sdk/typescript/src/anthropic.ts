/**
 * Anthropic drop-in wrapper — point your existing Anthropic client at the TrueFlow gateway.
 *
 * @example
 * ```ts
 * import { TrueFlowClient } from "@trueflow/sdk";
 *
 * const client = new TrueFlowClient({ apiKey: "tf_v1_..." });
 * const anthropic = client.anthropic();
 *
 * const msg = await anthropic.messages.create({
 *   model: "claude-sonnet-4-20250514",
 *   max_tokens: 1024,
 *   messages: [{ role: "user", content: "Hello!" }],
 * });
 * ```
 *
 * @module
 */

import { VERSION } from "./version.js";

/**
 * Create a configured Anthropic client that routes through the TrueFlow gateway.
 *
 * Requires the `@anthropic-ai/sdk` package as a peer dependency.
 *
 * @param gatewayUrl - The TrueFlow gateway URL.
 * @param apiKey - The TrueFlow virtual token.
 * @returns A configured Anthropic client instance.
 */
export function createAnthropicClient(gatewayUrl: string, apiKey: string): AnthropicClientLike {
    let AnthropicClass: AnthropicConstructor;
    try {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        AnthropicClass = require("@anthropic-ai/sdk").default ?? require("@anthropic-ai/sdk");
    } catch {
        throw new Error(
            "The '@anthropic-ai/sdk' package is required to use client.anthropic(). " +
            "Install it with: npm install @anthropic-ai/sdk",
        );
    }

    return new AnthropicClass({
        apiKey,
        baseURL: `${gatewayUrl.replace(/\/+$/, "")}/anthropic`,
        defaultHeaders: {
            "X-TrueFlow-SDK": `typescript/${VERSION}`,
        },
    });
}

interface AnthropicConstructor {
    new(opts: { apiKey: string; baseURL: string; defaultHeaders: Record<string, string> }): AnthropicClientLike;
}

/** Minimal type representing an Anthropic client. */
export interface AnthropicClientLike {
    messages: { create: (...args: unknown[]) => Promise<unknown> };
    [key: string]: unknown;
}
