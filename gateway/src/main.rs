use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::extract::DefaultBodyLimit;
use axum::routing::any;
use clap::Parser;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod cache;
mod cli;
mod config;
mod errors;
mod iac;
mod jobs;
mod mcp;
mod middleware;
mod models;
mod notification;
mod proxy;
mod rotation;
mod store;
mod utils;
mod vault;

use cache::TieredCache;
use store::payload_store::PayloadStore;
use store::postgres::PgStore;
use vault::builtin::BuiltinStore;

/// Shared application state passed to handlers and middleware.
pub struct AppState {
    pub db: PgStore,
    pub vault: BuiltinStore,
    pub cache: TieredCache,
    pub upstream_client: proxy::upstream::UpstreamClient,
    pub notifier: notification::slack::SlackNotifier,
    pub webhook: notification::webhook::WebhookNotifier,
    pub config: config::Config,
    pub lb: proxy::loadbalancer::LoadBalancer,
    pub pricing: models::pricing_cache::PricingCache,
    /// p50 latency per model (refreshed every 5min from audit_logs).
    pub latency: models::latency_cache::LatencyCache,
    /// Payload storage backend — Postgres (default) or S3/MinIO/local.
    pub payload_store: Arc<PayloadStore>,
    /// Observability exporters: Prometheus, Langfuse, DataDog.
    pub observer: Arc<middleware::observer::ObserverHub>,
    /// MCP server registry — manages connections and cached tool schemas.
    pub mcp_registry: Arc<mcp::registry::McpRegistry>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure OpenTelemetry (OTLP) -> Jaeger
    // We try to connect to OTELL_EXPORTER_OTLP_ENDPOINT or default localhost:4317
    // If it fails, we fallback to just logging to stdout?
    // Actually, init_tracer usually logs error if fails but doesn't panic main app unless unwrapped.

    use opentelemetry::KeyValue;

    use opentelemetry_sdk::{trace as sdktrace, Resource};

    let telemetry_layer = if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(opentelemetry_otlp::new_exporter().tonic())
            .with_trace_config(sdktrace::config().with_resource(Resource::new(vec![
                KeyValue::new("service.name", "trueflow-gateway"),
            ])))
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .expect("failed to install OpenTelemetry tracer");
        Some(tracing_opentelemetry::layer().with_tracer(tracer))
    } else {
        None
    };

    let env_filter = tracing_subscriber::EnvFilter::new(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "gateway=debug,tower_http=debug".into()),
    );

    // SIEM-ready JSON logs: set TRUEFLOW_LOG_FORMAT=json for structured output
    // compatible with Splunk, Datadog, ELK, CloudWatch.
    let use_json = std::env::var("TRUEFLOW_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let json_layer = if use_json {
        Some(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_span_list(true)
                .flatten_event(true),
        )
    } else {
        None
    };
    let plain_layer = if !use_json {
        Some(tracing_subscriber::fmt::layer())
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(json_layer)
        .with(plain_layer)
        .with(telemetry_layer)
        .init();

    let args = cli::Cli::parse();
    let cfg = config::load()?;

    let result = match args.command {
        Some(cli::Commands::Serve { port }) => run_server(cfg, port).await,
        Some(cli::Commands::Token { command }) => {
            let db = PgStore::connect(&cfg.database_url).await?;
            let vault = BuiltinStore::new(&cfg.master_key, db.pool().clone())?;
            let redis_client = redis::Client::open(cfg.redis_url.as_str())?;
            let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
            let cache = TieredCache::new(redis_conn);
            let upstream_client = proxy::upstream::UpstreamClient::new();
            let notifier = notification::slack::SlackNotifier::new(cfg.slack_webhook_url.clone());

            let lb_redis = cache.redis();
            let state = Arc::new(AppState {
                db,
                vault,
                cache,
                upstream_client,
                notifier,
                webhook: notification::webhook::WebhookNotifier::new(),
                config: cfg,
                lb: proxy::loadbalancer::LoadBalancer::new_with_redis(lb_redis),
                pricing: models::pricing_cache::PricingCache::new(),
                latency: models::latency_cache::LatencyCache::new(),
                payload_store: Arc::new(PayloadStore::from_env().unwrap_or(PayloadStore::Postgres)),
                observer: Arc::new(middleware::observer::ObserverHub::from_env()),
                mcp_registry: Arc::new(mcp::registry::McpRegistry::new()),
            });

            handle_token_command(command, &state).await
        }
        Some(cli::Commands::Credential { command }) => {
            let db = PgStore::connect(&cfg.database_url).await?;
            handle_credential_command(&db, &cfg, command).await
        }
        Some(cli::Commands::Approval { command }) => {
            let db = PgStore::connect(&cfg.database_url).await?;
            handle_approval_command(&db, command).await
        }
        Some(cli::Commands::Policy { command }) => {
            let db = PgStore::connect(&cfg.database_url).await?;
            let vault = BuiltinStore::new(&cfg.master_key, db.pool().clone())?;
            let redis_client = redis::Client::open(cfg.redis_url.as_str())?;
            let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
            let cache = TieredCache::new(redis_conn);
            let upstream_client = proxy::upstream::UpstreamClient::new();
            let notifier = notification::slack::SlackNotifier::new(cfg.slack_webhook_url.clone());

            let lb_redis = cache.redis();
            let state = Arc::new(AppState {
                db,
                vault,
                cache,
                upstream_client,
                notifier,
                webhook: notification::webhook::WebhookNotifier::new(),
                config: cfg,
                lb: proxy::loadbalancer::LoadBalancer::new_with_redis(lb_redis),
                pricing: models::pricing_cache::PricingCache::new(),
                latency: models::latency_cache::LatencyCache::new(),
                payload_store: Arc::new(PayloadStore::from_env().unwrap_or(PayloadStore::Postgres)),
                observer: Arc::new(middleware::observer::ObserverHub::from_env()),
                mcp_registry: Arc::new(mcp::registry::McpRegistry::new()),
            });

            handle_policy_command(command, &state).await
        }
        Some(cli::Commands::Config { command }) => handle_config_command(command).await,
        None => run_server(cfg, 8443).await,
    };

    if let Err(ref e) = result {
        eprintln!("Error: {:?}", e);
    }
    result
}

