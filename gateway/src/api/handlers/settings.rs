use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Extension, Json};

use super::dtos::{RehydrateRequest, UpdateSettingsRequest};
use crate::api::AuthContext;
use crate::AppState;

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<std::collections::HashMap<String, serde_json::Value>>, StatusCode> {
    auth.require_role("admin")?;

    let settings = state.db.get_all_system_settings().await.map_err(|e| {
        tracing::error!("Failed to fetch settings: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(settings))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    // SEC: allowlist of permitted setting keys — prevents arbitrary key injection
    const ALLOWED_KEYS: &[&str] = &[
        "default_rate_limit",
        "default_rate_limit_window",
        "hitl_timeout_minutes",
        "max_request_body_bytes",
        "audit_retention_days",
        "enable_response_cache",
        "enable_guardrails",
        "slack_webhook_url",
    ];

    for key in payload.settings.keys() {
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            tracing::warn!(key = %key, "update_settings: rejected unknown setting key");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    for (key, value) in payload.settings {
        state
            .db
            .set_system_setting(&key, &value, None)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update setting {}: {}", key, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn get_cache_stats(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    let mut conn = state.cache.redis();

    // Count llm_cache:* keys via SCAN (non-blocking)
    let mut cursor: u64 = 0;
    let mut key_count: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut sample_keys: Vec<serde_json::Value> = Vec::new();

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("llm_cache:*")
            .arg("COUNT")
            .arg(200u32)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("get_cache_stats SCAN failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        for key in &keys {
            key_count += 1;
            // Estimate size via STRLEN (works on string values stored as JSON)
            let size: u64 = redis::cmd("STRLEN")
                .arg(key)
                .query_async(&mut conn)
                .await
                .unwrap_or(0u64);
            total_bytes += size;

            // Collect up to 20 sample keys with TTL info for the UI
            if sample_keys.len() < 20 {
                let ttl_secs: i64 = redis::cmd("TTL")
                    .arg(key)
                    .query_async(&mut conn)
                    .await
                    .unwrap_or(-1i64);
                // Key suffix (last 12 chars) for display
                let display_key = if key.len() > 22 {
                    format!("{}…{}", &key[..10], &key[key.len() - 8..])
                } else {
                    key.clone()
                };
                sample_keys.push(serde_json::json!({
                    "key": display_key,
                    "full_key": key,
                    "size_bytes": size,
                    "ttl_secs": ttl_secs,
                }));
            }
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    // Also count other namespaces for context (non-blocking estimates)
    let spend_count: u64 = {
        let (_, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg("spend:*")
            .arg("COUNT")
            .arg(100u32)
            .query_async(&mut conn)
            .await
            .unwrap_or((0u64, vec![]));
        keys.len() as u64
    };

    let rl_count: u64 = {
        let (_, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg("rl:*")
            .arg("COUNT")
            .arg(100u32)
            .query_async(&mut conn)
            .await
            .unwrap_or((0u64, vec![]));
        keys.len() as u64
    };

    Ok(Json(serde_json::json!({
        "cache_key_count": key_count,
        "estimated_size_bytes": total_bytes,
        "default_ttl_secs": crate::proxy::response_cache::DEFAULT_CACHE_TTL_SECS,
        "max_entry_bytes": 256 * 1024,
        "cached_fields": ["model", "messages", "temperature", "max_tokens", "tools", "tool_choice"],
        "skip_conditions": ["temperature > 0.1", "stream: true", "x-trueflow-no-cache: true", "Cache-Control: no-cache/no-store"],
        "namespace_counts": {
            "llm_cache": key_count,
            "spend_tracking": spend_count,
            "rate_limits": rl_count,
        },
        "sample_entries": sample_keys,
    })))
}

pub async fn flush_cache(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    // SEC: Use targeted SCAN+DEL on the `cache:*` namespace ONLY.
    // FLUSHDB was dangerous because it also wiped spend tracking (`spend:*`),
    // rate limit state (`rl:*`), and HITL decisions (`hitl:*`), which would
    // silently reset budget enforcement and bypass rate limits.
    let mut conn = state.cache.redis();

    let mut cursor: u64 = 0;
    let mut deleted: u64 = 0;
    loop {
        // SCAN with match pattern and count hint
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("llm_cache:*")
            .arg("COUNT")
            .arg(200u32)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("flush_cache SCAN failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if !keys.is_empty() {
            let n = keys.len() as u64;
            let _: () = redis::cmd("DEL")
                .arg(keys)
                .query_async(&mut conn)
                .await
                .unwrap_or(());
            deleted += n;
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    tracing::info!(
        user_id = %auth.user_id.unwrap_or_default(),
        keys_deleted = deleted,
        "Response cache (cache:*) flushed — spend/rate-limit/HITL keys preserved"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Response cache flushed successfully",
        "keys_deleted": deleted
    })))
}

// ── PII Tokenization Vault ──────────────────────────────────────────────────

/// POST /api/v1/pii/rehydrate — reverse PII tokens back to original values.
///
/// Requires `pii:rehydrate` scope (PCI-DSS: only authorized callers can see raw PII).
/// Every rehydration request is logged for audit compliance.
pub async fn rehydrate_pii_tokens(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<RehydrateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;
    auth.require_scope("pii:rehydrate")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    if payload.tokens.is_empty() {
        return Ok(Json(serde_json::json!({ "values": {} })));
    }

    // Limit batch size to prevent abuse
    if payload.tokens.len() > 100 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Create a VaultCrypto instance for decryption
    let vault = crate::vault::builtin::VaultCrypto::new(&state.config.master_key).map_err(|e| {
        tracing::error!("VaultCrypto init failed in rehydrate: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let project_id = auth.default_project_id();

    let values = crate::middleware::pii_vault::rehydrate_tokens(
        state.db.pool(),
        &vault,
        &payload.tokens,
        project_id,
    )
    .await
    .map_err(|e| {
        tracing::error!("PII rehydration failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Audit log: record who rehydrated what tokens
    tracing::info!(
        user_id = ?auth.user_id,
        org_id = %auth.org_id,
        token_count = values.len(),
        "PII tokens rehydrated"
    );

    Ok(Json(serde_json::json!({
        "values": values,
        "token_count": values.len(),
    })))
}

// ── Anomaly Detection Events ─────────────────────────────────

/// GET /api/v1/anomalies — list recent anomaly velocity data per token.
///
/// Scans Redis `anomaly:tok:*` sorted sets, computes current velocity
/// vs. baseline for each token, and returns results.
pub async fn get_anomaly_events(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_role("admin")?;

    let mut conn = state.cache.redis();
    let config = crate::middleware::anomaly::AnomalyConfig::default();
    let now = chrono::Utc::now().timestamp() as f64;

    // SCAN for anomaly keys
    let mut cursor: u64 = 0;
    let mut events: Vec<serde_json::Value> = Vec::new();

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("anomaly:tok:*")
            .arg("COUNT")
            .arg(200u32)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("anomaly SCAN failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        for key in &keys {
            let token_id = key.strip_prefix("anomaly:tok:").unwrap_or(key);

            // Current window velocity
            let window_start = now - config.window_secs as f64;
            let current_velocity: u64 = redis::cmd("ZCOUNT")
                .arg(key)
                .arg(window_start)
                .arg(now)
                .query_async(&mut conn)
                .await
                .unwrap_or(0);

            // Total data points for baseline
            let cutoff = now - config.baseline_secs as f64;
            let total_points: u64 = redis::cmd("ZCOUNT")
                .arg(key)
                .arg(cutoff)
                .arg(now)
                .query_async(&mut conn)
                .await
                .unwrap_or(0);

            // Simple baseline estimate: total / number of windows
            let num_windows = (config.baseline_secs / config.window_secs) as f64;
            let baseline_mean = if num_windows > 0.0 {
                total_points as f64 / num_windows
            } else {
                0.0
            };

            let threshold = baseline_mean + config.sigma_threshold * baseline_mean.sqrt();
            let is_anomalous =
                current_velocity as f64 > threshold && total_points >= config.min_datapoints as u64;

            events.push(serde_json::json!({
                "token_id": token_id,
                "current_velocity": current_velocity,
                "baseline_mean": (baseline_mean * 100.0).round() / 100.0,
                "threshold": (threshold * 100.0).round() / 100.0,
                "is_anomalous": is_anomalous,
                "window_secs": config.window_secs,
                "total_data_points": total_points,
            }));
        }

        cursor = next_cursor;
        if cursor == 0 || events.len() >= 100 {
            break;
        }
    }

    // Sort: anomalous first, then by velocity desc
    events.sort_by(|a, b| {
        let a_anom = a["is_anomalous"].as_bool().unwrap_or(false);
        let b_anom = b["is_anomalous"].as_bool().unwrap_or(false);
        if a_anom != b_anom {
            return b_anom.cmp(&a_anom);
        }
        let a_vel = a["current_velocity"].as_u64().unwrap_or(0);
        let b_vel = b["current_velocity"].as_u64().unwrap_or(0);
        b_vel.cmp(&a_vel)
    });

    Ok(Json(serde_json::json!({
        "events": events,
        "total": events.len(),
        "window_secs": config.window_secs,
        "sigma_threshold": config.sigma_threshold,
    })))
}
