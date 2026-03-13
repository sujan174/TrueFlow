# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TrueFlow is an enterprise AI agent gateway built with Rust (Axum), TypeScript (Next.js), and Python. It sits between AI agents and upstream providers, providing security, policy enforcement, observability, and cost management.

**Core Architecture**: Agent requests flow through a Tower middleware stack where policies are evaluated, credentials are injected, and responses are processed before forwarding to LLM providers.

## Development Commands

### Full Stack Development
```bash
# Start dependencies (PostgreSQL, Redis)
docker compose up -d postgres redis

# Run gateway in dev mode (default port 8443)
cd gateway && cargo run

# Run gateway on alternate port (avoids port 8080 proxy)
cd gateway && cargo run -- serve --port 8081

# Run dashboard in dev mode
cd dashboard && npm install && npm run dev
```

### Gateway (Rust)
```bash
# Build and run
cargo build --release
cargo run

# Build with test hooks enabled (integration testing only, NEVER in production)
cargo build --features test-hooks

# Run tests
cargo test                              # All tests
cargo test --test integration           # Integration tests
cargo test --test adversarial_unit      # Unit tests (mocked upstreams)
cargo test --test full_path             # End-to-end path tests
cargo test test_token_creation          # Run specific test by name
cargo test --test integration -- test_name  # Specific test in integration suite

# Run load tests (requires k6, mock upstream on port 9000, gateway on 8082)
cd gateway/tests/loadtest && ./run_all.sh

# Linting and formatting
cargo clippy                            # Check for common issues
cargo fmt                               # Format code

# Database migrations
cargo run -- migrate                    # Run pending migrations
```

### CLI Commands
```bash
# Token management
cargo run -- token create --name my-token --credential cred-id --upstream openai
cargo run -- token list --project-id proj_123
cargo run -- token revoke --token-id tk_abc

# Credential management
cargo run -- credential add --name openai-key --provider openai --key sk-xxx
cargo run -- credential list --project-id proj_123

# Policy management
cargo run -- policy create --name rate-limit --rate-limit 10/min
cargo run -- policy list --project-id proj_123

# Declarative config (GitOps workflow)
cargo run -- config export --api-key admin-key > trueflow.yaml
cargo run -- config plan --file trueflow.yaml --api-key admin-key
cargo run -- config apply --file trueflow.yaml --api-key admin-key
```

### Python SDK
```bash
# Install development dependencies
cd sdk/python && pip install -e ".[openai,anthropic,langchain]"

# Run tests
python -m pytest tests/ -v              # All tests
python -m pytest tests/unit/ -v         # Unit tests only

# Run full E2E test suite (requires Docker)
python3 tests/e2e/test_mock_suite.py
```

### Dashboard (TypeScript)
```bash
cd dashboard
npm install
npm run dev                             # Development server on port 3000
npm run build                           # Production build
npm run lint                            # ESLint
```

## Architecture Overview

### Request Flow

```
Agent Request → Axum Handler
    → Request ID Middleware
    → Security Headers Middleware
    → CORS Layer
    → Body Limit (25MB)
    → Proxy Handler
        → Token resolution (virtual token → credential lookup)
        → Policy engine evaluation (pre-request phase)
        → PII redaction (patterns: SSN, email, credit card, etc.)
        → Spend cap check
        → Rate limiting (Redis-backed)
        → Load balancer selection (5 strategies)
        → Model router (provider detection, format translation)
        → Upstream request (with retries, circuit breaker)
        → Response processing (streaming SSE passthrough)
        → Policy engine evaluation (post-response phase)
        → Cost calculation & logging
        → Audit log write
```

### Component Structure