async fn run_server(cfg: config::Config, port: u16) -> anyhow::Result<()> {
    tracing::info!("Connecting to database...");
    let db = PgStore::connect(&cfg.database_url).await?;

    tracing::info!("Running migrations...");
    db.migrate().await?;

    tracing::info!("Initializing vault...");
    let vault = BuiltinStore::new(&cfg.master_key, db.pool().clone())?;

    tracing::info!("Connecting to Redis...");
    // Redis is required for rate limiting, caching, and spend cap enforcement.
    // If Redis is unavailable, the readiness probe returns 503 and the load balancer
    // stops routing traffic to this instance.
    // Degraded-mode operation without Redis is not supported for MVP.
    let redis_client = redis::Client::open(cfg.redis_url.as_str())?;
    // Use tokio::spawn to create connection manager properly in async context if needed,
    // but ConnectionManager::new is async.
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    let cache = TieredCache::new(redis_conn);

    let upstream_client = proxy::upstream::UpstreamClient::new();
    let notifier = notification::slack::SlackNotifier::new(cfg.slack_webhook_url.clone());

    let pricing = models::pricing_cache::PricingCache::new();
    let latency = models::latency_cache::LatencyCache::new();

    tracing::info!("Initializing payload store...");
    let payload_store = Arc::new(PayloadStore::from_env().context("invalid PAYLOAD_STORE_URL")?);

    let lb_redis = cache.redis();
    let state = Arc::new(AppState {
        db,
        vault,
        cache,
        upstream_client,
        notifier,
        webhook: notification::webhook::WebhookNotifier::new(),
        config: cfg,
        lb: proxy::loadbalancer::LoadBalancer::new_with_redis(lb_redis),
        pricing: pricing.clone(),
        latency: latency.clone(),
        payload_store,
        observer: Arc::new(middleware::observer::ObserverHub::from_env()),
        mcp_registry: Arc::new(mcp::registry::McpRegistry::new()),
    });

    // Load initial pricing from DB into the in-memory cache
    match state.db.list_model_pricing().await {
        Ok(rows) => {
            let entries = rows
                .into_iter()
                .map(|r| models::pricing_cache::PricingEntry {
                    provider: r.provider,
                    model_pattern: r.model_pattern,
                    input_per_m: r.input_per_m,
                    output_per_m: r.output_per_m,
                })
                .collect();
            pricing.reload(entries).await;
            tracing::info!("Loaded model pricing from DB");
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load model pricing from DB, using hardcoded fallback: {}",
                e
            );
        }
    }

    // ── MCP Server Restoration ──────────────────────────────────────────────
    // Load persisted MCP servers from DB and restore them to the in-memory registry.
    tracing::info!("Restoring persisted MCP servers...");
    match state.db.list_all_mcp_servers().await {
        Ok(servers) => {
            let count = servers.len();
            for server in servers {
                // Attempt to reconnect each persisted server
                match mcp::registry::McpServerConfig::from_persisted(&server) {
                    Ok(config) => {
                        // Load cached tools from DB
                        let tools = state.db.get_mcp_server_tools(server.id).await.ok();
                        let tool_names: Vec<String> = tools
                            .as_ref()
                            .map(|t| t.iter().map(|tool| tool.name.clone()).collect())
                            .unwrap_or_default();

                        // Register in memory (will reconnect on next request)
                        match state.mcp_registry.register_from_persisted(config, tools).await {
                            Ok(_) => {
                                tracing::info!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    tool_count = tool_names.len(),
                                    "MCP server restored from persistence"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    error = %e,
                                    "Failed to restore MCP server, will retry on next request"
                                );
                                // Mark as disconnected but keep in registry for retry
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            server_id = %server.id,
                            error = %e,
                            "Failed to parse persisted MCP server config"
                        );
                    }
                }
            }
            tracing::info!("Restored {} MCP servers from persistence", count);
        }
        Err(e) => {
            tracing::warn!("Failed to load persisted MCP servers: {}", e);
        }
    }

    // ── Key Rotation Scheduler ──────────────────────────────────────────────
    // Opt-in: set TRUEFLOW_ROTATION_ENABLED=true to enable background key rotation.
    if std::env::var("TRUEFLOW_ROTATION_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
    {
        let rotation_interval: u64 = std::env::var("TRUEFLOW_ROTATION_CHECK_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600); // default: check every hour

        match crate::vault::builtin::VaultCrypto::new(&state.config.master_key) {
            Ok(vault_crypto) => {
                let scheduler = Arc::new(rotation::RotationScheduler::new(
                    state.db.clone(),
                    vault_crypto,
                    state.cache.clone(),
                    rotation_interval,
                ));
                scheduler.start();
                tracing::info!(
                    check_interval_secs = rotation_interval,
                    "Key rotation scheduler enabled"
                );
            }
            Err(e) => {
                tracing::error!("Failed to init VaultCrypto for rotation: {}", e);
            }
        }
    }

    let app = axum::Router::new()
        // Health endpoints (no auth)
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .route("/readyz", axum::routing::get(readiness_check_layer))
        // Prometheus metrics (no auth — standard for /metrics)
        .route("/metrics", axum::routing::get(prometheus_metrics_handler))
        // Realtime WebSocket proxy — must come before the catch-all fallback
        .route(
            "/v1/realtime",
            axum::routing::get(proxy::realtime::realtime_handler),
        )
        // Management API — nested under /api/v1 (preserves middleware + fallback)
        .nest("/api/v1", api::api_router(state.clone()))
        // Proxy: catch everything else
        .fallback(any(proxy::handler::proxy_handler))
        .with_state(state.clone())
        // Enforce 25 MB body size limit on all routes
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        // SEC-06: Restrict CORS origins.
        // - Dev: allows any localhost:* for convenience
        // - Production (TRUEFLOW_ENV=production): only the explicit DASHBOARD_ORIGIN is permitted
        .layer({
            use axum::http::{HeaderName, Method};
            use tower_http::cors::AllowOrigin;
            let dashboard_origin = std::env::var("DASHBOARD_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:3000".to_string());
            let is_production = std::env::var("TRUEFLOW_ENV")
                .map(|v| v == "production")
                .unwrap_or(false);
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(move |origin, _| {
                    let origin_str = origin.to_str().unwrap_or("");
                    if origin_str == dashboard_origin {
                        return true;
                    }
                    // In production, do NOT allow arbitrary localhost origins
                    if is_production {
                        return false;
                    }
                    origin_str.starts_with("http://localhost:")
                        || origin_str.starts_with("http://127.0.0.1:")
                }))
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::PATCH,
                    Method::OPTIONS,
                ])
                // NOTE: Cannot use AllowHeaders::any() with allow_credentials(true) per CORS spec
                .allow_headers([
                    HeaderName::from_static("content-type"),
                    HeaderName::from_static("authorization"),
                    HeaderName::from_static("x-admin-key"),
                    HeaderName::from_static("x-dashboard-token"),
                    HeaderName::from_static("x-request-id"),
                ])
                .allow_credentials(true)
        })
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(axum::middleware::from_fn(security_headers_middleware));

    // Phase 4: Start background cleanup job for Level 2 log expiry
    jobs::cleanup::spawn(state.db.pool().clone());
    tracing::info!("Background cleanup job started (Level 2 log expiry every 1h)");

    // Phase 5: Start approval expiry job (every 60 seconds)
    jobs::approval_expiry::spawn(state.db.pool().clone());
    tracing::info!("Approval expiry job started (every 60s)");

    // Phase 5.1: Start session cleanup job (every 15 minutes)
    jobs::session_cleanup::spawn(state.db.pool().clone());
    tracing::info!("Session cleanup job started (orphaned session expiry every 15min)");

    // Phase 2.3: Start budget check job (every 15 minutes)
    {
        let budget_pool = state.db.pool().clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(900)); // 15min
            loop {
                interval.tick().await;
                if let Err(e) = jobs::budget_checker::run_budget_check(&budget_pool).await {
                    tracing::error!(error = %e, "budget check job failed");
                }
            }
        });
        tracing::info!("Budget check job started (project spend alerts every 15min)");
    }

    // Phase 2.5: Start latency cache refresh job for DynamicRoute (every 5 minutes)
    {
        let latency_pool = state.db.pool().clone();
        let latency_cache = latency.clone();
        tokio::spawn(async move {
            // Initial load at startup
            latency_cache.reload(&latency_pool).await;
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5min
            loop {
                interval.tick().await;
                latency_cache.reload(&latency_pool).await;
            }
        });
        tracing::info!("Latency cache refresh job started (p50 per model every 5min)");
    }

    // Phase 2.4: Periodic in-memory cache eviction (every 60s)
    {
        let eviction_cache = state.cache.local.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let now = std::time::Instant::now();
                let before = eviction_cache.len();
                eviction_cache.retain(|_, entry| entry.expires_at > now);
                let removed = before - eviction_cache.len();
                if removed > 0 {
                    tracing::debug!(
                        removed,
                        remaining = eviction_cache.len(),
                        "evicted expired local cache entries"
                    );
                }
            }
        });
        tracing::info!("Local cache eviction job started (every 60s)");
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("TrueFlow gateway listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

/// Middleware: injects a unique X-Request-Id into every response.
/// This allows clients to correlate errors with gateway logs.
async fn request_id_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let req_id = uuid::Uuid::new_v4().to_string();
    let mut resp = next.run(req).await;
    if let Ok(val) = axum::http::HeaderValue::from_str(&req_id) {
        resp.headers_mut().insert("x-request-id", val);
    }
    resp
}

