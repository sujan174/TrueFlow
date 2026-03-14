use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware;
use crate::middleware::fields::RequestContext;
use crate::middleware::pii::PiiDetector as _;
use crate::models::cost::{self, extract_model, extract_usage};
use crate::models::policy::{Action, RedactDirection, RedactOnMatch, TriggeredAction};
use crate::proxy;
use crate::vault::SecretStore;
use crate::AppState;

use super::audit::base_audit;
use super::headers::{headers_to_json, headers_to_json_reqwest};
use super::security::is_safe_webhook_url;

/// The main handler for all proxied requests.
#[tracing::instrument(skip(state, headers, body), fields(req_id = %uuid::Uuid::new_v4()))]
pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    // Copy agent name header before consuming request
    let agent_name = headers
        .get("X-TrueFlow-Agent-Name")
        .or_else(|| headers.get("user-agent"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Copy idempotency key for HITL
    let idempotency_key = headers
        .get("X-TrueFlow-Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // TEST HOOK: Extract cost/token/latency override headers for integration testing.
    // SEC: Compile-time gated — these headers are STRIPPED from release binaries.
    // Build with `--features test-hooks` to enable. Never use in production.
    #[cfg(feature = "test-hooks")]
    let (test_cost_override, test_tokens_override, test_latency_override) = {
        let cost = headers
            .get("x-trueflow-test-cost")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

        let tokens = headers
            .get("x-trueflow-test-tokens")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(p), Ok(c)) = (
                        parts[0].trim().parse::<u32>(),
                        parts[1].trim().parse::<u32>(),
                    ) {
                        Some((p, c))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

        let latency = headers
            .get("x-trueflow-test-latency")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        (cost, tokens, latency)
    };
    #[cfg(not(feature = "test-hooks"))]
    let (test_cost_override, test_tokens_override, test_latency_override) = (
        None::<rust_decimal::Decimal>,
        None::<(u32, u32)>,
        None::<u64>,
    );

    // ── Phase 4: Attribution headers ──────────────────────────
    let user_id = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let tenant_id = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let external_request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // ── Phase 5: Trace / session headers ─────────────────────
    let session_id = headers
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // ── Phase 6: Custom properties ────────────────────────────
    // X-Properties: {"env":"prod","run_id":"agent-run-42","customer":"acme"}
    // Arbitrary JSON key-values attached to every audit log for this request.
    // Stored as JSONB and GIN-indexed for fast filtering.
    let custom_properties: Option<serde_json::Value> = headers
        .get("x-properties")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok());

    // W3C Trace Context: parse `traceparent` header if present.
    // Format: 00-{trace_id}-{parent_id}-{flags}
    // We prefer traceparent over x-parent-span-id when both are present.
    let (w3c_trace_id, w3c_parent_id) = headers
        .get("traceparent")
        .and_then(|v| v.to_str().ok())
        .and_then(|tp| {
            let parts: Vec<&str> = tp.splitn(4, '-').collect();
            if parts.len() == 4 {
                Some((parts[1].to_string(), parts[2].to_string()))
            } else {
                tracing::debug!(traceparent = %tp, "malformed traceparent header, ignoring");
                None
            }
        })
        .map(|(t, p)| (Some(t), Some(p)))
        .unwrap_or((None, None));

    let parent_span_id = w3c_parent_id.or_else(|| {
        headers
            .get("x-parent-span-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    });

    if let (Some(ref tid), Some(ref pid)) = (&w3c_trace_id, &parent_span_id) {
        tracing::debug!(
            trace_id = %tid,
            parent_span_id = %pid,
            req_id = %request_id,
            "W3C trace context received"
        );
    }

    // Detect streaming request (will be confirmed after body parse)
    let is_streaming_req = crate::models::llm::is_streaming_request(&body);

    // Copy original Content-Type before consuming request
    let original_content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();

    // -- 1. Extract virtual token --
    let token_str = extract_bearer_token(&headers)?;

    // -- 2. Resolve token --
    let token = state
        .db
        .get_token(&token_str)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::TokenNotFound)?;

    if !token.is_active {
        return Err(AppError::TokenNotFound);
    }

    // Check token expiration
    if let Some(exp) = token.expires_at {
        if exp < chrono::Utc::now() {
            tracing::warn!(
                token_id = %token.id,
                expired_at = %exp,
                "proxy: token rejected — expired"
            );
            return Err(AppError::TokenNotFound);
        }
    }

    // -- 2.1 Parse per-token circuit breaker configuration --
    let cb_config: crate::proxy::loadbalancer::CircuitBreakerConfig = token
        .circuit_breaker
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // -- 3. Load policies --
    let path = uri.path().to_string();

    let policies = state
        .db
        .get_policies_for_token(token.project_id, &token.policy_ids)
        .await
        .map_err(AppError::Internal)?;

    // -- 3.1 Parse request body as JSON (for body inspection) --
    let mut parsed_body: Option<serde_json::Value> = if !body.is_empty() {
        serde_json::from_slice(&body).ok()
    } else {
        None
    };

    // -- 3.2 Evaluate PRE-FLIGHT policies --
    // Load usage counters from Redis for condition evaluation
    let usage_counters = {
        let mut counters = std::collections::HashMap::new();
        let mut conn = state.cache.redis();
        let now = chrono::Utc::now();

        let spend_daily_key = format!("spend:{}:daily:{}", token.id, now.format("%Y-%m-%d"));
        let spend_monthly_key = format!("spend:{}:monthly:{}", token.id, now.format("%Y-%m"));

        // Request counting keys
        let req_daily_key = format!("req:{}:daily:{}", token.id, now.format("%Y-%m-%d"));
        let req_hourly_key = format!("req:{}:hourly:{}", token.id, now.format("%Y-%m-%d:%H"));

        // Pipeline:
        // 1. Get Spend (Daily + Monthly)
        // 2. Incr Requests (Daily + Hourly)
        // We use a pipeline to minimize RTT.
        let mut pipe = redis::pipe();
        pipe.get(&spend_daily_key)
            .get(&spend_monthly_key)
            .incr(&req_daily_key, 1)
            .expire(&req_daily_key, 90000)
            .ignore() // Daily + buffer
            .incr(&req_hourly_key, 1)
            .expire(&req_hourly_key, 4000)
            .ignore(); // Hourly + buffer

        let (spend_daily, spend_monthly, req_daily, req_hourly): (
            Option<f64>,
            Option<f64>,
            u64,
            u64,
        ) = pipe
            .query_async(&mut conn)
            .await
            .unwrap_or((None, None, 0, 0));

        if let Some(v) = spend_daily {
            counters.insert("spend_today_usd".to_string(), v);
        }
        if let Some(v) = spend_monthly {
            counters.insert("spend_month_usd".to_string(), v);
        }

        counters.insert("requests_today".to_string(), req_daily as f64);
        counters.insert("requests_this_hour".to_string(), req_hourly as f64);

        counters
    };

    // SEC: Extract client IP from X-Forwarded-For ONLY if trusted proxies are configured.
    // This prevents IP spoofing via malicious X-Forwarded-For headers from untrusted clients.
    // If TRUSTED_PROXY_CIDRS is empty (default), we ignore these headers entirely.
    let client_ip_str = if state.config.trusted_proxy_cidrs.is_empty() {
        // No trusted proxies configured - ignore X-Forwarded-For for security
        None
    } else {
        // Trusted proxies are configured - trust X-Forwarded-For from the edge proxy
        headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
    };

    // Scope the RequestContext borrow so we can mutate parsed_body after evaluation
    let (outcome_actions, shadow_violations, pre_async_triggered) = {
        let ctx = RequestContext {
            method: &method,
            path: &path,
            uri: &uri,
            headers: &headers,
            body: parsed_body.as_ref(),
            body_size: body.len(),
            agent_name: agent_name.as_deref(),
            token_id: &token.id,
            token_name: &token.name,
            project_id: &token.project_id.to_string(),
            client_ip: client_ip_str.as_deref(),
            response_status: None,
            response_body: None,
            response_headers: None,
            usage: usage_counters.clone(),
        };

        let outcome = middleware::policy::evaluate_pre_flight(&policies, &ctx);
        (
            outcome.actions,
            outcome.shadow_violations,
            outcome.async_triggered,
        )
    };
    // ctx is now dropped — parsed_body can be mutated

    // -- X-TrueFlow-Guardrails header opt-in (per-request guardrails, no policy config needed) --
    // Header format:  X-TrueFlow-Guardrails: pii_redaction,prompt_injection
    // Each recognised preset injects synthetic TriggeredAction entries at the front of the queue.
    let mut outcome_actions = outcome_actions;
    if let Some(guardrail_header) = headers
        .get("x-trueflow-guardrails")
        .and_then(|v| v.to_str().ok())
    {
        let mut header_actions: Vec<TriggeredAction> = Vec::new();
        for preset in guardrail_header
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let action = match preset {
                "pii_redaction" => Some(Action::Redact {
                    direction: RedactDirection::Both,
                    patterns: [
                        "ssn",
                        "email",
                        "credit_card",
                        "phone",
                        "api_key",
                        "iban",
                        "dob",
                        "ipv4",
                    ]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                    fields: vec![],
                    on_match: RedactOnMatch::Redact,
                    nlp_backend: None,
                }),
                "pii_block" => Some(Action::Redact {
                    direction: RedactDirection::Request,
                    patterns: ["ssn", "email", "credit_card", "phone"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    fields: vec![],
                    on_match: RedactOnMatch::Block,
                    nlp_backend: None,
                }),
                "prompt_injection" => Some(Action::ContentFilter {
                    block_jailbreak: true,
                    block_harmful: true,
                    block_code_injection: true,
                    block_profanity: false,
                    block_bias: false,
                    block_competitor_mention: false,
                    block_sensitive_topics: false,
                    block_gibberish: false,
                    block_contact_info: false,
                    block_ip_leakage: false,
                    competitor_names: vec![],
                    topic_allowlist: vec![],
                    topic_denylist: vec![],
                    custom_patterns: vec![],
                    risk_threshold: 0.3,
                    max_content_length: 0,
                }),
                other => {
                    tracing::warn!(
                        preset = other,
                        "X-TrueFlow-Guardrails: unrecognised preset, ignoring"
                    );
                    None
                }
            };
            if let Some(a) = action {
                header_actions.push(TriggeredAction {
                    policy_id: uuid::Uuid::nil(),
                    policy_name: format!("header-guardrail:{}", preset),
                    rule_index: 0,
                    action: a,
                });
            }
        }
        // Prepend header actions so they run before any configured policies
        header_actions.extend(outcome_actions);
        outcome_actions = header_actions;
    }

    let mut shadow_violations = shadow_violations;

    // -- 3.3 Execute enforced actions --
    let mut hitl_required = false;
    let mut hitl_decision = None;
    let mut hitl_timeout_str = "30m".to_string();
    let mut hitl_latency_ms = None;
    let mut policy_rate_limited = false;
    let mut header_mutations = middleware::redact::HeaderMutations::default();
    let mut redacted_by_policy: Vec<String> = Vec::new();
    // A/B experiment tracking — set by Split action
    let mut experiment_name: Option<String> = None;
    let mut variant_name: Option<String> = None;
    // DynamicRoute tracking — set by DynamicRoute action
    let mut dynamic_upstream_override: Option<String> = None;
    let mut dynamic_route_strategy: Option<String> = None;
    let mut dynamic_route_reason: Option<String> = None;

    for triggered in &outcome_actions {
        match &triggered.action {
            // ── Allow ──
            Action::Allow => {
                // No-op
            }
            // ── Deny ──
            Action::Deny { status: _, message } => {
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &token.upstream_url,
                    &policies,
                    false,
                    None,
                    None,
                    user_id.clone(),
                    tenant_id.clone(),
                    external_request_id.clone(),
                    session_id.clone(),
                    parent_span_id.clone(),
                    custom_properties.clone(),
                );
                audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
                    policy: triggered.policy_name.clone(),
                    reason: message.clone(),
                });
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.shadow_violations = if shadow_violations.is_empty() {
                    None
                } else {
                    Some(shadow_violations)
                };
                audit.emit(&state);

                // Phase 5: Emit notification + webhook
                let state_clone = state.clone();
                let project_id = token.project_id;
                let title = format!("Policy Violation: {}", triggered.policy_name);
                let body_msg = message.clone();
                let webhook_event = crate::notification::webhook::WebhookEvent::policy_violation(
                    &token.id,
                    &token.name,
                    &token.project_id.to_string(),
                    &triggered.policy_name,
                    message,
                );
                let webhook_urls = state.config.webhook_urls.clone();
                state.webhook.dispatch(&webhook_urls, webhook_event).await;
                tokio::spawn(async move {
                    let _ = state_clone
                        .db
                        .create_notification(
                            project_id,
                            "policy_violation",
                            &title,
                            Some(&body_msg),
                            None,
                        )
                        .await;
                });

                return Err(AppError::PolicyDenied {
                    policy: triggered.policy_name.clone(),
                    reason: message.clone(),
                });
            }

            // ── Rate Limit ──
            Action::RateLimit {
                window,
                max_requests,
                key,
            } => {
                let window_secs = middleware::policy::parse_window_secs(window).unwrap_or(60);
                // SEC: Include policy_id + window in key so each rate_limit policy
                // gets its own independent counter. Without this, two policies on the
                // same token share one counter and the stricter window resets the
                // lenient one — a CLASS A bypass.
                let policy_prefix = format!("rl:{}:{}s", triggered.policy_id, window_secs);
                let rl_key = match key {
                    crate::models::policy::RateLimitKey::PerToken => {
                        format!("{}:tok:{}", policy_prefix, token.id)
                    }
                    crate::models::policy::RateLimitKey::PerAgent => {
                        format!(
                            "{}:agent:{}",
                            policy_prefix,
                            agent_name.as_deref().unwrap_or("unknown")
                        )
                    }
                    crate::models::policy::RateLimitKey::PerIp => format!(
                        "{}:ip:{}",
                        policy_prefix,
                        client_ip_str.as_deref().unwrap_or("unknown")
                    ),
                    crate::models::policy::RateLimitKey::PerUser => format!(
                        "{}:user:{}",
                        policy_prefix,
                        user_id.as_deref().unwrap_or(&token.id)
                    ),
                    crate::models::policy::RateLimitKey::Global => {
                        format!("{}:global", policy_prefix)
                    }
                };
                // Use sliding window to prevent 2x burst at window boundaries
                let count = state
                    .cache
                    .increment_sliding_window(&rl_key, window_secs)
                    .await
                    .map_err(AppError::Internal)?;

                if count > *max_requests {
                    let mut audit = base_audit(
                        request_id,
                        token.project_id,
                        &token.id,
                        agent_name,
                        method.as_str(),
                        &path,
                        &token.upstream_url,
                        &policies,
                        false,
                        None,
                        None,
                        user_id.clone(),
                        tenant_id.clone(),
                        external_request_id.clone(),
                        session_id.clone(),
                        parent_span_id.clone(),
                        custom_properties.clone(),
                    );
                    audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
                        policy: triggered.policy_name.clone(),
                        reason: "rate limit exceeded".to_string(),
                    });
                    audit.response_latency_ms = start.elapsed().as_millis() as u64;
                    audit.shadow_violations = if shadow_violations.is_empty() {
                        None
                    } else {
                        Some(shadow_violations)
                    };
                    audit.emit(&state);

                    // Phase 5: Emit notification + webhook
                    let state_clone = state.clone();
                    let project_id = token.project_id;
                    let title = format!("Rate Limit Exceeded: {}", triggered.policy_name);
                    let body_msg = format!(
                        "Limit of {} requests per {}s reached",
                        max_requests, window_secs
                    );
                    let webhook_event =
                        crate::notification::webhook::WebhookEvent::rate_limit_exceeded(
                            &token.id,
                            &token.name,
                            &token.project_id.to_string(),
                            &triggered.policy_name,
                            *max_requests,
                            window_secs,
                        );
                    let webhook_urls = state.config.webhook_urls.clone();
                    state.webhook.dispatch(&webhook_urls, webhook_event).await;
                    tokio::spawn(async move {
                        let _ = state_clone
                            .db
                            .create_notification(
                                project_id,
                                "rate_limit_exceeded",
                                &title,
                                Some(&body_msg),
                                None,
                            )
                            .await;
                    });

                    return Err(AppError::RateLimitExceeded { retry_after_secs: window_secs });
                }
                policy_rate_limited = true;
            }

            // ── Override (body mutation) ──
            Action::Override { set_body_fields } => {
                if let Some(ref mut body_val) = parsed_body {
                    if let Some(obj) = body_val.as_object_mut() {
                        for (k, v) in set_body_fields {
                            obj.insert(k.clone(), v.clone());
                        }
                        tracing::info!(
                            policy = %triggered.policy_name,
                            fields = ?set_body_fields.keys().collect::<Vec<_>>(),
                            "applied body overrides"
                        );
                    }
                }
            }

            // ── Split (A/B traffic split) ──
            Action::Split {
                variants,
                experiment,
            } => {
                if variants.is_empty() {
                    tracing::warn!(policy = %triggered.policy_name, "Split action has no variants, skipping");
                } else {
                    let total_weight: u32 = variants.iter().map(|v| v.weight).sum();
                    if total_weight == 0 {
                        tracing::warn!(policy = %triggered.policy_name, "Split action has zero total weight, skipping");
                    } else {
                        // Deterministic variant selection: derive a stable bucket from request_id bytes.
                        // XOR-fold the UUID bytes into a u32 so the same request_id always picks
                        // the same variant, ensuring stable assignment within a request.
                        let req_bytes = request_id.as_bytes();
                        let bucket_seed = req_bytes[0..4]
                            .iter()
                            .enumerate()
                            .fold(0u32, |acc, (i, &b)| acc ^ ((b as u32) << (i * 8)));
                        let bucket = bucket_seed % total_weight;
                        let mut cumulative: u32 = 0;
                        let mut chosen = &variants[0];
                        for variant in variants {
                            cumulative += variant.weight;
                            if bucket < cumulative {
                                chosen = variant;
                                break;
                            }
                        }
                        // Apply the chosen variant's body field overrides
                        if let Some(ref mut body_val) = parsed_body {
                            if let Some(obj) = body_val.as_object_mut() {
                                for (k, v) in &chosen.set_body_fields {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        // Track for audit log
                        experiment_name = experiment.clone();
                        variant_name = chosen.name.clone();
                        tracing::info!(
                            policy = %triggered.policy_name,
                            experiment = ?experiment,
                            variant = ?chosen.name,
                            weight = chosen.weight,
                            total_weight,
                            bucket,
                            fields = ?chosen.set_body_fields.keys().collect::<Vec<_>>(),
                            "A/B split: assigned variant"
                        );
                    }
                }
            }

            // ── DynamicRoute (smart model selection) ──
            Action::DynamicRoute {
                strategy,
                pool,
                fallback,
            } => {
                let cb_cooldown = {
                    // Read CB cooldown from token's circuit breaker config (default 30s)
                    let cb: proxy::loadbalancer::CircuitBreakerConfig = token
                        .circuit_breaker
                        .as_ref()
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();
                    cb.recovery_cooldown_secs
                };

                if let Some(decision) = proxy::smart_router::select_route(
                    strategy,
                    pool,
                    fallback.as_ref(),
                    &state.pricing,
                    &state.latency,
                    &state.lb,
                    &token.id,
                    cb_cooldown,
                )
                .await
                {
                    // Override model in body
                    if let Some(ref mut body_val) = parsed_body {
                        if let Some(obj) = body_val.as_object_mut() {
                            obj.insert("model".into(), serde_json::json!(decision.model));
                        }
                    }
                    tracing::info!(
                        policy     = %triggered.policy_name,
                        strategy   = %decision.strategy_used,
                        model      = %decision.model,
                        upstream   = %decision.upstream_url,
                        reason     = %decision.reason,
                        "dynamic_route: selected target"
                    );
                    dynamic_upstream_override = Some(decision.upstream_url);
                    dynamic_route_strategy = Some(decision.strategy_used);
                    dynamic_route_reason = Some(decision.reason);
                } else {
                    tracing::warn!(
                        policy = %triggered.policy_name,
                        "dynamic_route: no healthy target selected, proceeding with original model"
                    );
                }
            }

            // ── Throttle ──
            Action::Throttle { delay_ms } => {
                tracing::info!(delay_ms = delay_ms, policy = %triggered.policy_name, "throttling request");
                tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
            }

            // ── HITL (handled below after all other pre-flight checks) ──
            Action::RequireApproval { timeout, .. } => {
                hitl_required = true;
                hitl_timeout_str = timeout.clone();
            }

            // ── Log ──
            Action::Log { level, tags } => match level.as_str() {
                "error" => {
                    tracing::error!(policy = %triggered.policy_name, tags = ?tags, "policy log")
                }
                "warn" => {
                    tracing::warn!(policy = %triggered.policy_name, tags = ?tags, "policy log")
                }
                _ => tracing::info!(policy = %triggered.policy_name, tags = ?tags, "policy log"),
            },

            // ── Tag (stored in audit) ──
            Action::Tag { key, value } => {
                tracing::info!(
                    policy = %triggered.policy_name,
                    tag_key = %key, tag_value = %value,
                    "policy tag"
                );
            }

            // ── Webhook (fire & forget for now) ──
            Action::Webhook {
                url, timeout_ms, ..
            } => {
                // SEC: SSRF validation for policy-defined webhook URLs (async DNS resolution)
                if !is_safe_webhook_url(url).await {
                    tracing::warn!(
                        policy = %triggered.policy_name,
                        url = %url,
                        "policy webhook blocked: SSRF protection"
                    );
                } else {
                    let url = url.clone();
                    let timeout_ms = *timeout_ms;
                    let summary = serde_json::json!({
                        "policy": triggered.policy_name,
                        "method": method.to_string(),
                        "path": path,
                        "agent": agent_name,
                        "token_id": token.id,
                    });
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        let _ = client
                            .post(&url)
                            .timeout(Duration::from_millis(timeout_ms))
                            .json(&summary)
                            .send()
                            .await;
                    });
                }
            }

            // ── Content Filter (Prompt Guardrails) ──
            Action::ContentFilter { .. } => {
                if let Some(ref body_val) = parsed_body {
                    let result = middleware::guardrail::check_content(body_val, &triggered.action);
                    if result.blocked {
                        let reason = result
                            .reason
                            .clone()
                            .unwrap_or_else(|| "Content filter blocked request".to_string());
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            risk_score = %result.risk_score,
                            patterns = ?result.matched_patterns,
                            "content filter blocked request"
                        );
                        // P1.9: Return rich ContentBlocked error with matched patterns + confidence
                        // This feeds ContentBlockedError in the Python SDK with actionable details.
                        return Err(AppError::ContentBlocked {
                            reason: reason.clone(),
                            details: Some(serde_json::json!({
                                "policy": triggered.policy_name,
                                "reason": reason,
                                "matched_patterns": result.matched_patterns,
                                "confidence": result.risk_score,
                            })),
                        });
                    } else if !result.matched_patterns.is_empty() {
                        // Patterns matched but below threshold — log as warning
                        tracing::info!(
                            policy = %triggered.policy_name,
                            risk_score = %result.risk_score,
                            patterns = ?result.matched_patterns,
                            "content filter: patterns matched but below threshold"
                        );
                    }
                }
            }

            // ── Redact (pre-flight, request-side) ──
            // SEC: Run regex-heavy redaction on a blocking thread to prevent
            // Tokio worker starvation under large payloads (100KB+).
            Action::Redact { nlp_backend, .. } => {
                if let Some(ref mut body_val) = parsed_body {
                    let action_clone = triggered.action.clone();
                    let mut body_owned = std::mem::take(body_val);
                    let (returned_body, result) = tokio::task::spawn_blocking(move || {
                        let r =
                            middleware::redact::apply_redact(&mut body_owned, &action_clone, true);
                        (body_owned, r)
                    })
                    .await
                    .map_err(|e| {
                        AppError::Internal(anyhow::anyhow!("redact task failed: {}", e))
                    })?;
                    *body_val = returned_body;

                    // NLP augmentation: detect unstructured PII if nlp_backend is configured
                    let mut nlp_matched = Vec::new();
                    if let Some(nlp_cfg) = nlp_backend {
                        let timeout = middleware::external_guardrail::guardrail_timeout();
                        let text = middleware::pii::extract_text_from_value(body_val);
                        if !text.is_empty() {
                            let detector = middleware::pii::presidio::PresidioDetector::from_config(nlp_cfg, timeout);
                            let entities = if nlp_cfg.entities.is_empty() {
                                detector.detect(&text, Some(&nlp_cfg.language)).await
                            } else {
                                middleware::pii::presidio::detect_with_entities(
                                    &detector, &text, Some(&nlp_cfg.language), &nlp_cfg.entities,
                                ).await
                            };
                            match entities {
                                Ok(ents) if !ents.is_empty() => {
                                    nlp_matched = middleware::pii::apply_nlp_entities(body_val, &ents);
                                    tracing::info!(
                                        policy = %triggered.policy_name,
                                        nlp_types = ?nlp_matched,
                                        "NLP PII detection augmented request-side redaction"
                                    );
                                }
                                Ok(_) => {} // no entities found
                                Err(e) => {
                                    // Fail-open: log warning and continue with regex-only
                                    tracing::warn!(
                                        policy = %triggered.policy_name,
                                        error = %e,
                                        "NLP PII detection failed (fail-open), continuing with regex-only"
                                    );
                                }
                            }
                        }
                    }

                    let mut all_matched = result.matched_types;
                    all_matched.extend(nlp_matched);

                    if !all_matched.is_empty() {
                        tracing::info!(
                            policy = %triggered.policy_name,
                            patterns = ?all_matched,
                            blocked = result.should_block,
                            "applied request-side redaction"
                        );
                        if result.should_block {
                            // Block mode: reject the request and tell the caller which PII was found
                            return Err(AppError::ContentBlocked {
                                reason: format!(
                                    "Request contains PII that violates policy '{}'",
                                    triggered.policy_name
                                ),
                                details: Some(serde_json::json!({
                                    "policy": triggered.policy_name,
                                    "detected_pii": all_matched,
                                    "action": "Remove sensitive data and retry"
                                })),
                            });
                        }
                        redacted_by_policy.extend(all_matched);
                    }
                }
            }

            // ── Transform ──
            Action::Transform { operations } => {
                for op in operations {
                    if let Some(ref mut body_val) = parsed_body {
                        middleware::redact::apply_transform(body_val, &mut header_mutations, op);
                    } else {
                        // No body, but we can still do header transforms
                        let mut empty = serde_json::Value::Null;
                        middleware::redact::apply_transform(&mut empty, &mut header_mutations, op);
                    }
                }
                tracing::info!(
                    policy = %triggered.policy_name,
                    ops = operations.len(),
                    "applied transform operations"
                );
            }

            // ── ConditionalRoute (request-side) ──
            Action::ConditionalRoute { branches, fallback } => {
                let req_body_val = parsed_body.clone().unwrap_or(serde_json::Value::Null);
                let matched_target = proxy::smart_router::evaluate_conditional_route_branches(
                    branches,
                    &req_body_val,
                    &headers,
                );
                let target = matched_target.or_else(|| fallback.clone()).ok_or_else(|| {
                    AppError::PolicyDenied {
                        policy: triggered.policy_name.clone(),
                        reason: "no conditional route branch matched and no fallback configured"
                            .to_string(),
                    }
                })?;
                // Override upstream URL and model the same way DynamicRoute does
                dynamic_upstream_override = Some(target.upstream_url.clone());
                if let Some(ref mut body_val) = parsed_body {
                    if let Some(obj) = body_val.as_object_mut() {
                        obj.insert("model".into(), serde_json::json!(target.model));
                    }
                }
                tracing::info!(
                    policy = %triggered.policy_name,
                    model = %target.model,
                    upstream = %target.upstream_url,
                    "conditional_route: matched branch"
                );
            }

            // ValidateSchema only applies post-flight (response phase) — skip in pre-flight
            Action::ValidateSchema { .. } => {
                tracing::debug!(
                    policy = %triggered.policy_name,
                    "ValidateSchema is a response-phase action, skipping in pre-flight"
                );
            }

            // ExternalGuardrail: call external vendor API with a hard deadline, deny or log on violation
            Action::ExternalGuardrail {
                vendor,
                endpoint,
                api_key_env,
                threshold,
                on_fail,
            } => {
                let text = parsed_body
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                // check_with_timeout wraps the vendor call in a tokio::time::timeout (default 5s,
                // configurable via TRUEFLOW_GUARDRAIL_TIMEOUT_SECS). On expiry it returns Err(...)
                // which falls through to the fail-open branch below — capping worst-case latency.
                match middleware::external_guardrail::check_with_timeout(
                    vendor,
                    endpoint,
                    api_key_env.as_deref(),
                    *threshold,
                    &text,
                )
                .await
                {
                    Ok(result) if result.blocked => {
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            vendor = ?vendor,
                            label = %result.label,
                            score = %result.score,
                            "ExternalGuardrail: violation detected"
                        );
                        if on_fail != "log" {
                            let mut audit = base_audit(
                                request_id,
                                token.project_id,
                                &token.id,
                                agent_name,
                                method.as_str(),
                                &path,
                                &token.upstream_url,
                                &policies,
                                false,
                                None,
                                None,
                                user_id.clone(),
                                tenant_id.clone(),
                                external_request_id.clone(),
                                session_id.clone(),
                                parent_span_id.clone(),
                                custom_properties.clone(),
                            );
                            audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
                                policy: triggered.policy_name.clone(),
                                reason: format!(
                                    "external_guardrail({:?}): {}",
                                    vendor, result.label
                                ),
                            });
                            audit.response_latency_ms = start.elapsed().as_millis() as u64;
                            audit.emit(&state);
                            return Err(AppError::PolicyDenied {
                                policy: triggered.policy_name.clone(),
                                reason: format!("blocked by external guardrail: {}", result.label),
                            });
                        }
                    }
                    Ok(_) => {} // clean
                    Err(e) => {
                        tracing::error!(
                            policy = %triggered.policy_name,
                            vendor = ?vendor,
                            error = %e,
                            "ExternalGuardrail: vendor call failed (fail-open)"
                        );
                    }
                }
            }

            // ── ToolScope: per-tool whitelist/blacklist RBAC ──
            Action::ToolScope {
                allowed_tools,
                blocked_tools,
                deny_message,
            } => {
                let tool_names = middleware::engine::extract_tool_names(parsed_body.as_ref());
                if !tool_names.is_empty() {
                    if let Err(reason) = middleware::engine::evaluate_tool_scope(
                        &tool_names,
                        allowed_tools,
                        blocked_tools,
                        deny_message,
                    ) {
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            tools = ?tool_names,
                            reason = %reason,
                            "ToolScope: tool denied by policy"
                        );
                        let mut audit = base_audit(
                            request_id,
                            token.project_id,
                            &token.id,
                            agent_name,
                            method.as_str(),
                            &path,
                            &token.upstream_url,
                            &policies,
                            false,
                            None,
                            None,
                            user_id.clone(),
                            tenant_id.clone(),
                            external_request_id.clone(),
                            session_id.clone(),
                            parent_span_id.clone(),
                            custom_properties.clone(),
                        );
                        audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
                            policy: triggered.policy_name.clone(),
                            reason: reason.clone(),
                        });
                        audit.response_latency_ms = start.elapsed().as_millis() as u64;
                        audit.emit(&state);
                        return Err(AppError::PolicyDenied {
                            policy: triggered.policy_name.clone(),
                            reason,
                        });
                    }
                }
            }
        }
    }
    if !policy_rate_limited && state.config.default_rate_limit > 0 {
        let rl_key = format!("rl:default:tok:{}", token.id);
        // Use sliding window to prevent 2x burst at window boundaries
        let count = state
            .cache
            .increment_sliding_window(&rl_key, state.config.default_rate_limit_window)
            .await
            .map_err(AppError::Internal)?;

        if count > state.config.default_rate_limit {
            tracing::warn!(
                token_id = %token.id,
                count = count,
                limit = state.config.default_rate_limit,
                window_secs = state.config.default_rate_limit_window,
                "default rate limit exceeded"
            );
            let mut audit = base_audit(
                request_id,
                token.project_id,
                &token.id,
                agent_name,
                method.as_str(),
                &path,
                &token.upstream_url,
                &policies,
                false,
                None,
                None,
                user_id.clone(),
                tenant_id.clone(),
                external_request_id.clone(),
                session_id.clone(),
                parent_span_id.clone(),
                custom_properties.clone(),
            );
            audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
                policy: "DefaultRateLimit".to_string(),
                reason: format!(
                    "default rate limit of {} req/{}s exceeded",
                    state.config.default_rate_limit, state.config.default_rate_limit_window
                ),
            });
            audit.response_latency_ms = start.elapsed().as_millis() as u64;
            audit.shadow_violations = if shadow_violations.is_empty() {
                None
            } else {
                Some(shadow_violations)
            };
            audit.emit(&state);
            return Err(AppError::RateLimitExceeded { retry_after_secs: state.config.default_rate_limit_window });
        }
    }

    // -- 3.5 Check Spend Cap --
    if let Err(e) =
        middleware::spend::check_spend_cap(&state.cache, state.db.pool(), &token.id).await
    {
        let mut audit = base_audit(
            request_id,
            token.project_id,
            &token.id,
            agent_name,
            method.as_str(),
            &path,
            &token.upstream_url,
            &policies,
            false,
            None,
            None,
            user_id.clone(),
            tenant_id.clone(),
            external_request_id.clone(),
            session_id.clone(),
            parent_span_id.clone(),
            custom_properties.clone(),
        );
        audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
            policy: "SpendCap".to_string(),
            reason: e.to_string(),
        });
        audit.response_latency_ms = start.elapsed().as_millis() as u64;
        audit.emit(&state);

        // Webhook dispatch
        let webhook_event = crate::notification::webhook::WebhookEvent::spend_cap_exceeded(
            &token.id,
            &token.name,
            &token.project_id.to_string(),
            &e.to_string(),
        );
        let webhook_urls = state.config.webhook_urls.clone();
        state.webhook.dispatch(&webhook_urls, webhook_event).await;

        return Err(AppError::SpendCapReached {
            message: format!(
                "Spend cap reached (USD): {}. Check your limits at the TrueFlow dashboard.",
                e
            ),
        });
    }

    // -- 3.5b Project-Level Hard Cap --
    // Uses a 60s Redis cache to avoid a DB round-trip on every request.
    if crate::jobs::budget_checker::is_project_over_hard_cap_cached(
        state.db.pool(),
        &state.cache,
        token.project_id,
    )
    .await
    {
        let mut audit = base_audit(
            request_id,
            token.project_id,
            &token.id,
            agent_name,
            method.as_str(),
            &path,
            &token.upstream_url,
            &policies,
            false,
            None,
            None,
            user_id.clone(),
            tenant_id.clone(),
            external_request_id.clone(),
            session_id.clone(),
            parent_span_id.clone(),
            custom_properties.clone(),
        );
        audit.policy_result = Some(crate::models::audit::PolicyResult::Deny {
            policy: "ProjectBudgetCap".to_string(),
            reason: "Project hard spend cap exceeded".to_string(),
        });
        audit.response_latency_ms = start.elapsed().as_millis() as u64;
        audit.emit(&state);

        return Err(AppError::SpendCapReached {
            message: "Project spending limit reached. Contact your administrator or review limits at the TrueFlow dashboard.".to_string(),
        });
    }

    // -- 3.6 Session Lifecycle Guard --
    // If this request has a session_id, auto-create the session on first use,
    // then enforce status (reject if paused/completed) and session-level spend cap.
    if let Some(ref sid) = session_id {
        // Upsert: auto-create session on first request, or touch updated_at
        match state
            .db
            .upsert_session(sid, token.project_id, Uuid::parse_str(&token.id).ok(), None)
            .await
        {
            Ok(entity) => {
                // Reject requests against paused or completed sessions
                if entity.status != "active" {
                    tracing::info!(
                        session_id = %sid,
                        status = %entity.status,
                        "Session lifecycle: request rejected (non-active session)"
                    );
                    return Err(AppError::PolicyDenied {
                        policy: "SessionLifecycle".to_string(),
                        reason: format!(
                            "Session '{}' is {} — cannot accept new requests",
                            sid, entity.status
                        ),
                    });
                }

                // Check session-level spend cap
                if let Some(cap) = entity.spend_cap_usd {
                    if entity.total_cost_usd >= cap {
                        tracing::info!(
                            session_id = %sid,
                            total = %entity.total_cost_usd,
                            cap = %cap,
                            "Session spend cap exceeded"
                        );
                        return Err(AppError::SpendCapReached {
                            message: format!(
                                "Session '{}' has exceeded its spend cap ({} USD)",
                                sid, cap
                            ),
                        });
                    }
                }
            }
            Err(e) => {
                // Fail open: log error but don't block the request
                tracing::warn!(
                    session_id = %sid,
                    error = %e,
                    "Session upsert failed (fail-open)"
                );
            }
        }
    }

    // -- 3.7 Anomaly Detection (non-blocking, informational) --
    // Record this request's timestamp in a Redis sliding window and check
    // if the token's velocity exceeds 3σ from its rolling baseline.
    {
        let anomaly_config = middleware::anomaly::AnomalyConfig::default();
        let mut redis_conn = state.cache.redis();
        match middleware::anomaly::record_and_check(
            &mut redis_conn,
            &token.id.to_string(),
            &anomaly_config,
        )
        .await
        {
            Ok(result) if result.is_anomalous => {
                tracing::warn!(
                    token_id = %token.id,
                    velocity = result.current_velocity,
                    mean = result.baseline_mean,
                    threshold = result.threshold,
                    "Anomaly detected: velocity spike for token"
                );
                // Fire webhook (non-blocking)
                let webhook_event = crate::notification::webhook::WebhookEvent::anomaly_detected(
                    &token.id.to_string(),
                    &token.name,
                    &token.project_id.to_string(),
                    result.current_velocity,
                    result.baseline_mean,
                    result.threshold,
                );
                let webhook_urls = state.config.webhook_urls.clone();
                state.webhook.dispatch(&webhook_urls, webhook_event).await;
                // NOTE: anomaly detection is informational — we do NOT block the request.
                // Use rate limiting policies for enforcement.
            }
            Ok(_) => {} // Normal velocity
            Err(e) => {
                tracing::debug!(error = %e, "Anomaly check failed (non-critical, proceeding)");
            }
        }
    }

    // -- 3.8 Handle HITL --
    if hitl_required {
        let hitl_start = Instant::now();

        // ── HITL Concurrency Cap ──
        // Prevent unbounded pending approval requests from blocking handler threads.
        let hitl_max_pending: i64 = std::env::var("HITL_MAX_PENDING_PER_TOKEN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        match state
            .db
            .count_pending_approvals_for_token(&token.id, token.project_id)
            .await
        {
            Ok(pending_count) if pending_count >= hitl_max_pending => {
                tracing::warn!(
                    token_id = %token.id,
                    pending = pending_count,
                    max = hitl_max_pending,
                    "HITL concurrency cap exceeded — rejecting request"
                );
                return Err(AppError::Forbidden(format!(
                    "HITL concurrency cap exceeded: {} pending approval requests (max {}). \
                     Approve or reject existing requests before submitting new ones.",
                    pending_count, hitl_max_pending
                )));
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to check HITL pending count — proceeding with caution"
                );
                // Proceed anyway — fail-open to avoid blocking legitimate requests
            }
            _ => {} // Under cap, proceed normally
        }

        // Create approval request
        // Use DynamicRoute-selected upstream if available, otherwise fall back to token default
        let effective_upstream = dynamic_upstream_override
            .as_ref()
            .unwrap_or(&token.upstream_url);
        let summary = serde_json::json!({
            "method": method.to_string(),
            "path": path,
            "agent": agent_name,
            "upstream": effective_upstream,
            "body_preview": parsed_body.as_ref().map(|b| {
                // SEC: use char-based truncation to avoid panicking on multi-byte UTF-8 boundaries.
                // Limit raised to 2000 chars so HITL reviewers see enough context.
                let s = b.to_string();
                let char_count = s.chars().count();
                if char_count > 2000 {
                    let truncated: String = s.chars().take(2000).collect();
                    format!("{}…", truncated)
                } else {
                    s
                }
            }),
        });

        // Expiry: 10 minutes (can be overridden by policy timeout later)
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);

        let approval_id = state
            .db
            .create_approval_request(
                &token.id,
                token.project_id,
                idempotency_key.clone(),
                summary.clone(),
                expires_at,
            )
            .await
            .map_err(AppError::Internal)?;

        // Phase 5: Emit notifications
        // 1. Dashboard Notification
        let state_clone = state.clone();
        let project_id = token.project_id;
        let title = "Approval Required".to_string();
        let body_text = format!("Request to {} requires approval.", path);
        let metadata = serde_json::json!({ "approval_id": approval_id });
        tokio::spawn(async move {
            let _ = state_clone
                .db
                .create_notification(
                    project_id,
                    "approval_needed",
                    &title,
                    Some(&body_text),
                    Some(metadata),
                )
                .await;
        });

        // 2. Webhook Notification (includes full request body for app parsing)
        let webhook_event = crate::notification::webhook::WebhookEvent::approval_requested(
            &token.id,
            &token.name,
            &token.project_id.to_string(),
            &approval_id.to_string(),
            method.as_str(),
            &path,
            &token.upstream_url,
            parsed_body.clone(),
        );
        let webhook_urls = state.config.webhook_urls.clone();
        let webhook_notifier = state.webhook.clone();
        tokio::spawn(async move {
            webhook_notifier
                .dispatch(&webhook_urls, webhook_event)
                .await;
        });

        // 3. Send Slack notification (async)
        let notifier = state.notifier.clone();
        let app_id = approval_id;
        let summary_clone = summary.clone();
        let expires_at_clone = expires_at;
        tokio::spawn(async move {
            if let Err(e) = notifier
                .send_approval_request(&app_id, &summary_clone, &expires_at_clone)
                .await
            {
                tracing::error!("Failed to send approval notification: {}", e);
            }
        });

        let timeout_secs = middleware::policy::parse_window_secs(&hitl_timeout_str).unwrap_or(1800); // default 30m

        // ── HITL: Dedicated-connection BLPOP for instant approval delivery ──
        // We create a dedicated Redis connection (not the shared ConnectionManager)
        // so BLPOP blocks only this connection without affecting the connection pool.
        // This gives us sub-millisecond notification latency vs the old 500ms LPOP polling.
        let hitl_key = format!("hitl:decision:{}", approval_id);
        let mut decision_opt: Option<String> = None;

        // Try dedicated BLPOP first, fall back to LPOP polling if connection fails
        let blpop_timeout = timeout_secs.min(1800) as f64; // Redis BLPOP timeout in seconds
        let blpop_result: Result<Option<String>, ()> = async {
            // Open a fresh, dedicated connection for the blocking call
            let redis_url =
                std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
            let client = redis::Client::open(redis_url.as_str()).map_err(|e| {
                tracing::warn!("HITL: failed to create dedicated Redis client: {}", e);
            })?;
            let mut conn = client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| {
                    tracing::warn!("HITL: failed to open dedicated Redis connection: {}", e);
                })?;

            // BLPOP blocks until a value is pushed or timeout expires.
            // Returns Option<(key, value)> tuple.
            let result: redis::RedisResult<Option<(String, String)>> = redis::cmd("BLPOP")
                .arg(&hitl_key)
                .arg(blpop_timeout)
                .query_async(&mut conn)
                .await;

            match result {
                Ok(Some((_key, value))) => Ok(Some(value)),
                Ok(None) => Ok(None), // timeout expired
                Err(e) => {
                    tracing::warn!("HITL BLPOP failed: {}", e);
                    Err(())
                }
            }
        }
        .await;

        match blpop_result {
            Ok(Some(value)) => {
                decision_opt = Some(value);
            }
            Ok(None) => {
                // BLPOP timed out — no decision received
            }
            Err(()) => {
                // Redis failed — fall back to LPOP polling loop
                tracing::info!("HITL: falling back to LPOP polling");
                let mut redis_conn = state.cache.redis();
                let start_wait = std::time::Instant::now();
                let timeout_duration = std::time::Duration::from_secs(timeout_secs as u64);

                while start_wait.elapsed() < timeout_duration {
                    let lpop_result: redis::RedisResult<Option<String>> =
                        redis::AsyncCommands::lpop(&mut redis_conn, &hitl_key, None).await;

                    match lpop_result {
                        Ok(Some(value)) => {
                            decision_opt = Some(value);
                            break;
                        }
                        Ok(None) => {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                        Err(e) => {
                            tracing::warn!("HITL LPOP fallback also failed: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        let decision = match decision_opt {
            Some(value) => value,
            None => {
                // All Redis paths exhausted — final DB check
                state
                    .db
                    .get_approval_status(approval_id, token.project_id)
                    .await
                    .map_err(AppError::Internal)?
            }
        };

        match decision.as_str() {
            "approved" => {
                // 6B-4 FIX: Re-validate token is still active after HITL wait.
                // Prevents revoked tokens from executing approved requests.
                match state.db.get_token(&token.id).await {
                    Ok(Some(fresh_token)) if fresh_token.is_active => {
                        // Token still valid — proceed
                    }
                    Ok(Some(_not_active)) => {
                        tracing::warn!(
                            token_id = %token.id,
                            approval_id = %approval_id,
                            "HITL approved but token was revoked during wait — rejecting"
                        );
                        let mut audit = base_audit(
                            request_id,
                            token.project_id,
                            &token.id,
                            agent_name,
                            method.as_str(),
                            &path,
                            &token.upstream_url,
                            &policies,
                            true,
                            Some("approved_but_revoked".to_string()),
                            Some(hitl_start.elapsed().as_millis() as i32),
                            user_id.clone(),
                            tenant_id.clone(),
                            external_request_id.clone(),
                            session_id.clone(),
                            parent_span_id.clone(),
                            custom_properties.clone(),
                        );
                        audit.policy_result =
                            Some(crate::models::audit::PolicyResult::HitlRejected);
                        audit.response_latency_ms = start.elapsed().as_millis() as u64;
                        audit.emit(&state);
                        return Err(AppError::TokenRevoked);
                    }
                    Ok(None) => {
                        tracing::warn!(
                            token_id = %token.id,
                            "HITL approved but token no longer exists — rejecting"
                        );
                        return Err(AppError::TokenRevoked);
                    }
                    Err(e) => {
                        tracing::error!(
                            token_id = %token.id,
                            error = %e,
                            "HITL post-approval token re-validation failed — rejecting (fail-closed)"
                        );
                        return Err(AppError::Internal(e));
                    }
                }
                hitl_decision = Some("approved".to_string());
            }
            "rejected" => {
                hitl_decision = Some("rejected".to_string());
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &token.upstream_url,
                    &policies,
                    true,
                    hitl_decision.clone(),
                    Some(hitl_start.elapsed().as_millis() as i32),
                    user_id.clone(),
                    tenant_id.clone(),
                    external_request_id.clone(),
                    session_id.clone(),
                    parent_span_id.clone(),
                    custom_properties.clone(),
                );
                audit.policy_result = Some(crate::models::audit::PolicyResult::HitlRejected);
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.shadow_violations = if shadow_violations.is_empty() {
                    None
                } else {
                    Some(shadow_violations.clone())
                };
                audit.emit(&state);
                return Err(AppError::ApprovalRejected);
            }
            _ => {
                hitl_decision = Some("expired".to_string());
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &token.upstream_url,
                    &policies,
                    true,
                    hitl_decision.clone(),
                    Some(hitl_start.elapsed().as_millis() as i32),
                    user_id.clone(),
                    tenant_id.clone(),
                    external_request_id.clone(),
                    session_id.clone(),
                    parent_span_id.clone(),
                    custom_properties.clone(),
                );
                audit.policy_result = Some(crate::models::audit::PolicyResult::HitlTimeout);
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.shadow_violations = if shadow_violations.is_empty() {
                    None
                } else {
                    Some(shadow_violations.clone())
                };
                audit.emit(&state);
                return Err(AppError::ApprovalTimeout);
            }
        }

        hitl_latency_ms = Some(hitl_start.elapsed().as_millis() as i32);
    }

    // -- 4. Resolve credential + upstream URL --
    // Service Registry: if path starts with /v1/proxy/services/{name}/...,
    // dynamically resolve the service and use its credential + base_url.
    let service_prefix = "/v1/proxy/services/";
    let (effective_credential_id, effective_upstream_url, effective_path) = if let Some(rest) =
        path.strip_prefix(service_prefix)
    {
        let (svc_name, remaining_path) = match rest.find('/') {
            Some(pos) => (&rest[..pos], &rest[pos..]), // ("stripe", "/v1/charges")
            None => (rest, "/"),                       // ("stripe", "/")
        };

        let service = state
            .db
            .get_service_by_name(token.project_id, svc_name)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::Upstream(format!("Service not found: {}", svc_name)))?;

        // Service may or may not have a credential — passthrough is allowed
        Ok((
            service.credential_id,
            service.base_url.clone(),
            remaining_path.to_string(),
        ))
    } else {
        // Loadbalancer: use weighted routing. Fallback to upstream_url if JSONB is empty.
        let mut lb_upstreams = proxy::loadbalancer::parse_upstreams(token.upstreams.as_ref());

        if lb_upstreams.is_empty() {
            // Legacy/Single upstream mode: create a single target from upstream_url
            lb_upstreams.push(proxy::loadbalancer::UpstreamTarget {
                url: token.upstream_url.clone(),
                weight: 100,
                priority: 1,
                credential_id: None, // Will fallback to token.credential_id below
            });
        }

        // DEBUG LOGGING
        tracing::info!(token_id = %token.id, upstream_count = lb_upstreams.len(), "Calling LB select");

        // Always route through LB to ensure health tracking
        if let Some(idx) = state.lb.select(&token.id, &lb_upstreams, &cb_config) {
            let target = &lb_upstreams[idx];
            tracing::info!(token_id = %token.id, selected_url = %target.url, "LB selected target");
            // Use target-specific credential if set, otherwise token default
            let effective_cred_id = target.credential_id.or(token.credential_id);
            Ok((effective_cred_id, target.url.clone(), path.clone()))
        } else {
            // All upstreams unhealthy — return error instead of flooding a broken upstream
            tracing::error!(token_id = %token.id, "all upstreams unhealthy, circuit breaker open");
            Err(AppError::AllUpstreamsExhausted {
                details: Some(serde_json::json!({
                    "reason": "all_upstreams_unhealthy",
                    "token_id": token.id,
                })),
            })
        }
    }?;

    // -- 4.1 Credential injection vs passthrough --
    // If credential_id is Some, decrypt from vault and inject.
    // If None, operate in passthrough mode: forward X-Real-Authorization from the agent.
    struct InjectedCredential {
        key: String,
        mode: String,
        header: String,
    }

    let injected_cred = if let Some(cred_id) = effective_credential_id {
        let (real_key, _provider, injection_mode, injection_header) = state
            .vault
            .retrieve(&cred_id.to_string())
            .await
            .map_err(AppError::Internal)?;
        Some(InjectedCredential {
            key: real_key,
            mode: injection_mode,
            header: injection_header,
        })
    } else {
        None // Passthrough mode
    };

    // -- 5. Build upstream request --
    let upstream_url = proxy::transform::rewrite_url(&effective_upstream_url, &effective_path);

    // ── Response Cache: check for cache hit BEFORE upstream call ──
    let token_scopes: Vec<String> = token
        .scopes
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let skip_cache = proxy::response_cache::should_skip_cache(
        &headers,
        parsed_body.as_ref(),
        Some(&token_scopes),
    ) || is_streaming_req
        || method != Method::POST;
    let cache_key = if !skip_cache {
        parsed_body
            .as_ref()
            .and_then(|b| proxy::response_cache::compute_cache_key(&token.id, b))
    } else {
        None
    };

    if let Some(ref key) = cache_key {
        if let Some(cached) = proxy::response_cache::get_cached(&state.cache, key).await {
            tracing::info!(cache_key = %key, "response cache HIT");

            // BILLING: Record spend for cached responses
            if let (Some(prompt_tokens), Some(completion_tokens)) =
                (cached.prompt_tokens, cached.completion_tokens)
            {
                if let Some(ref cached_model) = cached.model {
                    // Detect provider from upstream URL
                    let provider = if token.upstream_url.contains("anthropic")
                        && !token.upstream_url.contains("bedrock")
                    {
                        "anthropic"
                    } else if token.upstream_url.contains("generativelanguage")
                        || token.upstream_url.contains("googleapis")
                    {
                        "google"
                    } else if token.upstream_url.contains("mistral") {
                        "mistral"
                    } else if token.upstream_url.contains("bedrock") {
                        "bedrock"
                    } else if token.upstream_url.contains("groq") {
                        "groq"
                    } else if token.upstream_url.contains("cohere") {
                        "cohere"
                    } else if token.upstream_url.contains("together") {
                        "together"
                    } else if token.upstream_url.contains("localhost:11434")
                        || token.upstream_url.contains("ollama")
                    {
                        "ollama"
                    } else {
                        "openai"
                    };

                    let final_cost = cost::calculate_cost_with_cache(
                        &state.pricing,
                        provider,
                        cached_model,
                        prompt_tokens,
                        completion_tokens,
                    )
                    .await;

                    if !final_cost.is_zero() {
                        let cost_f64 = final_cost.to_f64().unwrap_or(0.0);
                        if let Err(e) = middleware::spend::check_and_increment_spend(
                            &state.cache,
                            state.db.pool(),
                            &token.id,
                            cost_f64,
                        )
                        .await
                        {
                            tracing::error!(token_id = %token.id, cost = cost_f64, "Cache hit: spend cap exceeded or tracking failed: {}", e);
                        }
                    }
                }
            }

            let mut audit = base_audit(
                request_id,
                token.project_id,
                &token.id,
                agent_name,
                method.as_str(),
                &path,
                &upstream_url,
                &policies,
                hitl_required,
                hitl_decision,
                hitl_latency_ms,
                user_id,
                tenant_id,
                external_request_id,
                session_id,
                parent_span_id,
                custom_properties.clone(),
            );
            audit.policy_result = Some(crate::models::audit::PolicyResult::Allow);
            audit.upstream_status = Some(cached.status);
            audit.response_latency_ms = start.elapsed().as_millis() as u64;
            audit.cache_hit = true;
            audit.model = cached.model;
            audit.prompt_tokens = cached.prompt_tokens;
            audit.completion_tokens = cached.completion_tokens;
            audit.shadow_violations = if shadow_violations.is_empty() {
                None
            } else {
                Some(shadow_violations)
            };
            audit.emit(&state);

            let axum_status =
                StatusCode::from_u16(cached.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            return Response::builder()
                .status(axum_status)
                .header("content-type", cached.content_type)
                .header("x-trueflow-cache", "HIT")
                .body(Body::from(cached.body))
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("cached response build failed: {}", e))
                });
        }
    }

    // ── Universal Model Router: translate request for non-OpenAI providers ──
    let detected_model = parsed_body
        .as_ref()
        .and_then(|b| b.get("model"))
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();

    // ── Model Access Control (RBAC Depth) ──
    // Check if this token is allowed to use the requested model.
    if !detected_model.is_empty() {
        let group_models = if let Some(ref group_ids) = token.allowed_model_group_ids {
            if !group_ids.is_empty() {
                middleware::model_access::resolve_group_models(
                    state.db.pool(),
                    group_ids,
                    token.project_id,
                )
                .await
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if let Err(reason) = middleware::model_access::check_model_access(
            &detected_model,
            token.allowed_models.as_ref(),
            &group_models,
        ) {
            tracing::warn!(
                token_id = %token.id,
                model = %detected_model,
                "Model access denied: {}",
                reason
            );
            let mut audit = base_audit(
                request_id,
                token.project_id,
                &token.id,
                agent_name,
                method.as_str(),
                &path,
                &upstream_url,
                &policies,
                hitl_required,
                hitl_decision,
                hitl_latency_ms,
                user_id,
                tenant_id,
                external_request_id,
                session_id,
                parent_span_id,
                custom_properties,
            );
            audit.upstream_status = Some(403);
            audit.response_latency_ms = start.elapsed().as_millis() as u64;
            audit.emit(&state);
            return Err(AppError::Forbidden(reason));
        }
    }

    // ── Team-Level Enforcement (Budget + Model Access + Tags) ──
    let resolved_team = if let Some(team_id) = token.team_id {
        middleware::teams::get_team(state.db.pool(), team_id).await
    } else {
        None
    };

    if let Some(ref team) = resolved_team {
        // Check team budget
        if let Err(reason) = middleware::teams::check_team_budget(state.db.pool(), team).await {
            tracing::warn!(token_id = %token.id, team = %team.name, "Team budget exceeded: {}", reason);
            return Err(AppError::SpendCapReached { message: reason });
        }

        // Check team-level model restrictions
        if !detected_model.is_empty() {
            if let Err(reason) = middleware::teams::check_team_model_access(&detected_model, team) {
                tracing::warn!(token_id = %token.id, team = %team.name, model = %detected_model, "Team model access denied: {}", reason);
                return Err(AppError::Forbidden(reason));
            }
        }
    }

    let detected_provider = if !detected_model.is_empty() {
        proxy::model_router::detect_provider(&detected_model, &effective_upstream_url)
    } else {
        proxy::model_router::Provider::Unknown
    };

    // Translate request body if needed (OpenAI → Anthropic/Gemini)
    let router_translated = if let Some(ref body_val) = parsed_body {
        proxy::model_router::translate_request(detected_provider, body_val)
    } else {
        None
    };

    // Rewrite upstream URL for the target provider.
    // Gemini uses different endpoints for streaming vs non-streaming.
    // If DynamicRoute/ConditionalRoute selected a different upstream, use that instead.
    let upstream_url = if let Some(dyn_url) = dynamic_upstream_override {
        // DynamicRoute override takes precedence.
        // Re-detect provider from the new upstream URL + the DynamicRoute-selected model.
        let dyn_model = parsed_body
            .as_ref()
            .and_then(|b| b.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or(&detected_model);
        let dyn_provider = proxy::model_router::detect_provider(dyn_model, &dyn_url);

        if dyn_provider != proxy::model_router::Provider::OpenAI
            && dyn_provider != proxy::model_router::Provider::Unknown
        {
            // Non-OpenAI provider: let model_router rewrite the URL (e.g. Gemini paths)
            proxy::model_router::rewrite_upstream_url(
                dyn_provider,
                &dyn_url,
                dyn_model,
                is_streaming_req,
            )
        } else {
            // OpenAI-compatible: append the original request path to the override base URL
            proxy::transform::rewrite_url(&dyn_url, &effective_path)
        }
    } else if detected_provider != proxy::model_router::Provider::OpenAI
        && detected_provider != proxy::model_router::Provider::Unknown
    {
        proxy::model_router::rewrite_upstream_url(
            detected_provider,
            &upstream_url,
            &detected_model,
            is_streaming_req,
        )
    } else {
        upstream_url
    };

    // Use modified body if overrides were applied, otherwise original
    let final_body = if let Some(ref translated) = router_translated {
        // Model router translated the body — use translated version
        serde_json::to_vec(translated).unwrap_or_else(|_| body.to_vec())
    } else if let Some(ref modified) = parsed_body {
        // Check if body was modified by Override action
        serde_json::to_vec(modified).unwrap_or_else(|_| body.to_vec())
    } else {
        body.to_vec()
    };

    // ── MCP Tool Injection ───────────────────────────────────────
    // If X-MCP-Servers header is present, inject MCP tool schemas into the
    // request body's `tools[]` array before sending to the LLM.
    // Per-token allow/deny lists filter which MCP tools are injected.
    let mcp_server_names = crate::middleware::mcp::parse_mcp_header(&headers);
    let mcp_allowed = crate::middleware::mcp::parse_tool_list(token.mcp_allowed_tools.as_ref());
    let mcp_blocked = crate::middleware::mcp::parse_tool_list(token.mcp_blocked_tools.as_ref());
    let final_body = if !mcp_server_names.is_empty() {
        match crate::middleware::mcp::inject_mcp_tools(
            &state.mcp_registry,
            &mcp_server_names,
            &final_body,
            mcp_allowed.as_deref(),
            mcp_blocked.as_deref(),
        )
        .await
        {
            Some(injected) => injected,
            None => final_body,
        }
    } else {
        final_body
    };

    // ── BUG-1 FIX: Inject stream_options.include_usage for streaming requests ──
    // OpenAI only returns token counts in the final SSE chunk when the client
    // explicitly requests it via stream_options.include_usage = true.
    // Without this, all streaming responses have zero tokens and zero cost.
    // This is the industry-standard approach (Portkey, Helicone, LangSmith all do this).
    let final_body = if is_streaming_req {
        if let Ok(mut body_json) = serde_json::from_slice::<serde_json::Value>(&final_body) {
            // FIX(C3): Only inject stream_options for OpenAI-compatible providers.
            // Anthropic, Gemini, and Bedrock use different API formats that don't
            // recognize this field — Bedrock returns ValidationException, Gemini
            // returns INVALID_ARGUMENT. Only OpenAI/Azure/Groq/Mistral/Together/
            // Cohere/Ollama support stream_options.include_usage.
            let skip_stream_options = matches!(
                detected_provider,
                proxy::model_router::Provider::Anthropic
                    | proxy::model_router::Provider::Gemini
                    | proxy::model_router::Provider::Bedrock
            );
            if !skip_stream_options {
                // Set stream_options.include_usage = true (preserve any existing stream_options)
                let stream_opts = body_json.as_object_mut().and_then(|obj| {
                    obj.entry("stream_options")
                        .or_insert_with(|| serde_json::json!({}));
                    obj.get_mut("stream_options")
                });
                if let Some(opts) = stream_opts {
                    opts["include_usage"] = serde_json::json!(true);
                }
            }
            serde_json::to_vec(&body_json).unwrap_or(final_body)
        } else {
            final_body
        }
    } else {
        final_body
    };

    // Build upstream headers
    let mut upstream_headers = reqwest::header::HeaderMap::new();

    // Track injection mode for audit logging
    let _injection_mode_str: String;

    if let Some(ref cred) = injected_cred {
        // === Credential injection mode ===
        _injection_mode_str = cred.mode.clone();

        let header_name: reqwest::header::HeaderName = cred.header.parse().map_err(|_| {
            AppError::Internal(anyhow::anyhow!("invalid injection_header: {}", cred.header))
        })?;

        match cred.mode.as_str() {
            "basic" => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&cred.key);
                upstream_headers.insert(
                    header_name,
                    reqwest::header::HeaderValue::from_str(&format!("Basic {}", encoded))
                        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid key format")))?,
                );
            }
            "header" => {
                upstream_headers.insert(
                    header_name,
                    reqwest::header::HeaderValue::from_str(&cred.key)
                        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid key format")))?,
                );
            }
            "query" => {
                // Don't inject a header — we'll append to the URL below
            }
            // FIX(C1): SigV4 signing for Amazon Bedrock.
            // The vault stores credentials as "ACCESS_KEY_ID:SECRET_ACCESS_KEY".
            // The region is extracted from the upstream URL.
            // Signing is deferred until after the final body is ready (see below).
            "sigv4" => {
                // SigV4 signing will be applied later, after the final body is built,
                // because the signature depends on the request body hash.
                // We store the parsed credentials here and apply signing below.
                // (Headers are injected later in the sigv4 signing block)
            }
            _ => {
                upstream_headers.insert(
                    header_name,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", cred.key))
                        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid key format")))?,
                );
            }
        }
    } else {
        // === Passthrough mode ===
        // Forward the agent's own authorization via X-Real-Authorization or X-Upstream-Authorization
        _injection_mode_str = "passthrough".to_string();

        let real_auth = headers
            .get("X-Real-Authorization")
            .or_else(|| headers.get("X-Upstream-Authorization"))
            .and_then(|v| v.to_str().ok());

        if let Some(auth_value) = real_auth {
            upstream_headers.insert(
                "authorization",
                reqwest::header::HeaderValue::from_str(auth_value).map_err(|_| {
                    AppError::Internal(anyhow::anyhow!(
                        "invalid auth header in X-Real-Authorization"
                    ))
                })?,
            );
        }
        // If no real auth header provided, forward without Authorization.
        // This supports upstream APIs that don't require auth (e.g., public endpoints).
    }

    upstream_headers.insert(
        "Content-Type",
        reqwest::header::HeaderValue::from_str(&original_content_type).unwrap_or(
            reqwest::header::HeaderValue::from_static("application/json"),
        ),
    );

    // ── Provider-specific required header injection ─────────────────────────
    // Injects headers that the upstream API mandates (e.g. anthropic-version).
    // Uses entry().or_insert() so policy-level headers always win.
    proxy::model_router::inject_provider_headers(
        detected_provider,
        &mut upstream_headers,
        is_streaming_req,
    );

    // Apply transform header mutations (SetHeader / RemoveHeader)
    for name in &header_mutations.removals {
        if let Ok(header_name) = name.parse::<reqwest::header::HeaderName>() {
            upstream_headers.remove(header_name);
        }
    }
    for (name, value) in &header_mutations.inserts {
        if let (Ok(header_name), Ok(header_value)) = (
            name.parse::<reqwest::header::HeaderName>(),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            upstream_headers.insert(header_name, header_value);
        }
    }

    // W3C Trace Context propagation: forward a child traceparent to the upstream.
    // We use the incoming trace_id (if any) to maintain the same trace, but generate
    // a new span_id from our request_id so the gateway hop is visible in traces.
    let gateway_span_id = &request_id.to_string().replace('-', "")[..16]; // 16 hex chars
    let outbound_traceparent = if let Some(ref trace_id) = w3c_trace_id {
        // A traceparent was received — propagate the same trace with our span as parent
        format!("00-{}-{}-01", trace_id, gateway_span_id)
    } else {
        // No incoming traceparent — start a new one using req_id as trace_id
        let trace_hex = request_id.to_string().replace('-', "");
        format!("00-{}-{}-01", trace_hex, gateway_span_id)
    };
    if let Ok(hv) = reqwest::header::HeaderValue::from_str(&outbound_traceparent) {
        upstream_headers.insert("traceparent", hv);
        tracing::debug!(
            traceparent = %outbound_traceparent,
            req_id = %request_id,
            "forwarding traceparent to upstream"
        );
    }
    // Also forward tracestate if present (preserves vendor-specific data)
    if let Some(ts) = headers.get("tracestate").and_then(|v| v.to_str().ok()) {
        if let Ok(hv) = reqwest::header::HeaderValue::from_str(ts) {
            upstream_headers.insert("tracestate", hv);
        }
    }

    // Forward standard safe headers (required by strict APIs like GitHub)
    // BUT skip if a transform explicitly removed User-Agent
    let ua_removed = header_mutations
        .removals
        .iter()
        .any(|name| name.eq_ignore_ascii_case("user-agent"));
    if !ua_removed {
        if let Some(ua) = headers.get(reqwest::header::USER_AGENT) {
            upstream_headers.insert(reqwest::header::USER_AGENT, ua.clone());
        } else {
            // Fallback User-Agent if none provided by client
            upstream_headers.insert(
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_static("TrueFlow-Gateway/1.0"),
            );
        }
    }

    if let Some(accept) = headers.get(reqwest::header::ACCEPT) {
        upstream_headers.insert(reqwest::header::ACCEPT, accept.clone());
    }

    // Only forward explicitly allowed custom headers to upstream.
    // SEC: Allowlist approach — safer than blocklist for a security boundary.
    // A blocklist would let novel headers (e.g. x-api-key) slip through and
    // bypass credential injection, spend tracking, and audit logging.
    // Add entries here intentionally after security review.
    // Never add: x-api-key, x-auth-*, authorization, x-amz-*, x-goog-*, x-forwarded-*
    const FORWARDED_HEADERS: &[&str] = &[
        "x-mock-latency-ms",
        "x-mock-flaky",
        "x-mock-status",
        "x-mock-stream",
    ];

    for header_name in FORWARDED_HEADERS {
        if let Some(value) = headers.get(*header_name) {
            if let Ok(hname) = header_name.parse::<reqwest::header::HeaderName>() {
                upstream_headers.insert(hname, value.clone());
            }
        }
    }

    // Convert method Axum -> Reqwest
    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid method: {}", e)))?;

    // Handle query param injection by appending to the URL (only in credential injection mode)
    let final_upstream_url = if let Some(ref cred) = injected_cred {
        if cred.mode == "query" {
            let separator = if upstream_url.contains('?') { "&" } else { "?" };
            format!(
                "{}{}{}={}",
                upstream_url,
                separator,
                cred.header,
                urlencoding::encode(&cred.key)
            )
        } else {
            upstream_url.clone()
        }
    } else {
        upstream_url.clone()
    };

    // Track in-flight requests for least-busy routing
    state.lb.increment_in_flight(&final_upstream_url);

    // Ensure CB health state is initialized for single-upstream tokens.
    // Multi-upstream tokens are initialized via select_upstream(), but single-upstream
    // tokens skip that path and need explicit initialization.
    if cb_config.enabled {
        let synthetic = vec![proxy::loadbalancer::UpstreamTarget {
            url: final_upstream_url.clone(),
            credential_id: None,
            weight: 100,
            priority: 1,
        }];
        state.lb.ensure_health(&token.id, &synthetic);
    }

    // ── Circuit Breaker pre-check ────────────────────────────────────────────
    // For single-upstream tokens, fail fast with 503 when the circuit is OPEN.
    // This prevents flooding a known-broken upstream with requests.
    if cb_config.enabled {
        let cb_state = state.lb.get_circuit_state(
            &token.id,
            &final_upstream_url,
            cb_config.recovery_cooldown_secs,
        );
        if cb_state == "open" {
            state.lb.decrement_in_flight(&final_upstream_url);
            return Err(AppError::AllUpstreamsExhausted {
                details: Some(serde_json::json!({
                    "reason": "circuit_breaker_open",
                    "upstream": final_upstream_url,
                    "cooldown_secs": cb_config.recovery_cooldown_secs,
                })),
            });
        }
    }

    // Save upstream headers for potential MCP tool loop continuation requests
    let mcp_upstream_headers = if !mcp_server_names.is_empty() {
        Some(upstream_headers.clone())
    } else {
        None
    };

    // ── FIX(C1): Apply deferred SigV4 signing for Bedrock ──────────────────
    // SigV4 signing requires the final body (for payload hash) and final URL,
    // so it must happen after all body/URL transformations but before sending.
    // The credential key format is "ACCESS_KEY_ID:SECRET_ACCESS_KEY".
    if let Some(ref cred) = injected_cred {
        if cred.mode == "sigv4" {
            // Parse "ACCESS_KEY_ID:SECRET_ACCESS_KEY"
            let (access_key, secret_key) = cred.key.split_once(':').ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!(
                    "SigV4 credential must be in format ACCESS_KEY_ID:SECRET_ACCESS_KEY"
                ))
            })?;

            let region = proxy::sigv4::extract_region(&final_upstream_url)
                .unwrap_or_else(|| "us-east-1".to_string());

            proxy::sigv4::sign_request(
                method.as_str(),
                &final_upstream_url,
                &mut upstream_headers,
                &final_body,
                access_key,
                secret_key,
                &region,
                "bedrock",
            )
            .map_err(|e| AppError::Internal(anyhow::anyhow!("SigV4 signing failed: {}", e)))?;

            tracing::debug!(
                region = %region,
                "SigV4 request signed for Bedrock"
            );
        }
    }

    // Credential zeroization is deferred until after MCP tool loop (if active)
    // so that continuation requests can reuse the same auth headers.
    // If no MCP loop is needed, zeroize immediately.
    let mut injected_cred = injected_cred;
    if mcp_server_names.is_empty() {
        if let Some(ref mut cred) = injected_cred {
            use zeroize::Zeroize;
            cred.key.zeroize();
        }
    }

    // -- 5.1 Resolve Retry Config --
    // Use the first policy that specifies a retry config, or default
    let mut retry_config = policies
        .iter()
        .find_map(|p| p.retry.clone())
        .unwrap_or_default();

    // P1.7: Idempotency safety guard — skip retries for mutating requests without an idempotency key.
    // Retrying POST/PUT/PATCH without idempotency guarantees can cause duplicate charges/operations.
    let is_mutating = matches!(method, Method::POST | Method::PUT | Method::PATCH);
    let has_idempotency_key =
        headers.contains_key("idempotency-key") || headers.contains_key("x-idempotency-key");
    let idempotency_warning = if is_mutating && !has_idempotency_key && retry_config.max_retries > 0
    {
        retry_config.max_retries = 0; // Disable retries for safety
        Some("POST/PUT/PATCH request not retried (no Idempotency-Key header). Add 'Idempotency-Key: <unique-id>' to enable safe retries.")
    } else {
        None
    };

    // Forward with explicit timeout safety — scale for retries
    // For streaming requests, use forward_raw (no retry, returns raw response for piping)
    let safety_secs =
        65 + (retry_config.max_retries as u64 * (retry_config.max_backoff_ms / 1000 + 65));
    let upstream_resp = if is_streaming_req {
        // Streaming: no retry, direct connection
        match tokio::time::timeout(
            Duration::from_secs(safety_secs),
            state.upstream_client.forward_raw(
                reqwest_method,
                &final_upstream_url,
                upstream_headers,
                bytes::Bytes::from(final_body),
            ),
        )
        .await
        {
            Ok(Ok(res)) => {
                // FIX 4A-1: Only mark healthy if upstream DID NOT return 5xx.
                // 5xx = upstream is broken → open the circuit.
                // 4xx = upstream is alive (client error) → keep circuit closed.
                if res.status().is_server_error() {
                    state
                        .lb
                        .mark_failed(&token.id, &final_upstream_url, &cb_config);
                } else {
                    state.lb.mark_healthy(&token.id, &final_upstream_url);
                }
                state.lb.decrement_in_flight(&final_upstream_url);
                res
            }
            Ok(Err(e)) => {
                tracing::error!("Upstream streaming request failed: {}", e);
                state
                    .lb
                    .mark_failed(&token.id, &final_upstream_url, &cb_config);
                state.lb.decrement_in_flight(&final_upstream_url);
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &upstream_url,
                    &policies,
                    hitl_required,
                    hitl_decision,
                    hitl_latency_ms,
                    user_id.clone(),
                    tenant_id.clone(),
                    external_request_id.clone(),
                    session_id.clone(),
                    parent_span_id.clone(),
                    custom_properties.clone(),
                );
                audit.upstream_status = Some(502);
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.is_streaming = true;
                audit.emit(&state);
                return Err(e);
            }
            Err(_) => {
                tracing::error!("Upstream streaming request timed out (safety net)");
                state
                    .lb
                    .mark_failed(&token.id, &final_upstream_url, &cb_config);
                state.lb.decrement_in_flight(&final_upstream_url);
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &upstream_url,
                    &policies,
                    hitl_required,
                    hitl_decision,
                    hitl_latency_ms,
                    user_id,
                    tenant_id,
                    external_request_id,
                    session_id,
                    parent_span_id,
                    custom_properties.clone(),
                );
                audit.upstream_status = Some(504);
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.is_streaming = true;
                audit.emit(&state);
                return Err(AppError::Upstream(
                    "Upstream streaming request timed out".to_string(),
                ));
            }
        }
    } else {
        match tokio::time::timeout(
            Duration::from_secs(safety_secs),
            state.upstream_client.forward(
                reqwest_method,
                &final_upstream_url,
                upstream_headers,
                bytes::Bytes::from(final_body),
                &retry_config,
            ),
        )
        .await
        {
            Ok(Ok(res)) => {
                // FIX 4A-1: Only mark healthy if upstream DID NOT return 5xx.
                // 5xx = upstream is broken → open the circuit.
                // 4xx = upstream is alive (client error) → keep circuit closed.
                if res.status().is_server_error() {
                    state
                        .lb
                        .mark_failed(&token.id, &final_upstream_url, &cb_config);
                } else {
                    state.lb.mark_healthy(&token.id, &final_upstream_url);
                }
                state.lb.decrement_in_flight(&final_upstream_url);
                res
            }
            Ok(Err(e)) => {
                tracing::error!("Upstream request failed: {}", e);
                // Loadbalancer: mark upstream as failed
                state
                    .lb
                    .mark_failed(&token.id, &final_upstream_url, &cb_config);
                state.lb.decrement_in_flight(&final_upstream_url);
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &upstream_url,
                    &policies,
                    hitl_required,
                    hitl_decision,
                    hitl_latency_ms,
                    user_id.clone(),
                    tenant_id.clone(),
                    external_request_id.clone(),
                    session_id.clone(),
                    parent_span_id.clone(),
                    custom_properties.clone(),
                );
                audit.policy_result = Some(if hitl_required {
                    crate::models::audit::PolicyResult::HitlApproved
                } else {
                    crate::models::audit::PolicyResult::Allow
                });
                audit.upstream_status = Some(502);
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.shadow_violations = if shadow_violations.is_empty() {
                    None
                } else {
                    Some(shadow_violations)
                };
                audit.is_streaming = is_streaming_req;
                audit.emit(&state);
                return Err(e);
            }
            Err(_) => {
                tracing::error!("Upstream request timed out (safety net)");
                // Loadbalancer: mark upstream as failed
                state
                    .lb
                    .mark_failed(&token.id, &final_upstream_url, &cb_config);
                state.lb.decrement_in_flight(&final_upstream_url);
                let mut audit = base_audit(
                    request_id,
                    token.project_id,
                    &token.id,
                    agent_name,
                    method.as_str(),
                    &path,
                    &upstream_url,
                    &policies,
                    hitl_required,
                    hitl_decision,
                    hitl_latency_ms,
                    user_id,
                    tenant_id,
                    external_request_id,
                    session_id,
                    parent_span_id,
                    custom_properties.clone(),
                );
                audit.policy_result = Some(if hitl_required {
                    crate::models::audit::PolicyResult::HitlApproved
                } else {
                    crate::models::audit::PolicyResult::Allow
                });
                audit.upstream_status = Some(504);
                audit.response_latency_ms = start.elapsed().as_millis() as u64;
                audit.shadow_violations = if shadow_violations.is_empty() {
                    None
                } else {
                    Some(shadow_violations)
                };
                audit.is_streaming = is_streaming_req;
                audit.emit(&state);
                return Err(AppError::Upstream("Upstream request timed out".to_string()));
            }
        }
    };

    let status = upstream_resp.status();
    let resp_headers = upstream_resp.headers().clone();

    // ── STREAMING FAST PATH: zero-copy SSE passthrough ──────────────────────
    // For successful streaming responses, pipe bytes directly to the client.
    // Audit, cost tracking, and sanitization happen in a background task.
    if is_streaming_req && status.is_success() {
        // FIX(X2/X3): Route non-OpenAI providers through translating bridges.
        // - Bedrock: binary event stream → OpenAI SSE (dedicated decoder)
        // - Anthropic: Anthropic SSE → OpenAI SSE (per-chunk translation)
        // - Gemini: Gemini SSE → OpenAI SSE (per-chunk translation)
        // - All others: OpenAI-compatible, passthrough SSE unchanged
        let (stream_body, result_slot, stream_notify) = match detected_provider {
            proxy::model_router::Provider::Bedrock => proxy::stream_bridge::tee_bedrock_stream(
                upstream_resp,
                start,
                detected_model.clone(),
            ),
            proxy::model_router::Provider::Anthropic => {
                proxy::stream_bridge::tee_translating_sse_stream(
                    upstream_resp,
                    start,
                    detected_model.clone(),
                    proxy::model_router::translate_anthropic_sse_to_openai,
                )
            }
            proxy::model_router::Provider::Gemini => {
                proxy::stream_bridge::tee_translating_sse_stream(
                    upstream_resp,
                    start,
                    detected_model.clone(),
                    proxy::model_router::translate_gemini_sse_to_openai,
                )
            }
            _ => proxy::stream_bridge::tee_sse_stream(upstream_resp, start),
        };

        // Build the SSE response immediately — this starts streaming to the client
        let mut sse_response = axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("x-accel-buffering", "no") // Disable nginx buffering
            .body(stream_body)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("stream response build: {}", e)))?;

        // Forward safe upstream headers (skip hop-by-hop headers)
        for (key, value) in resp_headers.iter() {
            let name_str = key.as_str();
            if matches!(
                name_str,
                "content-length" | "transfer-encoding" | "connection"
            ) {
                continue;
            }
            if let Ok(name) = axum::http::HeaderName::from_bytes(name_str.as_bytes()) {
                if let Ok(val) = axum::http::HeaderValue::from_bytes(value.as_bytes()) {
                    sse_response.headers_mut().insert(name, val);
                }
            }
        }

        // Spawn background task: wait for stream to finish, then audit + cost
        let state_bg = state.clone();
        // Extract needed token fields (TokenRow doesn't implement Clone)
        let token_bg_id = token.id.clone();
        let token_bg_project_id = token.project_id;
        let token_bg_upstream_url = token.upstream_url.clone();
        let policies_bg = policies.clone();
        let shadow_violations_bg = shadow_violations.clone();
        let upstream_url_bg = upstream_url.clone();
        let path_bg = path.clone();
        let user_id_bg = user_id.clone();
        let tenant_id_bg = tenant_id.clone();
        let ext_req_id_bg = external_request_id.clone();
        let session_id_bg = session_id.clone();
        let session_id_for_spend = session_id.clone();
        let parent_span_id_bg = parent_span_id.clone();

        tokio::spawn(async move {
            // Wait up to 5 minutes for the stream to complete
            let sr = proxy::stream_bridge::wait_for_stream_result(
                &result_slot,
                &stream_notify,
                Duration::from_secs(300),
            )
            .await;

            let (prompt_tokens, completion_tokens, model_name, finish_reason, tool_calls, ttft_ms) =
                if let Some(ref r) = sr {
                    (
                        r.prompt_tokens,
                        r.completion_tokens,
                        r.model.clone(),
                        r.finish_reason.clone(),
                        r.tool_calls.clone(),
                        r.ttft_ms,
                    )
                } else {
                    (None, None, None, None, vec![], None)
                };

            // Cost tracking — BUG-2 FIX: use DB-backed pricing cache (with hardcoded fallback)
            let mut estimated_cost_usd: Option<rust_decimal::Decimal> = None;
            if let (Some(inp), Some(out)) = (prompt_tokens, completion_tokens) {
                // GAP-1 FIX: detect all supported providers, not just openai/anthropic
                let provider = if token_bg_upstream_url.contains("anthropic")
                    && !token_bg_upstream_url.contains("bedrock")
                {
                    "anthropic"
                } else if token_bg_upstream_url.contains("generativelanguage")
                    || token_bg_upstream_url.contains("googleapis")
                {
                    "google"
                } else if token_bg_upstream_url.contains("mistral") {
                    "mistral"
                } else if token_bg_upstream_url.contains("bedrock") {
                    "bedrock"
                } else if token_bg_upstream_url.contains("groq") {
                    "groq"
                } else if token_bg_upstream_url.contains("cohere") {
                    "cohere"
                } else if token_bg_upstream_url.contains("together") {
                    "together"
                } else if token_bg_upstream_url.contains("localhost:11434")
                    || token_bg_upstream_url.contains("ollama")
                {
                    "ollama"
                } else {
                    "openai"
                };
                let model = model_name.as_deref().unwrap_or("unknown");
                let final_cost =
                    cost::calculate_cost_with_cache(&state_bg.pricing, provider, model, inp, out)
                        .await;
                if !final_cost.is_zero() {
                    estimated_cost_usd = Some(final_cost);
                    let cost_f64 = final_cost.to_f64().unwrap_or(0.0);
                    if let Err(e) = middleware::spend::check_and_increment_spend(
                        &state_bg.cache,
                        state_bg.db.pool(),
                        &token_bg_id,
                        cost_f64,
                    )
                    .await
                    {
                        tracing::error!("Streaming: spend cap exceeded or tracking failed: {}", e);
                    }
                }
            }

            // Sanitize accumulated content for audit log
            let full_content = sr.as_ref().map(|r| r.content.clone()).unwrap_or_default();
            let sanitized_content = middleware::sanitize::sanitize_stream_content(&full_content);

            // Emit audit log
            let mut audit = base_audit(
                request_id,
                token_bg_project_id,
                &token_bg_id,
                agent_name,
                method.as_str(),
                &path_bg,
                &upstream_url_bg,
                &policies_bg,
                hitl_required,
                hitl_decision,
                hitl_latency_ms,
                user_id_bg,
                tenant_id_bg,
                ext_req_id_bg,
                session_id_bg,
                parent_span_id_bg,
                custom_properties.clone(),
            );
            audit.policy_result = Some(if hitl_required {
                crate::models::audit::PolicyResult::HitlApproved
            } else {
                crate::models::audit::PolicyResult::Allow
            });
            audit.upstream_status = Some(200);
            audit.response_latency_ms = start.elapsed().as_millis() as u64;
            audit.is_streaming = true;
            audit.prompt_tokens = prompt_tokens;
            audit.completion_tokens = completion_tokens;
            audit.model = model_name;
            audit.finish_reason = finish_reason;
            // Serialize tool calls to JSON Value for audit storage
            let tool_calls_json = if tool_calls.is_empty() {
                None
            } else {
                serde_json::to_value(&tool_calls).ok()
            };
            audit.tool_calls = tool_calls_json;
            audit.tool_call_count = tool_calls.len() as u16;
            audit.ttft_ms = ttft_ms;
            audit.estimated_cost_usd = estimated_cost_usd;
            audit.fields_redacted = if sanitized_content.redacted_types.is_empty() {
                None
            } else {
                Some(sanitized_content.redacted_types)
            };
            audit.shadow_violations = if shadow_violations_bg.is_empty() {
                None
            } else {
                Some(shadow_violations_bg)
            };
            audit.emit(&state_bg);

            // -- Session spend increment (streaming) --
            if let Some(ref sid) = session_id_for_spend {
                let cost = estimated_cost_usd.unwrap_or_default();
                let tokens =
                    prompt_tokens.unwrap_or(0) as i64 + completion_tokens.unwrap_or(0) as i64;
                if let Err(e) = state_bg
                    .db
                    .increment_session_spend(sid, token_bg_project_id, cost, tokens)
                    .await
                {
                    tracing::warn!(session_id = %sid, error = %e, "Failed to increment session spend (streaming)");
                }
            }
        });

        return Ok(sse_response);
    }

    // ── NON-STREAMING PATH (buffered) ────────────────────────────────────────
    let resp_body = upstream_resp
        .bytes()
        .await
        .map_err(|e| AppError::Upstream(format!("upstream body read failed: {}", e)))?;

    // -- 5.5 Post-flight policy evaluation --
    let mut resp_body_vec = resp_body.to_vec();

    // ── Universal Model Router: translate JSON responses ──
    // BUG-01 FIX: Removed dead `is_streaming_req` branch — streaming + success
    // requests are handled by the fast path (line 1948) and never reach here.
    if status.is_success() {
        // Non-streaming JSON response: translate to OpenAI format
        if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&resp_body_vec) {
            if let Some(translated) =
                proxy::model_router::translate_response(detected_provider, &parsed, &detected_model)
            {
                resp_body_vec = serde_json::to_vec(&translated).unwrap_or(resp_body_vec);
            }
        }
    } else {
        // Error response (4xx/5xx): normalize to OpenAI error format for non-OpenAI providers
        if let Some(normalized) =
            proxy::model_router::normalize_error_response(detected_provider, &resp_body_vec)
        {
            tracing::debug!(
                provider = ?detected_provider,
                status = %status,
                "normalizing upstream error response to OpenAI format"
            );
            resp_body_vec = serde_json::to_vec(&normalized).unwrap_or(resp_body_vec);
        }
    }

    // ── MCP Tool Execution Loop (Phase 2) ─────────────────────────────────────
    // If the LLM response contains MCP tool calls (finish_reason == "tool_calls"),
    // execute those tools via the MCP registry, build continuation messages,
    // and re-send to the LLM. Repeat until no more MCP tool calls or max iterations.
    let mut mcp_loop_iterations: u16 = 0;
    // Track cumulative token usage across all MCP loop iterations
    let mut mcp_cumulative_prompt_tokens: u32 = 0;
    let mut mcp_cumulative_completion_tokens: u32 = 0;
    if !mcp_server_names.is_empty() && status.is_success() {
        let max_iters = crate::middleware::mcp::MAX_TOOL_LOOP_ITERATIONS;

        for iteration in 0..max_iters {
            // Parse current response
            let current_resp: serde_json::Value = match serde_json::from_slice(&resp_body_vec) {
                Ok(v) => v,
                Err(_) => break,
            };

            // Check if LLM wants to call MCP tools
            if !crate::middleware::mcp::has_mcp_tool_calls(&current_resp) {
                break;
            }

            tracing::info!(
                iteration = iteration,
                "MCP tool loop: LLM requested tool calls, executing"
            );

            // Extract and filter tool calls
            let all_calls = crate::middleware::mcp::extract_mcp_tool_calls(&current_resp);
            let (permitted, denied) = crate::middleware::mcp::filter_mcp_tool_calls(
                all_calls,
                mcp_allowed.as_deref(),
                mcp_blocked.as_deref(),
            );

            if permitted.is_empty() && denied.is_empty() {
                break;
            }

            // Execute permitted MCP tool calls
            let tool_messages = crate::middleware::mcp::execute_mcp_tool_calls(
                &state.mcp_registry,
                permitted,
                &current_resp,
                token.project_id,
            )
            .await;

            let Some(mut new_messages) = tool_messages else {
                break;
            };

            // Add error results for denied tool calls so LLM knows they were blocked
            for denied_call in &denied {
                new_messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": denied_call.tool_call_id,
                    "content": format!("Error: Tool '{}' on server '{}' is blocked by token policy.",
                        denied_call.tool_name, denied_call.server_name),
                }));
            }

            // Build continuation request body
            let req_body_val = parsed_body
                .as_ref()
                .cloned()
                .unwrap_or(serde_json::json!({"messages": []}));
            let Some(continuation_body) =
                crate::middleware::mcp::build_continuation_body(&req_body_val, &new_messages)
            else {
                tracing::warn!("MCP tool loop: failed to build continuation body");
                break;
            };

            // Update parsed_body for the next iteration (so messages accumulate)
            parsed_body = serde_json::from_slice(&continuation_body).ok();

            // Re-send to the LLM with the same upstream URL and headers
            let loop_headers = mcp_upstream_headers.clone().unwrap_or_default();
            let loop_method = reqwest::Method::POST;
            let no_retry = crate::models::policy::RetryConfig::default();

            let loop_resp = match state
                .upstream_client
                .forward(
                    loop_method,
                    &final_upstream_url,
                    loop_headers,
                    bytes::Bytes::from(continuation_body),
                    &no_retry,
                )
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::error!(iteration = iteration, error = %e, "MCP tool loop: upstream request failed");
                    break;
                }
            };

            let loop_status = loop_resp.status();
            let loop_body = match loop_resp.bytes().await {
                Ok(b) => b.to_vec(),
                Err(e) => {
                    tracing::error!("MCP tool loop: failed to read response body: {}", e);
                    break;
                }
            };

            if !loop_status.is_success() {
                tracing::warn!(
                    iteration = iteration,
                    status = %loop_status,
                    "MCP tool loop: upstream returned non-success, stopping loop"
                );
                resp_body_vec = loop_body;
                break;
            }

            // Translate response if non-OpenAI provider
            resp_body_vec =
                if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&loop_body) {
                    if let Some(translated) = proxy::model_router::translate_response(
                        detected_provider,
                        &parsed,
                        &detected_model,
                    ) {
                        serde_json::to_vec(&translated).unwrap_or(loop_body)
                    } else {
                        loop_body
                    }
                } else {
                    loop_body
                };

            // SECURITY: Accumulate token usage for cumulative billing across MCP loop iterations
            // This ensures all LLM calls within the tool loop are billed to the token
            if let Ok(Some((iter_prompt, iter_completion))) =
                cost::extract_usage(&token.upstream_url, &resp_body_vec)
            {
                mcp_cumulative_prompt_tokens += iter_prompt;
                mcp_cumulative_completion_tokens += iter_completion;

                // Convert Provider enum to string for cost calculation
                let provider_str = match detected_provider {
                    proxy::model_router::Provider::Anthropic => "anthropic",
                    proxy::model_router::Provider::Gemini => "google",
                    proxy::model_router::Provider::Mistral => "mistral",
                    proxy::model_router::Provider::Bedrock => "bedrock",
                    proxy::model_router::Provider::Groq => "groq",
                    proxy::model_router::Provider::Cohere => "cohere",
                    proxy::model_router::Provider::TogetherAI => "together",
                    proxy::model_router::Provider::Ollama => "ollama",
                    proxy::model_router::Provider::AzureOpenAI => "openai",
                    proxy::model_router::Provider::OpenAI => "openai",
                    proxy::model_router::Provider::Unknown => "openai",
                };

                // Calculate and immediately bill for this iteration's cost
                let iter_cost = cost::calculate_cost_with_cache(
                    &state.pricing,
                    provider_str,
                    &detected_model,
                    iter_prompt,
                    iter_completion,
                )
                .await;

                if !iter_cost.is_zero() {
                    let cost_f64 = iter_cost.to_f64().unwrap_or(0.0);
                    if let Err(e) = middleware::spend::check_and_increment_spend(
                        &state.cache,
                        state.db.pool(),
                        &token.id,
                        cost_f64,
                    )
                    .await
                    {
                        tracing::error!(
                            iteration = iteration,
                            cost = %cost_f64,
                            error = %e,
                            "MCP loop: spend cap exceeded or tracking failed"
                        );
                    }
                }

                tracing::debug!(
                    iteration = iteration,
                    prompt_tokens = iter_prompt,
                    completion_tokens = iter_completion,
                    cumulative_prompt = mcp_cumulative_prompt_tokens,
                    cumulative_completion = mcp_cumulative_completion_tokens,
                    "MCP loop: accumulated token usage"
                );
            }

            mcp_loop_iterations = (iteration + 1) as u16;
        }

        // Zeroize credential now that MCP loop is done
        if let Some(ref mut cred) = injected_cred {
            use zeroize::Zeroize;
            cred.key.zeroize();
        }

        if mcp_loop_iterations > 0 {
            tracing::info!(iterations = mcp_loop_iterations, "MCP tool loop completed");
        }
    }

    let parsed_resp_body: Option<serde_json::Value> = serde_json::from_slice(&resp_body_vec).ok();

    // Convert reqwest headers to axum headers for RequestContext
    let axum_resp_headers = {
        let mut h = HeaderMap::new();
        for (key, value) in resp_headers.iter() {
            if let Ok(name) = axum::http::HeaderName::from_bytes(key.as_str().as_bytes()) {
                if let Ok(val) = axum::http::HeaderValue::from_bytes(value.as_bytes()) {
                    h.insert(name, val);
                }
            }
        }
        h
    };

    // -- 6. Sanitize response (moved before post-flight to support cost tracking) --
    let content_type = resp_headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");
    let sanitization_result = middleware::sanitize::sanitize_response(&resp_body_vec, content_type);
    let sanitized_body = sanitization_result.body;

    // -- 6.1 Calculate Cost & Update Spend --
    // MOVED BEFORE post-flight: the upstream was already called and tokens consumed.
    // Post-flight denials (ContentFilter, ValidateSchema, ExternalGuardrail, Deny)
    // must NOT skip billing — the provider charges regardless.
    let mut estimated_cost_usd = None;
    let mut audit_prompt_tokens: Option<u32> = None;
    let mut audit_completion_tokens: Option<u32> = None;
    let mut audit_model: Option<String> = None;

    // TEST HOOK: Allow forcing cost/tokens via header for deterministic testing
    if let Some((p, c)) = test_tokens_override {
        audit_prompt_tokens = Some(p);
        audit_completion_tokens = Some(c);
        audit_model = Some(detected_model.clone());
    }

    if let Some(cost_val) = test_cost_override {
        estimated_cost_usd = Some(cost_val);
        let cost_f64 = cost_val.to_f64().unwrap_or(0.0);
        // SEC: Even in test mode, log billing failures for observability
        if let Err(e) = middleware::spend::check_and_increment_spend(
            &state.cache,
            state.db.pool(),
            &token.id,
            cost_f64,
        )
        .await
        {
            tracing::warn!("Test hook billing failed: {}", e);
        }
    }

    // FIX: Skip billing if upstream returned 4xx or 5xx error
    if estimated_cost_usd.is_none() && status.is_success() {
        match extract_usage(&token.upstream_url, &sanitized_body) {
            Ok(Some((input, output))) => {
                audit_prompt_tokens = Some(input);
                audit_completion_tokens = Some(output);
                let model = extract_model(&sanitized_body).unwrap_or("unknown".to_string());
                audit_model = Some(model.clone());
                let provider = if token.upstream_url.contains("anthropic")
                    && !token.upstream_url.contains("bedrock")
                {
                    "anthropic"
                } else if token.upstream_url.contains("generativelanguage")
                    || token.upstream_url.contains("googleapis")
                {
                    "google"
                } else if token.upstream_url.contains("mistral") {
                    "mistral"
                } else if token.upstream_url.contains("bedrock") {
                    "bedrock"
                } else if token.upstream_url.contains("groq") {
                    "groq"
                } else if token.upstream_url.contains("cohere") {
                    "cohere"
                } else if token.upstream_url.contains("together") {
                    "together"
                } else if token.upstream_url.contains("localhost:11434")
                    || token.upstream_url.contains("ollama")
                {
                    "ollama"
                } else {
                    "openai"
                };
                let final_cost = cost::calculate_cost_with_cache(
                    &state.pricing,
                    provider,
                    &model,
                    input,
                    output,
                )
                .await;

                if !final_cost.is_zero() {
                    estimated_cost_usd = Some(final_cost);
                    let cost_f64 = final_cost.to_f64().unwrap_or(0.0);
                    if let Err(e) = middleware::spend::check_and_increment_spend(
                        &state.cache,
                        state.db.pool(),
                        &token.id,
                        cost_f64,
                    )
                    .await
                    {
                        tracing::error!("Spend cap exceeded or tracking failed: {}", e);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("Failed to extract usage: {}", e),
        }
    } else if estimated_cost_usd.is_none() && !status.is_success() {
        tracing::debug!(
            status = %status.as_u16(),
            "Skipping billing for non-success upstream response"
        );
    }

    {
        let project_id_str = token.project_id.to_string();
        let post_ctx = RequestContext {
            method: &method,
            path: &path,
            uri: &uri,
            headers: &headers,
            body: parsed_body.as_ref(),
            body_size: body.len(),
            agent_name: agent_name.as_deref(),
            token_id: &token.id,
            token_name: &token.name,
            project_id: &project_id_str,
            client_ip: client_ip_str.as_deref(),
            response_status: Some(status.as_u16()),
            response_body: parsed_resp_body.as_ref(),
            response_headers: Some(&axum_resp_headers),
            usage: usage_counters,
        };

        let post_outcome = middleware::policy::evaluate_post_flight(&policies, &post_ctx);

        // Execute post-flight actions
        for triggered in &post_outcome.actions {
            match &triggered.action {
                Action::Deny { message, .. } => {
                    tracing::warn!(
                        policy = %triggered.policy_name,
                        "post-flight deny: suppressing unsafe response"
                    );
                    return Err(AppError::PolicyDenied {
                        policy: triggered.policy_name.clone(),
                        reason: message.clone(),
                    });
                }
                // SEC: Run regex-heavy redaction on a blocking thread to prevent
                // Tokio worker starvation under large payloads (100KB+).
                Action::Redact { nlp_backend, .. } => {
                    if let Some(resp_json) = parsed_resp_body.clone() {
                        let action_clone = triggered.action.clone();
                        let (returned_body, result) = tokio::task::spawn_blocking(move || {
                            let mut body_owned = resp_json;
                            let r = middleware::redact::apply_redact(
                                &mut body_owned,
                                &action_clone,
                                false,
                            );
                            (body_owned, r)
                        })
                        .await
                        .map_err(|e| {
                            AppError::Internal(anyhow::anyhow!("redact task failed: {}", e))
                        })?;

                        let mut redacted_body = returned_body;

                        // NLP augmentation for post-flight
                        let mut nlp_matched = Vec::new();
                        if let Some(nlp_cfg) = nlp_backend {
                            let timeout = middleware::external_guardrail::guardrail_timeout();
                            let text = middleware::pii::extract_text_from_value(&redacted_body);
                            if !text.is_empty() {
                                let detector = middleware::pii::presidio::PresidioDetector::from_config(nlp_cfg, timeout);
                                let entities = if nlp_cfg.entities.is_empty() {
                                    detector.detect(&text, Some(&nlp_cfg.language)).await
                                } else {
                                    middleware::pii::presidio::detect_with_entities(
                                        &detector, &text, Some(&nlp_cfg.language), &nlp_cfg.entities,
                                    ).await
                                };
                                match entities {
                                    Ok(ents) if !ents.is_empty() => {
                                        nlp_matched = middleware::pii::apply_nlp_entities(&mut redacted_body, &ents);
                                        tracing::info!(
                                            policy = %triggered.policy_name,
                                            nlp_types = ?nlp_matched,
                                            "NLP PII detection augmented response-side redaction"
                                        );
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::warn!(
                                            policy = %triggered.policy_name,
                                            error = %e,
                                            "NLP PII detection failed (fail-open), continuing with regex-only"
                                        );
                                    }
                                }
                            }
                        }

                        let mut all_matched = result.matched_types;
                        all_matched.extend(nlp_matched);

                        if !all_matched.is_empty() {
                            tracing::info!(
                                policy = %triggered.policy_name,
                                patterns = ?all_matched,
                                "applied response-side redaction"
                            );
                            redacted_by_policy.extend(all_matched);
                            // Reserialize the redacted response body
                            if let Ok(new_body) = serde_json::to_vec(&redacted_body) {
                                resp_body_vec = new_body;
                            }
                        }
                    }
                }
                Action::Log { level, tags } => match level.as_str() {
                    "error" => {
                        tracing::error!(policy = %triggered.policy_name, tags = ?tags, "post-flight policy log")
                    }
                    "warn" => {
                        tracing::warn!(policy = %triggered.policy_name, tags = ?tags, "post-flight policy log")
                    }
                    _ => {
                        tracing::info!(policy = %triggered.policy_name, tags = ?tags, "post-flight policy log")
                    }
                },
                Action::Tag { key, value } => {
                    tracing::info!(
                        policy = %triggered.policy_name,
                        tag_key = %key, tag_value = %value,
                        "post-flight policy tag"
                    );
                }
                Action::Webhook {
                    url, timeout_ms, ..
                } => {
                    // SEC: SSRF validation for policy-defined webhook URLs (async DNS resolution)
                    if !is_safe_webhook_url(url).await {
                        tracing::warn!(
                            policy = %triggered.policy_name,
                            url = %url,
                            "post-flight policy webhook blocked: SSRF protection"
                        );
                    } else {
                        let url = url.clone();
                        let timeout_ms = *timeout_ms;
                        let summary = serde_json::json!({
                            "phase": "post",
                            "policy": triggered.policy_name,
                            "response_status": status.as_u16(),
                        });
                        tokio::spawn(async move {
                            let client = reqwest::Client::new();
                            let _ = client
                                .post(&url)
                                .timeout(Duration::from_millis(timeout_ms))
                                .json(&summary)
                                .send()
                                .await;
                        });
                    }
                }

                // ── ContentFilter (post-flight, response-side) ──
                // Scan the LLM response for jailbreak/harmful content, code injection, etc.
                Action::ContentFilter { .. } => {
                    if let Some(ref resp_json) = parsed_resp_body {
                        let result =
                            middleware::guardrail::check_content(resp_json, &triggered.action);
                        if result.blocked {
                            let reason = result
                                .reason
                                .clone()
                                .unwrap_or_else(|| "Output guardrail blocked response".to_string());
                            tracing::warn!(
                                policy = %triggered.policy_name,
                                risk_score = %result.risk_score,
                                patterns = ?result.matched_patterns,
                                "output content filter blocked response"
                            );
                            return Err(AppError::ContentBlocked {
                                reason: reason.clone(),
                                details: Some(serde_json::json!({
                                    "phase": "response",
                                    "policy": triggered.policy_name,
                                    "reason": reason,
                                    "matched_patterns": result.matched_patterns,
                                    "confidence": result.risk_score,
                                })),
                            });
                        } else if !result.matched_patterns.is_empty() {
                            tracing::info!(
                                policy = %triggered.policy_name,
                                risk_score = %result.risk_score,
                                patterns = ?result.matched_patterns,
                                "output content filter: patterns matched but below threshold"
                            );
                        }
                    }
                }

                // ── Transform (post-flight, response-side) ──
                Action::Transform { operations } => {
                    if let Some(mut resp_json) = parsed_resp_body.clone() {
                        let mut resp_header_mutations =
                            middleware::redact::HeaderMutations::default();
                        for op in operations {
                            middleware::redact::apply_transform(
                                &mut resp_json,
                                &mut resp_header_mutations,
                                op,
                            );
                        }
                        tracing::info!(
                            policy = %triggered.policy_name,
                            ops = operations.len(),
                            "applied post-flight transform operations"
                        );
                        if let Ok(new_body) = serde_json::to_vec(&resp_json) {
                            resp_body_vec = new_body;
                        }
                    }
                }

                // ConditionalRoute is request-phase only — skip post-flight
                Action::ConditionalRoute { .. } => {
                    tracing::debug!(
                        policy = %triggered.policy_name,
                        "ConditionalRoute is a request-phase action, skipping post-flight"
                    );
                }

                // ── ValidateSchema (post-flight, response-side) ──
                Action::ValidateSchema {
                    schema,
                    not,
                    message,
                } => {
                    if let Some(ref resp_json) = parsed_resp_body {
                        let result = middleware::guardrail::validate_schema(resp_json, schema);
                        // `not` mode: invert – pass only if validation FAILS
                        let should_deny = if *not { result.valid } else { !result.valid };
                        if should_deny {
                            let default_msg = if *not {
                                "Response matches a forbidden schema pattern".to_string()
                            } else {
                                format!(
                                    "Response failed JSON schema validation: {}",
                                    result.errors.join("; ")
                                )
                            };
                            let reason = message.clone().unwrap_or(default_msg);
                            tracing::warn!(
                                policy = %triggered.policy_name,
                                errors = ?result.errors,
                                not = not,
                                "schema validation blocked response"
                            );
                            return Err(AppError::PolicyDenied {
                                policy: triggered.policy_name.clone(),
                                reason,
                            });
                        } else {
                            tracing::debug!(
                                policy = %triggered.policy_name,
                                not = not,
                                "response passed schema validation"
                            );
                        }
                    }
                }

                Action::ExternalGuardrail {
                    vendor,
                    endpoint,
                    api_key_env,
                    threshold,
                    on_fail,
                } => {
                    let text = parsed_resp_body
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| String::from_utf8_lossy(&resp_body_vec).to_string());
                    match middleware::external_guardrail::check(
                        vendor,
                        endpoint,
                        api_key_env.as_deref(),
                        *threshold,
                        &text,
                    )
                    .await
                    {
                        Ok(result) if result.blocked => {
                            tracing::warn!(
                                policy = %triggered.policy_name,
                                vendor = ?vendor,
                                label = %result.label,
                                score = %result.score,
                                "ExternalGuardrail: post-flight violation detected"
                            );
                            if on_fail != "log" {
                                return Err(AppError::PolicyDenied {
                                    policy: triggered.policy_name.clone(),
                                    reason: format!(
                                        "external_guardrail({:?}): {}",
                                        vendor, result.label
                                    ),
                                });
                            }
                        }
                        Ok(_) => {} // clean
                        Err(e) => {
                            tracing::error!(
                                policy = %triggered.policy_name,
                                vendor = ?vendor,
                                error = %e,
                                "ExternalGuardrail: post-flight vendor call failed (fail-open)"
                            );
                        }
                    }
                }

                _ => {
                    tracing::debug!(
                        policy = %triggered.policy_name,
                        action = ?triggered.action,
                        "post-flight action not applicable"
                    );
                }
            }
        }

        // Collect post-flight shadow violations
        if !post_outcome.shadow_violations.is_empty() {
            shadow_violations.extend(post_outcome.shadow_violations);
        }
    }

    // Section 6 + 6.1 moved before post-flight (see above): spend is recorded
    // before any post-flight denial can skip it.

    // ── Phase 4: Calculate TPS ────────────────────────────────
    let elapsed_secs = start.elapsed().as_secs_f32();
    let tokens_per_second = audit_completion_tokens.map(|ct| {
        if elapsed_secs > 0.0 {
            ct as f32 / elapsed_secs
        } else {
            0.0
        }
    });

    // ── Phase 4: Privacy-gated body capture ───────────────────
    let log_level = token.log_level as u8;
    let (logged_req_body, logged_resp_body, logged_req_headers, logged_resp_headers) =
        match log_level {
            0 => (None, None, None, None),
            1 => {
                // Level 1: Run PII scrubbers on bodies
                let req = middleware::redact::redact_for_logging(&parsed_body);
                let resp = middleware::redact::redact_for_logging(&parsed_resp_body);
                (req, resp, None, None)
            }
            2 => {
                // Level 2: Full debug — store raw bodies + headers (auto-expires in 24h)
                let req = parsed_body
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_default());
                let resp = parsed_resp_body
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_default());
                let req_hdrs = Some(headers_to_json(&headers));
                let resp_hdrs = Some(headers_to_json_reqwest(&resp_headers));
                (req, resp, req_hdrs, resp_hdrs)
            }
            _ => (None, None, None, None),
        };

    // ── Phase 5: LLM Observability extraction ─────────────────
    let llm_tool_calls = parsed_resp_body
        .as_ref()
        .map(crate::models::llm::extract_tool_calls_from_value)
        .unwrap_or_default();
    let llm_finish_reason = parsed_resp_body
        .as_ref()
        .and_then(crate::models::llm::extract_finish_reason_from_value);
    let llm_error_type = if !status.is_success() {
        let body_str = parsed_resp_body
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();
        crate::models::llm::classify_error_from_str(status.as_u16(), &body_str)
    } else {
        None
    };
    let tool_call_count = llm_tool_calls.len() as u16;
    let tool_calls_json = if llm_tool_calls.is_empty() {
        None
    } else {
        serde_json::to_value(&llm_tool_calls).ok()
    };

    // -- 7. Emit audit log --
    let mut audit = base_audit(
        request_id,
        token.project_id,
        &token.id,
        agent_name,
        method.as_str(),
        &path,
        &upstream_url,
        &policies,
        hitl_required,
        hitl_decision,
        hitl_latency_ms,
        user_id,
        tenant_id,
        external_request_id,
        session_id,
        parent_span_id,
        custom_properties.clone(),
    );
    audit.policy_result = Some(if hitl_required {
        crate::models::audit::PolicyResult::HitlApproved
    } else {
        crate::models::audit::PolicyResult::Allow
    });
    audit.upstream_status = Some(status.as_u16());
    audit.response_latency_ms =
        test_latency_override.unwrap_or_else(|| start.elapsed().as_millis() as u64);
    audit.fields_redacted = if sanitization_result.redacted_types.is_empty() {
        None
    } else {
        Some(sanitization_result.redacted_types)
    };
    audit.shadow_violations = if shadow_violations.is_empty() {
        None
    } else {
        Some(shadow_violations)
    };
    audit.estimated_cost_usd = estimated_cost_usd;
    // Phase 4
    audit.log_level = log_level;
    audit.request_body = logged_req_body;
    audit.response_body = logged_resp_body;
    audit.request_headers = logged_req_headers;
    audit.response_headers = logged_resp_headers;
    audit.prompt_tokens = audit_prompt_tokens;
    audit.completion_tokens = audit_completion_tokens;
    let audit_model_for_cache = audit_model.clone();
    audit.model = audit_model;
    audit.tokens_per_second = tokens_per_second;
    // Phase 5
    audit.tool_calls = tool_calls_json;
    audit.tool_call_count = tool_call_count;
    audit.finish_reason = llm_finish_reason;
    audit.error_type = llm_error_type;
    audit.is_streaming = is_streaming_req;
    audit.cache_hit = false; // not a cache hit — we went to upstream
    audit.experiment_name = experiment_name;
    audit.variant_name = variant_name;
    let session_id_for_spend = audit.session_id.clone();
    audit.emit(&state);

    // -- Session spend increment (non-streaming) --
    // session_id was consumed by audit builder above, so we use the clone
    if let Some(ref sid) = session_id_for_spend {
        let cost = estimated_cost_usd.unwrap_or_default();
        let tokens =
            audit_prompt_tokens.unwrap_or(0) as i64 + audit_completion_tokens.unwrap_or(0) as i64;
        let state_for_session = state.clone();
        let sid_owned = sid.clone();
        let project_id = token.project_id;
        tokio::spawn(async move {
            if let Err(e) = state_for_session
                .db
                .increment_session_spend(&sid_owned, project_id, cost, tokens)
                .await
            {
                tracing::warn!(session_id = %sid_owned, error = %e, "Failed to increment session spend");
            }
        });
    }

    // ── Response Cache: store successful, non-streaming responses ──
    if let Some(ref key) = cache_key {
        if status.is_success() {
            let cached = proxy::response_cache::CachedResponse {
                status: status.as_u16(),
                body: sanitized_body.clone(),
                content_type: content_type.to_string(),
                model: audit_model_for_cache,
                prompt_tokens: audit_prompt_tokens,
                completion_tokens: audit_completion_tokens,
            };
            let state_ref = state.clone();
            let key = key.clone();
            tokio::spawn(async move {
                proxy::response_cache::set_cached(
                    &state_ref.cache,
                    &key,
                    &cached,
                    proxy::response_cache::DEFAULT_CACHE_TTL_SECS,
                )
                .await;
            });
        }
    }

    // -- Build response --
    let axum_status =
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut response = Response::builder().status(axum_status);

    for (key, value) in resp_headers.iter() {
        if let Ok(name) = axum::http::HeaderName::from_bytes(key.as_str().as_bytes()) {
            if let Ok(val) = axum::http::HeaderValue::from_bytes(value.as_bytes()) {
                if !matches!(
                    name.as_str(),
                    "server"
                        | "x-request-id"
                        | "x-powered-by"
                        | "content-length"
                        | "transfer-encoding"
                ) {
                    response = response.header(name, val);
                }
            }
        }
    }

    // -- Circuit breaker visibility headers --
    // X-TrueFlow-Upstream: which upstream was selected for this request
    // X-TrueFlow-CB-State: closed | open | half_open | disabled
    // X-Request-Id: correlation ID for debugging and support
    let cb_state: &'static str = if cb_config.enabled {
        state.lb.get_circuit_state(
            &token.id,
            &final_upstream_url,
            cb_config.recovery_cooldown_secs,
        )
    } else {
        "disabled"
    };
    response = response.header(
        "x-trueflow-cb-state",
        axum::http::HeaderValue::from_static(cb_state),
    );
    if let Ok(upstream_hv) = axum::http::HeaderValue::from_str(&final_upstream_url) {
        response = response.header("x-trueflow-upstream", upstream_hv);
    }
    // DynamicRoute observability: tell developers which strategy won and why
    if let Some(ref strategy) = dynamic_route_strategy {
        if let Ok(hv) = axum::http::HeaderValue::from_str(strategy) {
            response = response.header("x-trueflow-route-strategy", hv);
        }
    }
    if let Some(ref reason) = dynamic_route_reason {
        if let Ok(hv) = axum::http::HeaderValue::from_str(reason) {
            response = response.header("x-trueflow-route-reason", hv);
        }
    }
    // Attach request ID to every response for support correlation
    let req_id_str = format!("req_{}", request_id.simple());
    if let Ok(req_id_hv) = axum::http::HeaderValue::from_str(&req_id_str) {
        response = response.header("x-request-id", req_id_hv);
    }

    // -- Budget-remaining headers (best-effort, non-blocking) --
    // SEC-08 FIX: Only emit when log_level >= 1 (opt-in) to avoid leaking financial data
    if log_level >= 1 {
        if let Ok(status) =
            middleware::spend::get_spend_status(state.db.pool(), &state.cache, &token.id).await
        {
            if let Some(daily_limit) = status.daily_limit_usd {
                let remaining = (daily_limit - status.current_daily_usd).max(0.0);
                if let Ok(hv) = axum::http::HeaderValue::from_str(&format!("{:.4}", remaining)) {
                    response = response.header("x-trueflow-budget-remaining-daily", hv);
                }
            }
            if let Some(monthly_limit) = status.monthly_limit_usd {
                let remaining = (monthly_limit - status.current_monthly_usd).max(0.0);
                if let Ok(hv) = axum::http::HeaderValue::from_str(&format!("{:.4}", remaining)) {
                    response = response.header("x-trueflow-budget-remaining-monthly", hv);
                }
            }
            if let Some(lifetime_limit) = status.lifetime_limit_usd {
                let remaining = (lifetime_limit - status.current_lifetime_usd).max(0.0);
                if let Ok(hv) = axum::http::HeaderValue::from_str(&format!("{:.4}", remaining)) {
                    response = response.header("x-trueflow-budget-remaining-lifetime", hv);
                }
            }
        }
    }

    // P1.7: Attach idempotency warning if retries were skipped for safety
    if let Some(warning_msg) = idempotency_warning {
        if let Ok(warning_hv) = axum::http::HeaderValue::from_str(warning_msg) {
            response = response.header("x-trueflow-warning", warning_hv);
        }
    }

    // ── Feature 8: Async Guardrails ──────────────────────────────────────────
    // Spawn background evaluation for rules that opted into async_check=true.
    // The response is committed before these are evaluated — violations are
    // logged and trigger audit/webhook events but cannot block the response.
    if !pre_async_triggered.is_empty() {
        let _state_async = state.clone();
        let token_id_async = token.id.clone();
        let async_triggered = pre_async_triggered;
        // Snapshot the sanitized body for async guardrail content checks
        let async_body_snapshot = parsed_body.clone();
        tokio::spawn(async move {
            for triggered in &async_triggered {
                tracing::info!(
                    token_id = %token_id_async,
                    policy = %triggered.policy_name,
                    rule = triggered.rule_index,
                    action = ?triggered.action,
                    "async guardrail: evaluating non-blocking rule"
                );
                // Re-evaluate content filter / validate_schema actions
                // on the captured body. Other action types (rate_limit, deny,
                // webhook, etc.) are executed directly.
                match &triggered.action {
                    crate::models::policy::Action::ContentFilter { .. } => {
                        if let Some(ref body) = async_body_snapshot {
                            let result = crate::middleware::guardrail::check_content(
                                body,
                                &triggered.action,
                            );
                            if result.blocked {
                                tracing::warn!(
                                    token_id = %token_id_async,
                                    policy = %triggered.policy_name,
                                    risk_score = %result.risk_score,
                                    patterns = ?result.matched_patterns,
                                    "async guardrail: content filter VIOLATION (response already sent)"
                                );
                                // Emit async violation audit
                                let event = crate::models::audit::AsyncGuardrailViolation {
                                    token_id: token_id_async.clone(),
                                    policy_name: triggered.policy_name.clone(),
                                    matched_patterns: result.matched_patterns,
                                    risk_score: result.risk_score,
                                };
                                crate::models::audit::emit_async_violation(event).await;
                            }
                        }
                    }
                    crate::models::policy::Action::ValidateSchema { schema, not, .. } => {
                        if let Some(ref body) = async_body_snapshot {
                            let result =
                                crate::middleware::guardrail::validate_schema(body, schema);
                            let violated = if *not { result.valid } else { !result.valid };
                            if violated {
                                tracing::warn!(
                                    token_id = %token_id_async,
                                    policy = %triggered.policy_name,
                                    errors = ?result.errors,
                                    "async guardrail: schema validation VIOLATION (response already sent)"
                                );
                            }
                        }
                    }
                    crate::models::policy::Action::Webhook {
                        url, timeout_ms, ..
                    } => {
                        // SEC-04 FIX: Validate webhook URL before async fire (async DNS resolution)
                        let w_url = url.clone();
                        if !is_safe_webhook_url(&w_url).await {
                            tracing::warn!(
                                token_id = %token_id_async,
                                policy = %triggered.policy_name,
                                url = %w_url,
                                "async guardrail: blocked unsafe webhook URL (SSRF protection)"
                            );
                            continue;
                        }
                        let w_timeout = *timeout_ms;
                        let _ = reqwest::Client::new()
                            .post(&w_url)
                            .timeout(std::time::Duration::from_millis(w_timeout))
                            .json(&serde_json::json!({"event": "async_guardrail_webhook"}))
                            .send()
                            .await;
                    }
                    _other => {
                        tracing::debug!(
                            token_id = %token_id_async,
                            policy = %triggered.policy_name,
                            "async guardrail: action type not evaluated asynchronously"
                        );
                    }
                }
            }
        });
    }

    response
        .body(Body::from(sanitized_body))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("response build failed: {}", e)))
}

fn extract_bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::TokenNotFound)?;

    if !auth.starts_with("Bearer ") {
        return Err(AppError::TokenNotFound);
    }
    let token = auth[7..].trim().to_string();
    if !token.starts_with("tf_v1_") {
        return Err(AppError::TokenNotFound);
    }
    Ok(token)
}