```
trueflow/
├── gateway/                  # Rust gateway - core proxy and policy engine
│   ├── src/
│   │   ├── main.rs          # Entry point, AppState, route definitions
│   │   ├── cli.rs           # CLI command definitions
│   │   ├── middleware/      # Tower middleware: auth, policy, guardrails, PII
│   │   │   ├── engine/      # Policy condition evaluation engine
│   │   │   ├── pii/         # PII detection patterns
│   │   │   └── guardrail/   # External guardrail integrations
│   │   ├── proxy/           # Upstream proxy handling
│   │   │   ├── handler/     # Main proxy handler
│   │   │   ├── model_router/# Provider-specific request/response translation
│   │   │   ├── loadbalancer.rs  # 5 LB strategies
│   │   │   └── stream.rs    # SSE streaming passthrough
│   │   ├── api/             # Management REST API handlers
│   │   ├── store/           # PostgreSQL database layer
│   │   ├── vault/           # AES-256-GCM envelope encryption
│   │   ├── models/          # Domain models: token, policy, cost
│   │   ├── cache/           # Tiered L1/L2 caching (in-memory + Redis)
│   │   ├── jobs/            # Background tasks
│   │   └── mcp/             # Model Context Protocol client
│   ├── tests/               # Integration and adversarial tests
│   │   ├── integration.rs   # Basic API flow tests
│   │   ├── adversarial_unit.rs  # Security boundary tests
│   │   ├── full_path.rs     # End-to-end request flow
│   │   └── loadtest/        # k6 load tests
│   └── migrations/          # SQL schema migrations (001–040+)
├── dashboard/               # Next.js admin UI (App Router, ShadCN)
│   └── src/
│       ├── app/             # Page routes (App Router)
│       └── components/      # Shared UI components
├── sdk/python/              # Python SDK with OpenAI/Anthropic drop-in
└── sdk/typescript/          # TypeScript SDK
```

### Key Domain Concepts

- **Virtual Token**: A `tf_v1_...` token that agents use instead of real API keys. Maps to a stored credential with attached policies and spend limits.
- **Credential**: Real API key stored in encrypted vault (AES-256-GCM envelope encryption).
- **Policy**: JSON rules with conditions (AND/OR logic) and actions (deny, rate-limit, redact, route, etc.).
- **Projects**: Multi-tenant isolation - tokens, credentials, and policies are scoped to projects.

### Required Environment Variables
- `DATABASE_URL`: PostgreSQL connection string
- `REDIS_URL`: Redis connection string
- `TRUEFLOW_MASTER_KEY`: 32-byte hex string for encryption
- `TRUEFLOW_ADMIN_KEY`: Admin API key for management operations
- `DASHBOARD_ORIGIN`: Allowed CORS origin for dashboard
- `DASHBOARD_SECRET`: Shared secret for dashboard proxy
- `TRUEFLOW_ENABLE_TEST_HOOKS`: Set to "1" in development only

**IMPORTANT**: Port 8080 is used by a proxy. Run the gateway locally on an alternate port (e.g., `--port 8081`).

### Optional Variables
- `RUST_LOG`: Logging level (info, debug, trace)
- `TRUEFLOW_LOG_FORMAT`: Set to "json" for SIEM-compatible structured logs
- `OTEL_EXPORTER_OTLP_ENDPOINT`: OpenTelemetry endpoint
- `LANGFUSE_PUBLIC_KEY`/`LANGFUSE_SECRET_KEY`: Langfuse integration
- `DD_API_KEY`: DataDog integration

## Provider Support

The gateway auto-detects providers by model name prefix:
- `gpt-*` → OpenAI
- `claude-*` → Anthropic
- `gemini-*` → Google Gemini
- `azure-*` → Azure OpenAI
- `bedrock-*` → AWS Bedrock
- `command-*` → Cohere
- `mistral-*` → Mistral
- `groq-*` → Groq
- `together-*` → Together AI
- `ollama-*` → Ollama

## Framework Integrations

Agents using OpenAI-compatible SDKs can point `base_url` at TrueFlow:
- **LangChain**: `langchain_openai.ChatOpenAI(base_url="http://localhost:8443/v1", api_key="tf_v1_...")`
- **CrewAI**: Use `TrueFlowLLM` wrapper from Python SDK
- **LlamaIndex**: Configure as custom LLM provider

## Observability

- **Tracing**: OpenTelemetry (OTLP) export to Jaeger/Tempo
- **Metrics**: Prometheus-compatible `/metrics` endpoint
- **Logging**: Structured JSON logging when `TRUEFLOW_LOG_FORMAT=json`
- **Exporters**: Langfuse, DataDog, custom webhooks