/// Middleware layer that extracts state for readiness check.
async fn readiness_check_layer(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    readiness_check(&state).await
}

/// Readiness probe: checks database and Redis connectivity.
/// Returns 200 if both are healthy, 503 otherwise.
async fn readiness_check(state: &AppState) -> (axum::http::StatusCode, &'static str) {
    // Check database connectivity
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(state.db.pool())
        .await
        .is_ok();

    // Check Redis connectivity
    let redis_ok = state.cache.ping().await;

    if db_ok && redis_ok {
        (axum::http::StatusCode::OK, "ok")
    } else {
        let reason = match (db_ok, redis_ok) {
            (false, false) => "database and redis unavailable",
            (false, true) => "database unavailable",
            (true, false) => "redis unavailable",
            (true, true) => unreachable!(),
        };
        tracing::warn!(db_ok, redis_ok, "readiness check failed: {}", reason);
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, reason)
    }
}

/// GET /metrics — Prometheus text exposition format.
/// Unauthenticated (standard for Prometheus scrape targets).
async fn prometheus_metrics_handler() -> axum::response::Response<axum::body::Body> {
    let body = middleware::metrics::encode_metrics();
    axum::response::Response::builder()
        .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| {
            axum::response::Response::new(axum::body::Body::from("# error encoding metrics\n"))
        })
}

