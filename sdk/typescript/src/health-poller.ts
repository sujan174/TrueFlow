/**
 * Background health poller — continuously monitors the TrueFlow gateway health.
 *
 * @example
 * ```ts
 * import { TrueFlowClient, HealthPoller } from "@trueflow/sdk";
 *
 * const client = new TrueFlowClient({ apiKey: "tf_v1_..." });
 * const poller = new HealthPoller(client, { intervalMs: 10_000 });
 * poller.start();
 *
 * // Hot path — zero extra HTTP requests:
 * if (poller.isHealthy) {
 *   const openai = client.openai();
 * } else {
 *   // use fallback
 * }
 *
 * poller.stop();
 * ```
 *
 * @module
 */

import type { TrueFlowClient } from "./client.js";

/** Options for configuring the health poller. */
export interface HealthPollerOptions {
    /** Polling interval in milliseconds (default: 15_000). */
    intervalMs?: number;
    /** Per-probe timeout in milliseconds (default: 3_000). */
    timeoutMs?: number;
}

/**
 * Background health poller that continuously probes the gateway's `/healthz`
 * endpoint and caches the result, so agents can check health on the critical
 * path without paying an HTTP round-trip per request.
 */
export class HealthPoller {
    private readonly client: TrueFlowClient;
    private readonly intervalMs: number;
    private readonly timeoutMs: number;
    private healthy = true; // optimistic default
    private timer: ReturnType<typeof setInterval> | undefined;

    constructor(client: TrueFlowClient, options: HealthPollerOptions = {}) {
        this.client = client;
        this.intervalMs = options.intervalMs ?? 15_000;
        this.timeoutMs = options.timeoutMs ?? 3_000;
    }

    /** True if the last health probe succeeded. */
    get isHealthy(): boolean {
        return this.healthy;
    }

    /**
     * Start the background polling timer.
     *
     * @returns `this` for chaining.
     */
    start(): this {
        // Immediately probe once, then repeat on interval
        void this.probe();
        this.timer = setInterval(() => void this.probe(), this.intervalMs);
        // Don't block process exit (Node.js)
        if (typeof this.timer === "object" && "unref" in this.timer) {
            (this.timer as NodeJS.Timeout).unref();
        }
        return this;
    }

    /** Stop the background polling timer. */
    stop(): void {
        if (this.timer !== undefined) {
            clearInterval(this.timer);
            this.timer = undefined;
        }
    }

    private async probe(): Promise<void> {
        this.healthy = await this.client.isHealthy({ timeoutMs: this.timeoutMs });
    }
}
