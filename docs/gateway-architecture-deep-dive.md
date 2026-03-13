# TrueFlow Gateway: Architectural Deep Dive

TrueFlow is an enterprise AI agent gateway built in Rust using the Axum web framework. It acts as a sophisticated, secure, and observable proxy between AI agents/applications and upstream LLM providers (OpenAI, Anthropic, Gemini, etc.).

## 1. System Architecture & Tech Stack

### Core Stack
*   **Language:** Rust (chosen for memory safety, performance, and predictable latency)
*   **Web Framework:** Axum (built on top of Tower and Hyper)
*   **Async Runtime:** Tokio
*   **Database:** PostgreSQL (accessed via `sqlx` for compile-time verified, type-safe queries)
*   **Cache & State:** Redis (accessed via `redis` crate) for L2 caching, rate limiting, distributed circuit breakers, and cross-instance spend tracking.
*   **HTTP Client:** `reqwest` (for upstream provider calls)

### High-Level Request Lifecycle (The Hot Path)

When a request hits the gateway (e.g., `POST /v1/chat/completions`), it flows through a highly structured pipeline:

1.  **Ingress & TLS:** Axum router accepts the connection (`src/main.rs`). A `DefaultBodyLimit` (25MB) and CORS layers are applied globally.
2.  **`proxy_handler` Entrypoint:** Requests land in `src/proxy/handler/core.rs`. The gateway parses the request body as JSON to understand the payload.
3.  **Token Resolution & Auth:** The `Authorization: Bearer tf_v1_...` token is extracted. The gateway looks up the token's configuration (upstream URL, credentials, attached policies) via a tiered cache (L1 in-memory, L2 Redis).
4.  **Policy Engine (Pre-Flight):** Policies attached to the token are evaluated synchronously against the request context (headers, body, token metadata).
5.  **Smart Routing / Load Balancing:** If the policy dictates dynamic routing or load balancing, the gateway selects the optimal upstream target based on health, latency, or cost.
6.  **Credential Injection (Envelope Encryption):** The gateway retrieves the encrypted provider API key associated with the project/token, decrypts it in memory, and injects it into the upstream request headers.
7.  **Upstream Proxying:** The request is forwarded to the LLM provider.
8.  **Streaming vs. Non-Streaming Handling:**
    *   *Non-Streaming:* The full response is buffered, cost is calculated, post-flight policies (like content filtering) are applied, and the response is sent to the client.
    *   *Streaming (SSE):* The connection is kept open. Chunks are passed through to the client instantly, while a background task (`StreamAccumulator`) buffers the chunks, extracts tool calls, redacts PII, and calculates the final cost when the stream ends.
9.  **Post-Flight & Observability:** Cost is deducted from spend caps, audit logs are generated, and background webhooks/tags are dispatched.

---

## 2. The Policy Engine (`src/middleware/engine/`)

The policy engine is the brain of TrueFlow. It uses a declarative JSON-Logic style DSL to evaluate conditions and execute actions.

### Architecture
Policies are evaluated in two distinct phases:
1.  **Pre-Flight (`Phase::Pre`):** Runs *before* the request is sent to the LLM. Used for rate limiting, blocking, prompt injection detection, routing, and request transformations.
2.  **Post-Flight (`Phase::Post`):** Runs *after* the LLM responds. Used for PII redaction, response schema validation, and logging.

Furthermore, policies operate in two modes:
*   **`Enforce`:** Actions are executed (e.g., a request is actually blocked).
*   **`Shadow`:** The engine logs that a policy *would* have triggered, but does not interfere with the request. This is crucial for safely testing new security rules in production.

### How Middleware is Evaluated (`evaluate_policies` in `engine.rs`)