/// Middleware: injects security headers into every response.
/// These protect against XSS, clickjacking, MIME sniffing, and info leakage.
async fn security_headers_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();

    // Prevent MIME-type sniffing (e.g., interpreting a .txt as HTML)
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());

    // Prevent clickjacking by disallowing iframe embedding
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());

    // Enable browser XSS filter (legacy but still useful)
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());

    // Prevent the browser from caching sensitive API responses
    headers.insert("Cache-Control", "no-store".parse().unwrap());

    // Strip Referrer to avoid leaking tokens in URLs
    headers.insert("Referrer-Policy", "no-referrer".parse().unwrap());

    // Restrict permissions (camera, mic, geolocation, etc.)
    headers.insert(
        "Permissions-Policy",
        "camera=(), microphone=(), geolocation=()".parse().unwrap(),
    );

    // Remove server identity header
    headers.remove("Server");

    // HSTS: Instructs browsers to only use HTTPS.
    // Currently set to max-age=0 (no-op) for HTTP dev environments.
    // When TLS is enabled in production, change to:
    //   "max-age=63072000; includeSubDomains; preload"
    let is_production = std::env::var("TRUEFLOW_ENV")
        .map(|v| v == "production")
        .unwrap_or(false);
    let hsts_value = if is_production {
        "max-age=63072000; includeSubDomains; preload"
    } else {
        "max-age=0"
    };
    headers.insert("Strict-Transport-Security", hsts_value.parse().unwrap());

    resp
}

