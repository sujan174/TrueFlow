/**
 * Realtime WebSocket sessions proxied through the TrueFlow gateway.
 *
 * @example
 * ```ts
 * const session = await client.realtime.connect({ model: "gpt-4o-realtime-preview" });
 * session.send({ type: "session.update", session: { modalities: ["text"] } });
 * for await (const event of session) {
 *   console.log(event.type);
 *   if (event.type === "response.done") break;
 * }
 * session.close();
 * ```
 *
 * @module
 */

import type { JsonObject } from "../types.js";

/** A Realtime WebSocket session. Send/receive JSON events as typed objects. */
export class RealtimeSession {
    private ws: WebSocket;

    /** @internal */
    constructor(ws: WebSocket) {
        this.ws = ws;
    }

    /** Send a Realtime API event. */
    send(event: JsonObject): void {
        this.ws.send(JSON.stringify(event));
    }

    /** Receive the next event. Resolves when a message arrives. */
    recv(): Promise<JsonObject> {
        return new Promise<JsonObject>((resolve, reject) => {
            const cleanup = () => {
                this.ws.removeEventListener("message", onMessage);
                this.ws.removeEventListener("error", onError);
            };
            const onMessage = (evt: MessageEvent) => {
                cleanup();
                try {
                    resolve(JSON.parse(String(evt.data)) as JsonObject);
                } catch {
                    reject(new Error("Failed to parse WebSocket message"));
                }
            };
            const onError = (evt: Event) => {
                cleanup();
                reject(new Error(`WebSocket error: ${evt.type}`));
            };
            this.ws.addEventListener("message", onMessage);
            this.ws.addEventListener("error", onError);
        });
    }

    /** Async iterator over incoming events until the connection closes. */
    async *[Symbol.asyncIterator](): AsyncIterator<JsonObject> {
        while (this.ws.readyState === WebSocket.OPEN) {
            try {
                yield await this.recv();
            } catch {
                return;
            }
        }
    }

    /** Close the WebSocket connection. */
    close(): void {
        this.ws.close();
    }
}

/** Options for connecting to the Realtime API. */
export interface RealtimeConnectOptions {
    /** The realtime model to use (default: `"gpt-4o-realtime-preview-2024-12-17"`). */
    model?: string;
    /** Extra headers to forward with the upgrade request. */
    additionalHeaders?: Record<string, string>;
}

export class RealtimeResource {
    private readonly gatewayUrl: string;
    private readonly apiKey: string;

    constructor(gatewayUrl: string, apiKey: string) {
        this.gatewayUrl = gatewayUrl;
        this.apiKey = apiKey;
    }

    /**
     * Open a Realtime WebSocket session through the TrueFlow gateway.
     *
     * @example
     * ```ts
     * const session = await client.realtime.connect({ model: "gpt-4o-realtime-preview" });
     * ```
     */
    async connect(options: RealtimeConnectOptions = {}): Promise<RealtimeSession> {
        const model = options.model ?? "gpt-4o-realtime-preview-2024-12-17";
        const wsUrl = this.gatewayUrl
            .replace(/^http:/, "ws:")
            .replace(/^https:/, "wss:");
        const url = `${wsUrl}/v1/realtime?model=${encodeURIComponent(model)}`;

        // Use subprotocol to pass auth (standard WebSocket API doesn't support custom headers)
        const protocols = ["realtime", `headers.authorization.bearer.${this.apiKey}`];
        if (options.additionalHeaders) {
            for (const [key, value] of Object.entries(options.additionalHeaders)) {
                protocols.push(`headers.${key.toLowerCase()}.${value}`);
            }
        }

        const ws = new WebSocket(url, protocols);

        return new Promise<RealtimeSession>((resolve, reject) => {
            ws.addEventListener("open", () => {
                resolve(new RealtimeSession(ws));
            });
            ws.addEventListener("error", (evt: Event) => {
                reject(new Error(`WebSocket connection failed: ${evt.type}`));
            });
        });
    }
}
