import type { HttpClient } from "../http.js";
import type { JsonObject } from "../types.js";

/**
 * Experiment tracking — A/B testing for models, prompts, and routing strategies.
 *
 * @example
 * ```ts
 * const admin = TrueFlowClient.admin({ adminKey: "..." });
 *
 * // Create an experiment
 * const exp = await admin.experiments.create({
 *   name: "gpt4o-vs-claude",
 *   variants: [
 *     { name: "control", weight: 50, model: "gpt-4o" },
 *     { name: "treatment", weight: 50, model: "claude-3-5-sonnet-20241022" },
 *   ],
 * });
 *
 * // Check results
 * const results = await admin.experiments.results(exp.id);
 *
 * // Stop when done
 * await admin.experiments.stop(exp.id);
 * ```
 */
export class ExperimentsResource {
    constructor(private readonly http: HttpClient) { }

    /** Create an A/B experiment. */
    async create(options: {
        name: string;
        variants: Array<{
            name: string;
            weight: number;
            model?: string;
            set_body_fields?: Record<string, unknown>;
        }>;
        condition?: Record<string, unknown>;
    }): Promise<JsonObject> {
        const body: Record<string, unknown> = {
            name: options.name,
            variants: options.variants,
        };
        if (options.condition) body["condition"] = options.condition;
        const res = await this.http.post("/api/v1/experiments", body);
        return (await res.json()) as JsonObject;
    }

    /** List all running experiments. */
    async list(): Promise<JsonObject[]> {
        const res = await this.http.get("/api/v1/experiments");
        return (await res.json()) as JsonObject[];
    }

    /** Get an experiment with its analytics. */
    async get(experimentId: string): Promise<JsonObject> {
        const res = await this.http.get(`/api/v1/experiments/${experimentId}`);
        return (await res.json()) as JsonObject;
    }

    /** Get per-variant results for an experiment. */
    async results(experimentId: string): Promise<JsonObject> {
        const res = await this.http.get(`/api/v1/experiments/${experimentId}/results`);
        return (await res.json()) as JsonObject;
    }

    /** Stop a running experiment. */
    async stop(experimentId: string): Promise<JsonObject> {
        const res = await this.http.post(`/api/v1/experiments/${experimentId}/stop`, {});
        return (await res.json()) as JsonObject;
    }

    /** Update variant weights for a running experiment. */
    async update(
        experimentId: string,
        options: {
            variants: Array<{
                name: string;
                weight: number;
                model?: string;
                set_body_fields?: Record<string, unknown>;
            }>;
        },
    ): Promise<JsonObject> {
        const res = await this.http.put(`/api/v1/experiments/${experimentId}`, {
            variants: options.variants,
        });
        return (await res.json()) as JsonObject;
    }
}