async fn handle_policy_command(
    cmd: cli::PolicyCommands,
    state: &Arc<AppState>,
) -> anyhow::Result<()> {
    match cmd {
        cli::PolicyCommands::Create {
            name,
            mode,
            phase,
            project_id,
            rate_limit,
            hitl_timeout,
            hitl_fallback,
        } => {
            let pid = parse_project_id(project_id)?;

            let mut rules = Vec::new();

            if let Some(rl) = rate_limit {
                // Format: "10/min" -> { "type": "rate_limit", "window": "min", "max_requests": 10 }
                let parts: Vec<&str> = rl.split('/').collect();
                if parts.len() != 2 {
                    anyhow::bail!("Invalid rate_limit format. Expected 'MAX/WINDOW' (e.g. 10/min)");
                }
                let count: u64 = parts[0].parse().context("Invalid rate limit count")?;
                let window = parts[1].to_string();
                rules.push(serde_json::json!({
                    "type": "rate_limit",
                    "window": window,
                    "max_requests": count
                }));
            }

            if let Some(timeout) = hitl_timeout {
                let fallback = hitl_fallback.unwrap_or_else(|| "reject".to_string());
                rules.push(serde_json::json!({
                    "type": "human_approval",
                    "timeout": timeout,
                    "fallback": fallback
                }));
            }

            if phase != "pre" && phase != "post" {
                anyhow::bail!("Invalid phase: {}. Must be 'pre' or 'post'", phase);
            }

            let rules_json = serde_json::to_value(rules)?;
            let id = state
                .db
                .insert_policy(pid, &name, &mode, &phase, rules_json, None)
                .await?;
            println!(
                "Policy created:\n  Name:     {}\n  ID:       {}\n  Mode:     {}\n  Phase:    {}",
                name, id, mode, phase
            );
        }
        cli::PolicyCommands::List { project_id } => {
            let pid = uuid::Uuid::parse_str(&project_id).context("Invalid project_id")?;
            let policies = state.db.list_policies(pid, 1000, 0).await?;
            if policies.is_empty() {
                println!("No policies found.");
            } else {
                println!(
                    "{:<38} {:<20} {:<10} {:<10}",
                    "ID", "NAME", "MODE", "ACTIVE"
                );
                for p in policies {
                    println!(
                        "{:<38} {:<20} {:<10} {:<10}",
                        p.id, p.name, p.mode, p.is_active
                    );
                }
            }
        }
        cli::PolicyCommands::Delete { id, project_id } => {
            let pid = parse_project_id(project_id)?;
            let pol_uuid = uuid::Uuid::parse_str(&id).context("Invalid policy ID")?;
            let deleted = state.db.delete_policy(pol_uuid, pid).await?;
            if deleted {
                println!("Policy deleted.");
            } else {
                println!("Policy not found or already deleted.");
            }
        }
    }
    Ok(())
}