1.  **Context Construction:** The gateway builds a `RequestContext` containing the parsed JSON body, headers, token ID, project ID, and extracted `target_model`.
2.  **Rule Iteration:** For every policy attached to the token matching the current phase, the engine iterates through its `rules`.
3.  **Condition Matching:** `evaluate_condition(&rule.when, ctx)` evaluates the JSON-logic conditions (e.g., `request.body.model == 'gpt-4'`, `contains(request.body.messages[].content, 'secret')`).
4.  **Action Queuing:** If a condition matches:
    *   If `Shadow` mode: A log entry is generated.
    *   If `Enforce` mode: The actions defined in `rule.then` are collected into a queue of `TriggeredAction` structs.
    *   *Async Actions:* If `rule.async_check` is true, the action is pushed to an `async_triggered` queue to run in the background (non-blocking).
5.  **Execution (`match action.action`):** Back in the `proxy_handler`, the gateway iterates over the queued actions and executes them.
    *   *Deny:* Returns a 403 immediately.
    *   *RateLimit:* Checks Redis. If exceeded, returns 429.
    *   *Redact:* Modifies the JSON body to mask sensitive data.
    *   *DynamicRoute:* Invokes the `SmartRouter`.

---

## 3. Dynamic Routing & Load Balancing (`src/proxy/smart_router.rs`)

When a policy triggers a `DynamicRoute` action, TrueFlow dynamically selects the upstream provider rather than using the token's default.

### Routing Strategies
The router supports multiple strategies (`RoutingStrategy` enum):
1.  **`Latency`:** Selects the provider with the lowest historical Time-To-First-Token (TTFT). Requires the `LatencyCache` to be populated by past requests.
2.  **`Cost`:** Selects the cheapest provider capable of serving the request. It calculates the projected cost based on the input tokens (if known/estimable) and the provider's pricing.
3.  **`RoundRobin`:** Distributes requests evenly across a pool of targets. TrueFlow uses a highly concurrent `DashMap` holding `AtomicU64` counters per token to ensure lock-free round-robin distribution across async workers.

### Circuit Breaking
Integrated into routing is a distributed Circuit Breaker. If an upstream target consistently fails (e.g., OpenAI is down), the circuit breaker opens (tracked in Redis so all gateway instances know). The `SmartRouter` will automatically filter out targets with open circuit breakers, falling back to healthy providers.

---

## 4. Cost Calculation & Spend Caps (`src/middleware/spend.rs` & `src/models/cost.rs`)

TrueFlow tracks costs with high precision using `rust_decimal` to prevent floating-point inaccuracies when dealing with fractions of a cent.

### Pricing Model (`models/cost.rs`)
1.  **Pricing Cache:** TrueFlow maintains a `PricingCache` in memory, populated from the `model_pricing` PostgreSQL table.
2.  **Lookup:** It looks up the `input_cost_per_m` (per million tokens) and `output_cost_per_m` based on the provider and a regex pattern of the model name (e.g., `^gpt-4o-mini.*`).
3.  **Calculation:** `cost = (input_tokens / 1M * input_rate) + (output_tokens / 1M * output_rate)`.

### Normal vs. Streaming (SSE) Cost Extraction

*   **Normal (Blocking) Requests:**
    When the upstream returns a 200 OK, the gateway parses the entire JSON response. It looks for standard usage blocks:
    *   OpenAI/Anthropic: `response.usage.prompt_tokens` and `completion_tokens`.
    *   Gemini: `response.usageMetadata.promptTokenCount` and `candidatesTokenCount`.
    The cost is calculated immediately, and `check_and_increment_spend` is called.

*   **Streaming (SSE) Requests (`src/proxy/stream_bridge.rs`):**
    Because SSE chunks are sent to the client immediately (to maintain low latency), the gateway cannot know the total tokens until the stream ends.
    1.  **Teeing:** The gateway "tees" the stream. One branch goes directly to the Axum response body (to the client). The other branch goes into a background Tokio task running the `StreamAccumulator`.
    2.  **Accumulation:** The accumulator parses the `data: ...` chunks.
    3.  **Usage Extraction:** Modern providers send a final chunk containing the usage statistics (e.g., OpenAI sends an empty choice with a `usage` object at the very end). The accumulator extracts this.
    4.  **Fallback (Token Counting):** If the provider *doesn't* send usage in the stream, TrueFlow must theoretically fallback to local tokenization (though the code primarily relies on provider usage blocks).
    5.  **Delayed Billing:** Once the stream completes, the background task calculates the final cost and updates the spend tracking in Redis/Postgres asynchronously.

