//! Realtime WebSocket Proxy
//!
//! Proxies WebSocket connections from clients to OpenAI's Realtime API
//! (or other providers that support the OpenAI Realtime protocol).
//!
//! Route: GET /v1/realtime
//!
//! The handler:
//!   1. Validates the token (same auth as the REST proxy)
//!   2. Decrypts the credential from the vault
//!   3. Performs HTTP→WebSocket upgrade with the client
//!   4. Opens a second WS connection to the upstream with credentials injected
//!   5. Runs a bidirectional relay loop (client ↔ upstream)
//!   6. Logs the session summary on close

use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{handshake::client::Request, Message},
};

use crate::middleware;
use crate::vault::SecretStore;
use crate::AppState;

// ── Query params ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RealtimeQuery {
    /// The model for the Realtime session.
    /// Defaults to "gpt-4o-realtime-preview-2024-12-17"
    #[serde(default = "default_realtime_model")]
    pub model: String,
}

fn default_realtime_model() -> String {
    "gpt-4o-realtime-preview-2024-12-17".to_string()
}

// ── Handler ───────────────────────────────────────────────────

/// GET /v1/realtime
///
/// Upgrades the connection to WebSocket and proxies to the upstream
/// realtime API. Authentication uses the same `Authorization: Bearer <token>`
/// header as the REST proxy.
pub async fn realtime_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RealtimeQuery>,
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    // ── 1. Authenticate token ─────────────────────────────────
    let bearer = headers
        .get("authorization")
        .or_else(|| headers.get("Authorization"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = state
        .db
        .get_token(&bearer)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !token.is_active {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // ── 2. Resolve upstream URL and credential ─────────────────
    let upstream_base = token.upstream_url.clone();
    let model = params.model.clone();
    let token_id = token.id.clone();

    // Build the upstream WSS URL
    let upstream_ws_url = build_realtime_url(&upstream_base, &model);

    // Resolve credential via vault
    let api_key: Option<String> = if let Some(cred_id) = token.credential_id {
        match state.vault.retrieve(&cred_id.to_string()).await {
            Ok((plaintext, _provider, mode, header)) => {
                // For "bearer"/"header" mode, use the key directly
                // For "basic" mode, use as-is (caller can base64-encode if needed)
                let _ = (mode, header); // consumed for logging if needed
                Some(plaintext)
            }
            Err(e) => {
                tracing::error!(token_id = %token_id, "realtime: credential decrypt error: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    } else {
        // Passthrough: client must supply the key via X-Real-Authorization
        headers
            .get("x-real-authorization")
            .or_else(|| headers.get("X-Real-Authorization"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer ").or(Some(v)))
            .map(|k| k.to_string())
    };

    let api_key = api_key.ok_or_else(|| {
        tracing::warn!(token_id = %token_id, "realtime: no API key available");
        StatusCode::UNAUTHORIZED
    })?;

    // ── B6-3 FIX: Pre-flight policy enforcement ───────────────
    // Rate limit check (same mechanism as REST proxy)
    if state.config.default_rate_limit > 0 {
        let rl_key = format!("rl:realtime:tok:{}", token_id);
        let count = state
            .cache
            .increment(&rl_key, state.config.default_rate_limit_window)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if count > state.config.default_rate_limit {
            tracing::warn!(token_id = %token_id, "realtime: rate limit exceeded");
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    // Spend cap check
    if let Err(e) =
        middleware::spend::check_spend_cap(&state.cache, state.db.pool(), &token_id).await
    {
        tracing::warn!(token_id = %token_id, error = %e, "realtime: spend cap exceeded");
        return Err(StatusCode::PAYMENT_REQUIRED);
    }

    // ── 3. Upgrade client connection and start relay ───────────
    let upstream_ws_url2 = upstream_ws_url.clone();
    let token_id2 = token_id.clone();
    let project_id = token.project_id;
    let state_clone = state.clone();

    Ok(ws.on_upgrade(move |client_ws| async move {
        if let Err(e) = relay(client_ws, &upstream_ws_url2, &api_key, &token_id2, &model, project_id, &state_clone).await {
            tracing::warn!(token_id = %token_id2, url = %upstream_ws_url2, "realtime relay ended: {}", e);
        }
    }))
}

// ── Relay ─────────────────────────────────────────────────────

async fn relay(
    client_ws: axum::extract::ws::WebSocket,
    upstream_url: &str,
    api_key: &str,
    token_id: &str,
    model: &str,
    project_id: uuid::Uuid,
    _state: &Arc<AppState>,
) -> anyhow::Result<()> {
    let session_start = Instant::now();

    // ── Connect to upstream with credential injection ──────────
    let request = Request::builder()
        .uri(upstream_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("OpenAI-Beta", "realtime=v1")
        .header("User-Agent", "TrueFlow-Gateway/1.0")
        .body(())?;

    // Use the native-tls connector bundled with tokio-tungstenite
    let (upstream_ws, _resp) = connect_async_tls_with_config(request, None, false, None).await?;

    tracing::info!(
        token_id = %token_id,
        url = %upstream_url,
        "realtime: upstream connected"
    );

    // ── Split both ends ────────────────────────────────────────
    let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();
    let (mut client_sink, mut client_stream) = client_ws.split();

    let mut msg_count_client: u64 = 0;
    let mut msg_count_upstream: u64 = 0;

    // Forward client → upstream and upstream → client concurrently
    let client_to_upstream = async {
        while let Some(Ok(msg)) = client_stream.next().await {
            if let Some(m) = axum_to_tungstenite(msg) {
                msg_count_client += 1;
                if upstream_sink.send(m).await.is_err() {
                    break;
                }
            } else {
                break; // Close frame
            }
        }
        let _ = upstream_sink.close().await;
    };

    let upstream_to_client = async {
        while let Some(Ok(msg)) = upstream_stream.next().await {
            msg_count_upstream += 1;
            let axum_msg = tungstenite_to_axum(msg);
            if client_sink.send(axum_msg).await.is_err() {
                break;
            }
        }
    };

    // Run both directions concurrently; stop when either ends
    tokio::select! {
        _ = client_to_upstream => {},
        _ = upstream_to_client => {},
    }

    let session_duration_ms = session_start.elapsed().as_millis() as u64;

    tracing::info!(
        token_id = %token_id,
        client_msgs = msg_count_client,
        upstream_msgs = msg_count_upstream,
        duration_ms = session_duration_ms,
        "realtime: session ended"
    );

    // ── B6-2 FIX: Emit audit log entry for the realtime session ──
    // Structured log captured by observability pipeline (Langfuse, Datadog, Prometheus)
    tracing::info!(
        audit_type = "realtime_session",
        token_id = %token_id,
        project_id = %project_id,
        model = %model,
        upstream_url = %upstream_url,
        client_msgs = msg_count_client,
        upstream_msgs = msg_count_upstream,
        duration_ms = session_duration_ms,
        "realtime: session audit"
    );

    Ok(())
}

// ── URL builder ───────────────────────────────────────────────

fn build_realtime_url(base_url: &str, model: &str) -> String {
    // Normalize: HTTP → WS, HTTPS → WSS
    let base = base_url
        .trim_end_matches('/')
        .replace("https://", "wss://")
        .replace("http://", "ws://");

    // If already pointing at realtime, just add query param
    if base.contains("/v1/realtime") {
        format!("{}?model={}", base, urlencoding::encode(model))
    } else {
        format!("{}/v1/realtime?model={}", base, urlencoding::encode(model))
    }
}

// ── Message conversion ────────────────────────────────────────

/// Convert an axum WebSocket message to a tungstenite message.
/// Returns `None` for Close frames (handled separately).
fn axum_to_tungstenite(msg: axum::extract::ws::Message) -> Option<Message> {
    use axum::extract::ws::Message as AM;
    match msg {
        AM::Text(t) => Some(Message::Text(t.to_string())),
        AM::Binary(b) => Some(Message::Binary(b.to_vec())),
        AM::Ping(d) => Some(Message::Ping(d.to_vec())),
        AM::Pong(d) => Some(Message::Pong(d.to_vec())),
        AM::Close(_) => None,
    }
}

/// Convert a tungstenite message to an axum WebSocket message.
fn tungstenite_to_axum(msg: Message) -> axum::extract::ws::Message {
    use axum::extract::ws::Message as AM;
    match msg {
        Message::Text(t) => AM::Text(t),
        Message::Binary(b) => AM::Binary(b),
        Message::Ping(d) => AM::Ping(d),
        Message::Pong(d) => AM::Pong(d),
        Message::Close(f) => AM::Close(f.map(|cf| axum::extract::ws::CloseFrame {
            code: cf.code.into(),
            reason: cf.reason.to_string().into(),
        })),
        Message::Frame(_) => AM::Binary(vec![]),
    }
}