async fn handle_token_command(
    cmd: cli::TokenCommands,
    state: &Arc<AppState>,
) -> anyhow::Result<()> {
    match cmd {
        cli::TokenCommands::Create {
            name,
            credential,
            upstream,
            project_id,
            policy_ids,
        } => {
            let pid = parse_project_id(project_id)?;

            // Resolve credential ID (could be name or UUID)
            // Ideally we should lookup by name if not UUID, but for now let's try UUID first
            let cred_id = if let Ok(uuid) = uuid::Uuid::parse_str(&credential) {
                uuid
            } else {
                // Lookup by name
                // We don't have a get_credential_by_name yet, so we list and find
                let creds = state.db.list_credentials(pid).await?;
                let cred_id = creds
                    .into_iter()
                    .find(|c| c.name == credential)
                    .map(|c| c.id)
                    .ok_or_else(|| anyhow::anyhow!("Credential not found: {}", credential))?;
                cred_id
            };

            // Parse policy IDs
            let mut p_ids = Vec::new();
            if let Some(ids) = policy_ids {
                for id_str in ids {
                    p_ids.push(
                        uuid::Uuid::parse_str(&id_str)
                            .context(format!("Invalid policy ID: {}", id_str))?,
                    );
                }
            }

            let token_id = format!("tf_v1_{}", uuid::Uuid::new_v4().simple());

            let new_token = crate::store::postgres::NewToken {
                id: token_id.clone(),
                project_id: pid,
                name,
                credential_id: Some(cred_id),
                upstream_url: upstream,
                scopes: serde_json::json!([]),
                policy_ids: p_ids,
                log_level: Some(1),    // Default to redacted logging for CLI
                circuit_breaker: None, // Use gateway defaults
                allowed_models: None,
                allowed_providers: None,
                team_id: None,
                tags: None,
                mcp_allowed_tools: None,
                mcp_blocked_tools: None,
                external_user_id: None,
                metadata: None,
                purpose: "llm".to_string(), // Default to LLM for CLI tokens
            };

            state.db.insert_token(&new_token).await?;
            println!(
                "Token created:\n  ID: {}\n  Use:   Authorization: Bearer {}",
                new_token.id, new_token.id
            );
        }
        cli::TokenCommands::List { project_id } => {
            let pid = uuid::Uuid::parse_str(&project_id).context("Invalid project_id")?;
            let tokens = state.db.list_tokens(pid, 1000, 0).await?;
            if tokens.is_empty() {
                println!("No tokens found.");
            } else {
                println!("{:<38} {:<20} {:<10}", "ID", "NAME", "ACTIVE");
                for t in tokens {
                    println!("{:<38} {:<20} {:<10}", t.id, t.name, t.is_active);
                }
            }
        }
        cli::TokenCommands::Revoke { token_id } => {
            // Look up token to get its project_id for the scoped revoke query
            let token_row = state
                .db
                .get_token(&token_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Token not found: {}", token_id))?;
            let revoked = state
                .db
                .revoke_token(&token_id, token_row.project_id)
                .await?;
            if revoked {
                println!("Token revoked.");
            } else {
                println!("Token not found.");
            }
        }
    }
    Ok(())
}

async fn handle_credential_command(
    db: &PgStore,
    cfg: &config::Config,
    cmd: cli::CredentialCommands,
) -> anyhow::Result<()> {
    match cmd {
        cli::CredentialCommands::Add {
            name,
            provider,
            key,
            project_id,
            mode,
            header,
        } => {
            let project = parse_project_id(project_id)?;

            // Validate injection configuration (H1)
            let valid_modes = ["bearer", "basic", "header", "query"];
            if !valid_modes.contains(&mode.as_str()) {
                anyhow::bail!("invalid mode: {}. Must be one of {:?}", mode, valid_modes);
            }

            // Block dangerous headers
            let dangerous_headers = [
                "host",
                "content-length",
                "transfer-encoding",
                "connection",
                "upgrade",
            ];
            if dangerous_headers.contains(&header.to_lowercase().as_str()) {
                anyhow::bail!(
                    "header '{}' is reserved and cannot be used for injection",
                    header
                );
            }

            let (encrypted_dek, dek_nonce, encrypted_secret, secret_nonce) =
                encrypt_credential(&cfg.master_key, &key)?;

            let cred = store::postgres::NewCredential {
                project_id: project,
                name: name.clone(),
                provider: provider.clone(),
                encrypted_dek,
                dek_nonce,
                encrypted_secret,
                secret_nonce,
                injection_mode: mode.clone(),
                injection_header: header.clone(),
            };

            let id = db.insert_credential(&cred).await?;
            println!("Credential stored:");
            println!("  Name:     {}", name);
            println!("  Provider: {}", provider);
            println!("  Mode:     {}", mode);
            println!("  Header:   {}", header);
            println!("  ID:       {}", id);
        }

        cli::CredentialCommands::List { project_id } => {
            let project = parse_project_id(Some(project_id))?;
            let creds = db.list_credentials(project).await?;

            if creds.is_empty() {
                println!("No credentials found.");
                return Ok(());
            }

            println!(
                "{:<38} {:<20} {:<12} {:<8} CREATED",
                "ID", "NAME", "PROVIDER", "ACTIVE"
            );
            for c in creds {
                println!(
                    "{:<38} {:<20} {:<12} {:<8} {}",
                    c.id,
                    c.name,
                    c.provider,
                    c.is_active,
                    c.created_at.format("%Y-%m-%d")
                );
            }
        }
    }
    Ok(())
}

async fn handle_approval_command(db: &PgStore, cmd: cli::ApprovalCommands) -> anyhow::Result<()> {
    match cmd {
        cli::ApprovalCommands::List { project_id } => {
            let project = parse_project_id(project_id)?;
            let approvals = db.list_pending_approvals(project, 1000, 0).await?;

            if approvals.is_empty() {
                println!("No pending approvals.");
                return Ok(());
            }

            println!("{:<38} {:<30} EXPIRES", "ID", "SUMMARY");
            for r in approvals {
                // Truncate summary for display
                let summary = r.request_summary.to_string();
                let summary_display = if summary.len() > 30 {
                    format!("{}...", &summary[..27])
                } else {
                    summary
                };
                println!("{:<38} {:<30} {}", r.id, summary_display, r.expires_at);
            }
        }
        cli::ApprovalCommands::Approve { request_id, project_id } => {
            let id = uuid::Uuid::parse_str(&request_id)?;
            let project = parse_project_id(project_id)?;

            let ok = db
                .update_approval_status(id, project, models::approval::ApprovalStatus::Approved)
                .await?;
            if ok {
                println!("Request {} approved.", id);
            } else {
                println!("Request {} not found or not pending.", id);
            }
        }
        cli::ApprovalCommands::Reject { request_id, project_id } => {
            let id = uuid::Uuid::parse_str(&request_id)?;
            let project = parse_project_id(project_id)?;

            let ok = db
                .update_approval_status(id, project, models::approval::ApprovalStatus::Rejected)
                .await?;
            if ok {
                println!("Request {} rejected.", id);
            } else {
                println!("Request {} not found or not pending.", id);
            }
        }
    }
    Ok(())
}

fn encrypt_credential(
    master_key_hex: &str,
    plaintext: &str,
) -> anyhow::Result<crate::vault::builtin::EncryptedBlob> {
    let crypto = vault::builtin::VaultCrypto::new(master_key_hex)?;
    crypto.encrypt_string(plaintext)
}

fn parse_project_id(id: Option<String>) -> anyhow::Result<uuid::Uuid> {
    let raw = id
        .or_else(|| std::env::var("TRUEFLOW_PROJECT_ID").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "missing --project-id argument (or set TRUEFLOW_PROJECT_ID env var)"
            )
        })?;
    raw.parse()
        .map_err(|_| anyhow::anyhow!("invalid project ID: {}", raw))
}

