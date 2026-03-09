use crate::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub mod analytics;
pub mod config;
pub mod experiment_handlers;
pub mod guardrail_presets;
pub mod handlers;
pub mod mcp_handlers;
pub mod prompt_handlers;

// ── Auth Context ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ApiKeyRole {
    SuperAdmin, // Env var key: full access to everything
    Admin,      // API key: full access within org
    Member,     // API key: read/write resources, no delete/revoke
    ReadOnly,   // API key: read-only
}

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub role: ApiKeyRole,
    pub scopes: Vec<String>,
    #[allow(dead_code)]
    pub key_id: Option<Uuid>,
}

impl AuthContext {
    /// Returns the default project ID for the current context.
    /// For MVP, this is still the hardcoded default Project ID until meaningful project switching is added.
    pub fn default_project_id(&self) -> Uuid {
        // In the future, this could be user.default_project_id or similar.
        // For now, we stick to the known default.
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }

    /// Check if the context has the required scope (or is SuperAdmin/Admin).
    pub fn has_scope(&self, scope: &str) -> bool {
        match self.role {
            ApiKeyRole::SuperAdmin | ApiKeyRole::Admin => true,
            _ => self.scopes.iter().any(|s| s == scope),
        }
    }

    pub fn require_scope(&self, scope: &str) -> Result<(), StatusCode> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }

    pub fn require_role(&self, role: &str) -> Result<(), StatusCode> {
        match role {
            "admin" => match self.role {
                ApiKeyRole::SuperAdmin | ApiKeyRole::Admin => Ok(()),
                _ => Err(StatusCode::FORBIDDEN),
            },
            "superadmin" => match self.role {
                ApiKeyRole::SuperAdmin => Ok(()),
                _ => Err(StatusCode::FORBIDDEN),
            },
            _ => {
                tracing::error!("Unknown role required: {}", role);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// Build the Management API router.
/// All routes are relative — the caller mounts this under `/api/v1`.
pub fn api_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/tokens",
            get(handlers::list_tokens).post(handlers::create_token),
        )
        .route("/tokens/:id", delete(handlers::revoke_token))
        .route("/tokens/:id/usage", get(handlers::get_token_usage))
        .route(
            "/tokens/:id/circuit-breaker",
            get(handlers::get_circuit_breaker).patch(handlers::update_circuit_breaker),
        )
        .route(
            "/policies",
            get(handlers::list_policies).post(handlers::create_policy),
        )
        .route(
            "/policies/:id",
            put(handlers::update_policy).delete(handlers::delete_policy),
        )
        .route(
            "/policies/:id/versions",
            get(handlers::list_policy_versions),
        )
        .route(
            "/credentials",
            get(handlers::list_credentials).post(handlers::create_credential),
        )
        .route("/credentials/:id", delete(handlers::delete_credential))
        .route(
            "/projects",
            get(handlers::list_projects).post(handlers::create_project),
        )
        .route(
            "/projects/:id",
            put(handlers::update_project).delete(handlers::delete_project),
        )
        .route(
            "/projects/:id/purge",
            // GDPR Article 17 — Right to Erasure: purges all project data (audit logs, sessions, usage)
            post(handlers::purge_project_data),
        )
        .route(
            "/approvals", // HITL requests
            get(handlers::list_approvals),
        )
        .route("/approvals/:id/decision", post(handlers::decide_approval))
        .route("/audit", get(handlers::list_audit_logs))
        .route("/audit/:id", get(handlers::get_audit_log))
        .route("/audit/stream", get(handlers::stream_audit_logs))
        .route("/sessions", get(handlers::list_sessions))
        .route("/sessions/:id", get(handlers::get_session))
        // Session Lifecycle
        .route(
            "/sessions/:id/status",
            patch(handlers::update_session_status),
        )
        .route(
            "/sessions/:id/spend-cap",
            put(handlers::set_session_spend_cap),
        )
        .route("/sessions/:id/entity", get(handlers::get_session_entity))
        // Services
        .route(
            "/services",
            get(handlers::list_services).post(handlers::create_service),
        )
        .route("/services/:id", delete(handlers::delete_service))
        // Notifications
        .route("/notifications", get(handlers::list_notifications))
        .route(
            "/notifications/unread",
            get(handlers::count_unread_notifications),
        )
        //.route("/notifications/:id/read", post(handlers::mark_notification_read)) // Using post for side-effect?
        // Wait, mark_read handler is POST
        .route(
            "/notifications/:id/read",
            post(handlers::mark_notification_read),
        )
        .route(
            "/notifications/read-all",
            post(handlers::mark_all_notifications_read),
        )
        // Key management (New)
        .route(
            "/auth/keys",
            get(handlers::list_api_keys).post(handlers::create_api_key),
        )
        .route("/auth/keys/:id", delete(handlers::revoke_api_key))
        .route("/auth/whoami", get(handlers::whoami))
        // Billing (New)
        .route("/billing/usage", get(handlers::get_org_usage))
        // Analytics (New)
        .route("/analytics/tokens", get(handlers::get_token_analytics))
        .route(
            "/analytics/tokens/:id/volume",
            get(handlers::get_token_volume),
        )
        .route(
            "/analytics/tokens/:id/status",
            get(handlers::get_token_status),
        )
        .route(
            "/analytics/tokens/:id/latency",
            get(handlers::get_token_latency),
        )
        .route("/analytics/volume", get(analytics::get_request_volume))
        .route("/analytics/status", get(analytics::get_status_distribution))
        .route(
            "/analytics/latency",
            get(analytics::get_latency_percentiles),
        )
        // New Server-Side Analytics (Phase 8)
        .route("/analytics/summary", get(handlers::get_analytics_summary))
        .route(
            "/analytics/timeseries",
            get(handlers::get_analytics_timeseries),
        )
        .route(
            "/analytics/experiments",
            get(handlers::get_analytics_experiments),
        )
        .route(
            "/analytics/spend/breakdown",
            get(handlers::get_spend_breakdown),
        )
        // Settings & System
        .route(
            "/settings",
            get(handlers::get_settings).put(handlers::update_settings),
        )
        .route("/system/cache-stats", get(handlers::get_cache_stats))
        .route("/system/flush-cache", post(handlers::flush_cache))
        // PII Tokenization Vault
        .route("/pii/rehydrate", post(handlers::rehydrate_pii_tokens))
        // Upstream Health
        .route("/health/upstreams", get(handlers::get_upstream_health))
        // Anomaly Detection
        .route("/anomalies", get(handlers::get_anomaly_events))
        // Model Access Groups (RBAC Depth)
        .route(
            "/model-access-groups",
            get(handlers::list_model_access_groups).post(handlers::create_model_access_group),
        )
        .route(
            "/model-access-groups/:id",
            put(handlers::update_model_access_group).delete(handlers::delete_model_access_group),
        )
        // Teams (Org Hierarchy)
        .route(
            "/teams",
            get(handlers::list_teams).post(handlers::create_team),
        )
        .route(
            "/teams/:id",
            put(handlers::update_team).delete(handlers::delete_team),
        )
        .route(
            "/teams/:id/members",
            get(handlers::list_team_members).post(handlers::add_team_member),
        )
        .route(
            "/teams/:id/members/:user_id",
            delete(handlers::remove_team_member),
        )
        .route("/teams/:id/spend", get(handlers::get_team_spend))
        // Spend Caps
        .route(
            "/tokens/:id/spend",
            get(handlers::get_spend_caps).put(handlers::upsert_spend_cap),
        )
        .route(
            "/tokens/:id/spend/:period",
            delete(handlers::delete_spend_cap),
        )
        // Webhooks
        .route(
            "/webhooks",
            get(handlers::list_webhooks).post(handlers::create_webhook),
        )
        .route("/webhooks/:id", delete(handlers::delete_webhook))
        .route("/webhooks/test", post(handlers::test_webhook))
        // Model Pricing
        .route(
            "/pricing",
            get(handlers::list_pricing).put(handlers::upsert_pricing),
        )
        .route("/pricing/:id", delete(handlers::delete_pricing))
        // Guardrail Presets — one-call guardrail enablement
        .route("/guardrails/presets", get(guardrail_presets::list_presets))
        .route(
            "/guardrails/enable",
            post(guardrail_presets::enable_guardrails),
        )
        .route(
            "/guardrails/disable",
            delete(guardrail_presets::disable_guardrails),
        )
        .route(
            "/guardrails/status",
            get(guardrail_presets::guardrails_status),
        )
        // Config-as-Code — export/import policies+tokens as YAML or JSON
        .route("/config/export", get(config::export_config))
        .route("/config/export/policies", get(config::export_policies))
        .route("/config/export/tokens", get(config::export_tokens))
        .route("/config/import", post(config::import_config))
        // MCP Server Management
        .route(
            "/mcp/servers",
            get(mcp_handlers::list_mcp_servers).post(mcp_handlers::register_mcp_server),
        )
        .route("/mcp/servers/test", post(mcp_handlers::test_mcp_server))
        .route(
            "/mcp/servers/discover",
            post(mcp_handlers::discover_mcp_server),
        )
        .route("/mcp/servers/:id", delete(mcp_handlers::delete_mcp_server))
        .route(
            "/mcp/servers/:id/refresh",
            post(mcp_handlers::refresh_mcp_server),
        )
        .route(
            "/mcp/servers/:id/tools",
            get(mcp_handlers::list_mcp_server_tools),
        )
        .route(
            "/mcp/servers/:id/reauth",
            post(mcp_handlers::reauth_mcp_server),
        )
        // Prompt Management
        .route(
            "/prompts",
            get(prompt_handlers::list_prompts).post(prompt_handlers::create_prompt),
        )
        .route("/prompts/folders", get(prompt_handlers::list_folders))
        .route(
            "/prompts/:id",
            get(prompt_handlers::get_prompt)
                .put(prompt_handlers::update_prompt)
                .delete(prompt_handlers::delete_prompt),
        )
        .route(
            "/prompts/:id/versions",
            get(prompt_handlers::list_versions).post(prompt_handlers::create_version),
        )
        .route(
            "/prompts/:id/versions/:version",
            get(prompt_handlers::get_version),
        )
        .route("/prompts/:id/deploy", post(prompt_handlers::deploy_version))
        .route(
            "/prompts/by-slug/:slug/render",
            get(prompt_handlers::render_prompt_get).post(prompt_handlers::render_prompt_post),
        )
        // Experiment Management (A/B Testing)
        .route(
            "/experiments",
            get(experiment_handlers::list_experiments).post(experiment_handlers::create_experiment),
        )
        .route(
            "/experiments/:id",
            get(experiment_handlers::get_experiment).put(experiment_handlers::update_experiment),
        )
        .route(
            "/experiments/:id/results",
            get(experiment_handlers::get_experiment_results),
        )
        .route(
            "/experiments/:id/stop",
            post(experiment_handlers::stop_experiment),
        )
        .layer(middleware::from_fn_with_state(state, admin_auth))
        .layer(TraceLayer::new_for_http())
        .fallback(fallback_404)
}

async fn fallback_404() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Middleware: validates `X-Admin-Key` (SuperAdmin) or `Authorization: Bearer <api_key>` (RBAC).
async fn admin_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Check for Env Key (SuperAdmin)
    // We check this first to allow emergency access even if DB is down.
    let provided_key_header = req
        .headers()
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok());

    // We also support the env key in the Authorization header for convenience
    let bearer_token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim());

    // Load expected SuperAdmin key
    let expected_env_key = std::env::var("TRUEFLOW_ADMIN_KEY")
        .or_else(|_| std::env::var("TRUEFLOW_MASTER_KEY"))
        .unwrap_or_else(|_| "CHANGE_ME_INSECURE_DEFAULT".to_string());

    // SEC-08: Refuse insecure default key in non-dev environments
    let insecure_default = expected_env_key == "CHANGE_ME_INSECURE_DEFAULT";

    // SEC-07: constant-time comparison for admin key
    // Uses SHA-256 to normalize both values to fixed length before comparison,
    // preventing timing side-channel from leaking key length.
    fn ct_eq(a: &str, b: &str) -> bool {
        use sha2::{Digest, Sha256};
        use subtle::ConstantTimeEq;
        let hash_a = Sha256::digest(a.as_bytes());
        let hash_b = Sha256::digest(b.as_bytes());
        hash_a.ct_eq(&hash_b).into()
    }

    // ── Path A: SuperAdmin (Env Key) ─────────────────────────────
    if let Some(k) = provided_key_header {
        if !insecure_default && ct_eq(k, &expected_env_key) {
            let ctx = AuthContext {
                org_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(), // Default Org
                user_id: None,
                role: ApiKeyRole::SuperAdmin,
                scopes: vec![],
                key_id: None,
            };
            req.extensions_mut().insert(ctx);
            return Ok(next.run(req).await);
        }
    }

    if let Some(k) = bearer_token {
        if !insecure_default && ct_eq(k, &expected_env_key) {
            let ctx = AuthContext {
                org_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                user_id: None,
                role: ApiKeyRole::SuperAdmin,
                scopes: vec![],
                key_id: None,
            };
            req.extensions_mut().insert(ctx);
            return Ok(next.run(req).await);
        }
        // ── Path C: OIDC JWT (if token looks like a JWT) ──────────
        //
        // JWTs have 3 dot-separated base64 parts. API keys start with "ak_".
        // If it looks like a JWT, try OIDC with full JWKS crypto verification.
        // If that fails, fall through to the API key path (graceful degradation).
        if k.split('.').count() == 3 && !k.starts_with("ak_") {
            use crate::middleware::oidc;

            // Step 1: Peek at the JWT payload to extract `iss` (unverified)
            //         so we can look up the OIDC provider in the DB.
            let iss_peek = {
                let parts: Vec<&str> = k.split('.').collect();
                use base64::Engine;
                let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
                engine
                    .decode(parts[1])
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .and_then(|v| v.get("iss").and_then(|i| i.as_str()).map(String::from))
            };

            if let Some(issuer) = iss_peek {
                // Step 2: Look up provider by issuer
                match state.db.get_oidc_provider_by_issuer(&issuer).await {
                    Ok(Some(provider_row)) => {
                        // Convert DB row → OidcProvider
                        let provider = oidc::OidcProvider {
                            id: provider_row.id,
                            org_id: provider_row.org_id,
                            name: provider_row.name,
                            issuer_url: provider_row.issuer_url,
                            client_id: provider_row.client_id,
                            jwks_uri: provider_row.jwks_uri,
                            audience: provider_row.audience,
                            claim_mapping: provider_row.claim_mapping,
                            default_role: provider_row.default_role,
                            default_scopes: provider_row.default_scopes,
                            enabled: provider_row.enabled,
                        };

                        // Step 3: Full crypto-verified JWT validation
                        //         (JWKS fetch → signature verify → claims extract → RBAC map)
                        match oidc::validate_jwt(k, &provider).await {
                            Ok(auth_result) => {
                                // Map OIDC role string → ApiKeyRole
                                let role = match auth_result.role.as_str() {
                                    "superadmin" => ApiKeyRole::SuperAdmin,
                                    "admin" => ApiKeyRole::Admin,
                                    "member" => ApiKeyRole::Member,
                                    "readonly" => ApiKeyRole::ReadOnly,
                                    _ => ApiKeyRole::ReadOnly, // safe default
                                };

                                let role_str = format!("{:?}", role);
                                tracing::info!(
                                    user = %auth_result.user_id,
                                    provider = %provider.name,
                                    role = %role_str,
                                    "OIDC: JWT authenticated successfully (crypto-verified)"
                                );

                                let ctx = AuthContext {
                                    org_id: auth_result.org_id,
                                    user_id: None, // OIDC users don't have a local UUID yet
                                    role,
                                    scopes: auth_result.scopes,
                                    key_id: None,
                                };

                                req.extensions_mut().insert(ctx);
                                return Ok(next.run(req).await);
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    issuer = %issuer,
                                    "OIDC: JWT crypto verification failed"
                                );
                                return Err(StatusCode::UNAUTHORIZED);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            issuer = %issuer,
                            "OIDC: no provider configured for issuer, falling through to API key"
                        );
                        // Fall through to API key path
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "OIDC: DB error looking up provider, falling through to API key"
                        );
                        // Fall through to API key path
                    }
                }
            } else {
                tracing::debug!(
                    "Bearer token looks like JWT but couldn't extract issuer, trying API key path"
                );
                // Fall through to API key path
            }
        }

        // ── Path B: API Key (DB) ─────────────────────────────────
        // Key format: ak_live_prefix_hex
        // We hash the full key to look it up.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(k.as_bytes());
        let hash = hex::encode(hasher.finalize());

        let key_row = state.db.get_api_key_by_hash(&hash).await.map_err(|e| {
            tracing::error!("DB error looking up API key: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let Some(key) = key_row {
            // Check expiration before anything else
            if let Some(exp) = key.expires_at {
                if exp < chrono::Utc::now() {
                    tracing::warn!(
                        key_prefix = %key.key_prefix,
                        expired_at = %exp,
                        "API key rejected: expired"
                    );
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }

            // Update last used (fire and forget / async)
            let key_id = key.id;
            let db = state.db.clone();
            tokio::spawn(async move {
                if let Err(e) = db.touch_api_key_usage(key_id).await {
                    tracing::warn!("Failed to update API key stats: {}", e);
                }
            });

            // Parse scopes
            let scopes: Vec<String> = serde_json::from_value(key.scopes).unwrap_or_default();

            // Map role string to enum
            let role = match key.role.as_str() {
                "admin" => ApiKeyRole::Admin,
                "member" => ApiKeyRole::Member,
                "readonly" => ApiKeyRole::ReadOnly,
                _ => ApiKeyRole::Member,
            };

            let ctx = AuthContext {
                org_id: key.org_id,
                user_id: key.user_id,
                role,
                scopes,
                key_id: Some(key.id),
            };

            req.extensions_mut().insert(ctx);
            return Ok(next.run(req).await);
        } else {
            // Invalid API key
            let masked = if k.len() > 8 {
                format!("{}…{}", &k[..4], &k[k.len() - 4..])
            } else {
                "****".to_string()
            };
            tracing::warn!("admin API: invalid API key provided: {}", masked);
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    tracing::warn!("admin API: missing auth header");
    Err(StatusCode::UNAUTHORIZED)
}
