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

// ── Scope Checking ─────────────────────────────────────────────

/// SEC-14: Check if a scope is granted by the user's scopes, supporting wildcards.
/// - "*" grants all access
/// - "resource:*" grants all actions on a resource
/// - Exact match required otherwise
fn check_scope_with_wildcards(granted_scopes: &[String], required_scope: &str) -> bool {
    // Wildcard scope grants all access
    if granted_scopes.iter().any(|s| s == "*") {
        return true;
    }

    // Direct match
    if granted_scopes.iter().any(|s| s == required_scope) {
        return true;
    }

    // Resource wildcard (e.g., "tokens:*" matches "tokens:write")
    let parts: Vec<&str> = required_scope.split(':').collect();
    if parts.len() == 2 {
        let resource_wildcard = format!("{}:*", parts[0]);
        if granted_scopes.iter().any(|s| s == &resource_wildcard) {
            return true;
        }
    }

    false
}

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
    /// Returns the default project ID for the current organization context.
    /// CRIT-1 FIX: Uses org-specific default instead of global shared UUID.
    ///
    /// This generates a deterministic UUID from the org_id, ensuring each org
    /// gets its own default project rather than sharing a global one.
    ///
    /// For new deployments, we recommend requiring explicit project_id in all
    /// API requests. Set TRUEFLOW_REQUIRE_EXPLICIT_PROJECT=1 to enforce this.
    pub fn default_project_id(&self) -> Uuid {
        // Check if explicit project is required
        if std::env::var("TRUEFLOW_REQUIRE_EXPLICIT_PROJECT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            tracing::error!(
                org_id = %self.org_id,
                "CRIT-1: TRUEFLOW_REQUIRE_EXPLICIT_PROJECT is set but no project_id provided in request"
            );
            // Return a nil UUID to indicate error - callers should check for this
            // and return a proper error to the client
            return Uuid::nil();
        }

        // CRIT-1 FIX: Return the org_id as the default project ID.
        // In this design, each org has a default project with the same ID as the org.
        // This prevents cross-tenant data access while maintaining backward compatibility.
        self.org_id
    }

    /// Check if the context has the required scope (or is SuperAdmin/Admin).
    /// SEC-14: Supports wildcard scopes:
    /// - "*" grants all access
    /// - "resource:*" grants all actions on a resource (e.g., "tokens:*" matches "tokens:read", "tokens:write")
    /// - Exact match (e.g., "tokens:read" only matches "tokens:read")
    pub fn has_scope(&self, scope: &str) -> bool {
        match self.role {
            ApiKeyRole::SuperAdmin | ApiKeyRole::Admin => true,
            _ => check_scope_with_wildcards(&self.scopes, scope),
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
            // SEC-15: Return FORBIDDEN for unknown roles instead of INTERNAL_SERVER_ERROR
            // Unknown roles are a client/access issue, not a server error
            unknown => {
                tracing::warn!(
                    requested_role = unknown,
                    "SEC-15: Unknown role string requested, returning FORBIDDEN"
                );
                Err(StatusCode::FORBIDDEN)
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
        // Bulk token operations (SaaS builder support)
        .route("/tokens/bulk", post(handlers::bulk_create_tokens))
        .route("/tokens/bulk-revoke", post(handlers::bulk_revoke_tokens))
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
        .route("/auth/keys/:id", delete(handlers::revoke_api_key).put(handlers::update_api_key))
        .route("/auth/whoami", get(handlers::whoami))
        // User management (Supabase Auth sync)
        .route("/auth/sync-user", post(handlers::sync_user))
        .route("/users", get(handlers::list_users))
        .route("/users/me", get(handlers::get_current_user))  // Must be before /users/:id
        .route("/users/:id", get(handlers::get_user))
        .route("/users/:id/role", patch(handlers::update_user_role))
        .route("/users/me/last-project", put(handlers::update_last_project))
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
        // New analytics endpoints for dashboard
        .route("/analytics/models", get(analytics::get_model_usage))
        .route(
            "/analytics/spend/provider",
            get(analytics::get_spend_by_provider),
        )
        .route(
            "/analytics/latency/provider",
            get(analytics::get_latency_by_provider),
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
        // User-level spend analytics (SaaS builder support)
        .route("/analytics/users", get(handlers::get_user_spend))
        // Traffic analytics (Traffic Tab)
        .route(
            "/analytics/traffic/timeseries",
            get(handlers::get_traffic_timeseries),
        )
        .route(
            "/analytics/latency/timeseries",
            get(handlers::get_latency_timeseries),
        )
        // Cost analytics (Cost Tab)
        .route("/analytics/budget-health", get(handlers::get_budget_health))
        .route(
            "/analytics/spend/timeseries",
            get(handlers::get_spend_timeseries),
        )
        .route(
            "/analytics/cost-efficiency",
            get(handlers::get_cost_efficiency),
        )
        .route("/analytics/burn-rate", get(handlers::get_budget_burn_rate))
        .route("/analytics/token-spend", get(handlers::get_token_spend))
        // Users & Tokens analytics (Users & Tokens Tab)
        .route("/analytics/users/growth", get(handlers::get_user_growth))
        .route(
            "/analytics/users/engagement",
            get(handlers::get_user_engagement),
        )
        .route("/analytics/tokens/alerts", get(handlers::get_token_alerts))
        .route(
            "/analytics/users/requests",
            get(handlers::get_requests_per_user),
        )
        // Cache analytics (Cache Tab)
        .route("/analytics/cache/summary", get(handlers::get_cache_summary))
        .route(
            "/analytics/cache/hit-rate-timeseries",
            get(handlers::get_cache_hit_rate_timeseries),
        )
        .route(
            "/analytics/cache/top-queries",
            get(handlers::get_top_cached_queries),
        )
        .route(
            "/analytics/cache/model-efficiency",
            get(handlers::get_model_cache_efficiency),
        )
        .route(
            "/analytics/cache/latency-comparison",
            get(handlers::get_cache_latency_comparison),
        )
        // Model analytics (Models Tab)
        .route(
            "/analytics/models/usage-timeseries",
            get(handlers::get_model_usage_timeseries),
        )
        .route(
            "/analytics/models/error-rates",
            get(handlers::get_model_error_rates),
        )
        .route(
            "/analytics/models/latency",
            get(handlers::get_model_latency),
        )
        .route(
            "/analytics/models/stats",
            get(handlers::get_model_stats),
        )
        .route(
            "/analytics/models/cost-latency-scatter",
            get(handlers::get_cost_latency_scatter),
        )
        // Security analytics (Security Tab)
        .route(
            "/analytics/security/summary",
            get(handlers::get_security_summary),
        )
        .route(
            "/analytics/security/guardrail-triggers",
            get(handlers::get_guardrail_triggers),
        )
        .route(
            "/analytics/security/pii-breakdown",
            get(handlers::get_pii_breakdown),
        )
        .route(
            "/analytics/security/policy-actions",
            get(handlers::get_policy_actions),
        )
        .route(
            "/analytics/security/shadow-policies",
            get(handlers::get_shadow_policies),
        )
        .route(
            "/analytics/security/data-residency",
            get(handlers::get_data_residency),
        )
        // HITL analytics (HITL Tab)
        .route("/analytics/hitl/summary", get(handlers::get_hitl_summary))
        .route("/analytics/hitl/volume", get(handlers::get_hitl_volume))
        .route("/analytics/hitl/latency", get(handlers::get_hitl_latency))
        .route(
            "/analytics/hitl/reasons",
            get(handlers::get_hitl_rejection_reasons),
        )
        // Error analytics (Errors Tab)
        .route("/analytics/errors/summary", get(handlers::get_error_summary))
        .route("/analytics/errors/timeseries", get(handlers::get_error_timeseries))
        .route("/analytics/errors/breakdown", get(handlers::get_error_breakdown))
        .route("/analytics/errors/logs", get(handlers::get_error_logs))
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
        .route("/mcp/servers/:id", get(mcp_handlers::get_mcp_server).delete(mcp_handlers::delete_mcp_server))
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
            "/experiments/:id/timeseries",
            get(experiment_handlers::get_experiment_timeseries),
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

    // SEC-06: Reject empty or whitespace-only admin key
    let empty_key = expected_env_key.trim().is_empty();
    if empty_key {
        tracing::error!("SEC-06: TRUEFLOW_ADMIN_KEY is empty or whitespace-only - rejecting all SuperAdmin access");
    }

    // SEC-08: Refuse insecure default key in non-dev environments
    let insecure_default = expected_env_key == "CHANGE_ME_INSECURE_DEFAULT";

    // MED-4: Warn if admin key has weak entropy (less than 32 chars)
    // This is a security best practice - short keys are easier to brute force
    let key_len = expected_env_key.trim().len();
    if !empty_key && !insecure_default && key_len < 32 {
        tracing::warn!(
            key_len = key_len,
            "MED-4: TRUEFLOW_ADMIN_KEY is shorter than recommended (32 chars). \
             Consider using a longer key for better security. \
             Generate with: openssl rand -hex 32"
        );
    }

    // Block SuperAdmin access if key is empty or insecure
    let superadmin_disabled = empty_key || insecure_default;

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
        if !superadmin_disabled && ct_eq(k, &expected_env_key) {
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
        if !superadmin_disabled && ct_eq(k, &expected_env_key) {
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
                                // SEC-01: Map OIDC role string → ApiKeyRole
                                // IMPORTANT: OIDC can NEVER grant SuperAdmin role.
                                // SuperAdmin is only available via environment admin key.
                                // Any "superadmin" claim from OIDC is capped at Admin.
                                let role = match auth_result.role.as_str() {
                                    "superadmin" => {
                                        tracing::warn!(
                                            user = %auth_result.user_id,
                                            provider = %provider.name,
                                            "SEC-01: OIDC attempted to grant SuperAdmin, capping at Admin"
                                        );
                                        ApiKeyRole::Admin // Cap at Admin - SuperAdmin not allowed via OIDC
                                    }
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