### Spend Enforcement (The Concurrency Challenge)

Enforcing spend caps (Daily, Monthly, Lifetime) accurately in a highly concurrent environment is notoriously difficult due to Time-Of-Check to Time-Of-Use (TOCTOU) race conditions.

**How TrueFlow solves it (`check_and_increment_spend`):**
TrueFlow uses a single, atomic **Redis Lua Script** to handle cross-cap checking and incrementing.
1.  The script receives the daily, monthly, and lifetime limits, and the cost of the current request.
2.  *Phase 1:* It increments all relevant counters using `INCRBYFLOAT`.
3.  *Phase 2:* It checks if the *new* values exceed the limits.
4.  If a limit is exceeded, it returns a denial (e.g., `"DAILY"`). Note: A recent fix in the codebase ensures that if a limit is exceeded, the counter is *not* artificially inflated by denied requests.
5.  If allowed, it returns `"OK"`.
6.  *Persistence:* A background Tokio task (`tokio::spawn`) is fired to eventually persist these Redis increments to the durable PostgreSQL database (`update_db_spend`).

---

## 5. Security: Envelope Encryption (`src/vault/builtin.rs`)

TrueFlow never stores plaintext provider API keys in the database. It uses an **Envelope Encryption** architecture.

1.  **KEK (Key Encryption Key):** A 32-byte Master Key provided via the `TRUEFLOW_MASTER_KEY` environment variable. This key *never* touches the database.
2.  **DEK (Data Encryption Key):** When a new credential is created, TrueFlow generates a unique random DEK.
3.  **Encryption:**
    *   The plaintext API key is encrypted using AES-256-GCM with the DEK.
    *   The DEK itself is encrypted using AES-256-GCM with the KEK.
4.  **Storage:** The database stores a tuple: `(encrypted_dek, dek_nonce, encrypted_api_key, api_key_nonce)`.
5.  **Decryption at Runtime:** When a request arrives, the gateway fetches the tuple, decrypts the DEK using the in-memory KEK, and then uses the plaintext DEK to decrypt the API key, injecting it into the `Authorization` header sent to OpenAI/Anthropic.

---

## 6. Observability & Auditing (`src/middleware/audit.rs`)

TrueFlow is designed for enterprise auditability.
*   Every request generates an `AuditLog` entry.
*   The log includes the Request ID, Token ID, Project ID, Method, Path, Provider, Status Code, Tokens Used, Cost, and Latency (TTFT and Total Duration).
*   If a policy modifies the request (e.g., PII Redaction), the `redacted_fields` are noted in the audit log.
*   **Performance:** Audit logs are fired asynchronously. The main proxy handler does not wait for the database insert to complete.

---

## Interview Cheat Sheet: Key Concepts to Nail

*   **Concurrency:** Emphasize that you understand Rust's `Arc`, `Mutex`, `RwLock`, and how TrueFlow uses `tokio::spawn` to move heavy tasks (like streaming accumulation and DB persistence) off the critical hot path.
*   **The SSE Challenge:** Be ready to explain *why* streaming is hard. (You don't know the cost or the full content until the end, so you have to buffer it in the background while still passing bytes to the client).
*   **TOCTOU in Billing:** Explain the Redis Lua script approach for spend caps. Checking a balance and then incrementing it requires atomic operations to prevent users from bypassing limits with concurrent requests.
*   **Zero-Trust/Encryption:** Explain Envelope Encryption. It limits the blast radius; if the database is dumped, the API keys are still safe unless the attacker also compromises the host memory containing the `TRUEFLOW_MASTER_KEY`.
*   **Extensibility:** The policy engine's JSON-Logic design means non-engineers (e.g., security teams) can write rules to block specific behaviors without deploying new code.