// ── Config (IaC) command handler ─────────────────────────────

async fn handle_config_command(cmd: cli::ConfigCommands) -> anyhow::Result<()> {
    match cmd {
        cli::ConfigCommands::Export {
            file,
            gateway_url,
            api_key,
            project_id,
        } => {
            let client = iac::client::ApiClient::new(&gateway_url, &api_key, project_id)?;
            let doc = client.export().await?;
            let yaml = doc.to_yaml()?;

            if let Some(path) = file {
                std::fs::write(&path, &yaml)?;
                println!("Exported config to {}", path);
            } else {
                print!("{}", yaml);
            }
            Ok(())
        }

        cli::ConfigCommands::Plan {
            file,
            gateway_url,
            api_key,
            project_id,
        } => {
            let local = iac::schema::ConfigDoc::from_file(std::path::Path::new(&file))?;
            let client = iac::client::ApiClient::new(&gateway_url, &api_key, project_id)?;
            let live = client.export().await?;
            let plan = iac::diff::compute_plan(&local, &live);

            println!("\n{}", plan);

            if plan.is_empty() {
                std::process::exit(0);
            } else {
                // Exit 2 = changes detected (useful for CI)
                std::process::exit(2);
            }
        }

        cli::ConfigCommands::Apply {
            file,
            gateway_url,
            api_key,
            project_id,
            yes,
        } => {
            let local = iac::schema::ConfigDoc::from_file(std::path::Path::new(&file))?;
            let client = iac::client::ApiClient::new(&gateway_url, &api_key, project_id)?;
            let live = client.export().await?;
            let plan = iac::diff::compute_plan(&local, &live);

            if plan.is_empty() {
                println!("No changes. Live state matches the config file.");
                return Ok(());
            }

            println!("\n{}", plan);

            if !yes {
                eprint!("Apply these changes? [y/N] ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            // 1. Apply policies + tokens via config import
            let result = client.import_config(&local).await?;
            println!(
                "Policies: {} created, {} updated",
                result.policies_created, result.policies_updated
            );
            println!(
                "Tokens:   {} created, {} updated",
                result.tokens_created, result.tokens_updated
            );

            // 2. Apply spend caps (not covered by the v1 import API)
            // Re-fetch live state to get token IDs for newly created tokens
            let refreshed = client.export().await?;

            // Build a name→live_token map from refreshed state
            // We need to find token IDs via the list endpoint
            let mut spend_cap_changes = 0;
            for local_token in &local.tokens {
                if local_token.spend_caps.is_empty() {
                    continue;
                }

                // Find the token's live spend caps from the refreshed export
                let live_token = refreshed
                    .tokens
                    .iter()
                    .find(|t| t.name == local_token.name);

                let live_caps = live_token
                    .map(|t| &t.spend_caps)
                    .cloned()
                    .unwrap_or_default();

                for (period, &limit) in &local_token.spend_caps {
                    let needs_update = match live_caps.get(period) {
                        Some(&live_limit) => (limit - live_limit).abs() > 0.001,
                        None => true,
                    };

                    if needs_update {
                        // We need the token ID — get it from the API
                        // The refreshed export doesn't have IDs, so we need to use
                        // a workaround: the spend cap API takes the token ID, not name.
                        // We'll call the list_tokens internal method via a fresh export.
                        // For now, use the import result + re-export approach:
                        // Actually — let's just call the spend cap endpoint.
                        // The client needs token IDs. Let's get them.
                        if spend_cap_changes == 0 {
                            // Lazy-init: only fetch token list if we actually need it
                        }

                        // Use the client to set the spend cap by resolving name → ID
                        // via the tokens list API
                        let token_id =
                            client.find_token_id(&local_token.name).await?;
                        client
                            .upsert_spend_cap(&token_id, period, limit)
                            .await?;
                        spend_cap_changes += 1;
                    }
                }
            }

            if spend_cap_changes > 0 {
                println!("Spend caps: {} updated", spend_cap_changes);
            }

            println!("\nApply complete.");
            Ok(())
        }
    }
}
