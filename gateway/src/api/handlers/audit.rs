use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Extension, Json,
};
use futures::stream::{self, Stream};
use uuid::Uuid;

use super::dtos::AuditFilterParams;
use super::helpers::verify_project_ownership;
use crate::api::AuthContext;
use crate::store::postgres::{AuditFilter, AuditLogDetailRow, AuditLogRow};
use crate::AppState;

pub async fn list_audit_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<AuditFilterParams>,
) -> Result<Json<Vec<AuditLogRow>>, StatusCode> {
    // Audit logs require explicit scope or read-all
    auth.require_scope("audit:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200); // 1 <= limit <= 200
    let offset = params.offset.unwrap_or(0).max(0); // non-negative

    // Build filter struct from params
    let filters = AuditFilter {
        status: params.status,
        token_id: params.token_id,
        model: params.model,
        policy_result: params.policy_result,
        method: params.method,
        path_contains: params.path_contains,
        agent_name: params.agent_name,
        error_type: params.error_type,
        start_time: params.start_time,
        end_time: params.end_time,
    };

    let logs = state
        .db
        .list_audit_logs_filtered(project_id, filters, limit, offset)
        .await
        .map_err(|e| {
            tracing::error!("list_audit_logs_filtered failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(logs))
}

/// GET /api/v1/audit/:id — single audit log detail with bodies
pub async fn get_audit_log(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Query(params): Query<AuditFilterParams>,
) -> Result<Json<AuditLogDetailRow>, StatusCode> {
    auth.require_scope("audit:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    verify_project_ownership(&state, auth.org_id, project_id).await?;
    let log_id = Uuid::parse_str(&id_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    let log = state
        .db
        .get_audit_log_detail(log_id, project_id)
        .await
        .map_err(|e| {
            tracing::error!("get_audit_log_detail failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(log))
}

pub async fn stream_audit_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<AuditFilterParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // MED-1: Check auth before streaming
    let has_scope = auth.require_scope("audit:read").is_ok();
    let project_id = params
        .project_id
        .unwrap_or_else(|| auth.default_project_id());
    let project_ok = verify_project_ownership(&state, auth.org_id, project_id)
        .await
        .is_ok();
    let authorized = has_scope && project_ok;

    // Build filter struct (without time range for streaming - we use last_seen instead)
    let filters = AuditFilter {
        status: params.status,
        token_id: params.token_id,
        model: params.model,
        policy_result: params.policy_result,
        method: params.method,
        path_contains: params.path_contains,
        agent_name: params.agent_name,
        error_type: params.error_type,
        start_time: None, // Not used for streaming
        end_time: None,
    };

    // MED-1: Use an enum to track stream state - send error event then end if unauthorized
    enum StreamState {
        Unauthorized,      // Need to send error event
        UnauthorizedDone,  // Error sent, end stream
        Active,            // Normal operation
    }

    let stream = stream::unfold(
        (
            state,
            project_id,
            filters,
            None::<chrono::DateTime<chrono::Utc>>,
            if authorized { StreamState::Active } else { StreamState::Unauthorized },
        ),
        |(state, project_id, filters, last_seen, stream_state)| async move {
            // MED-1: Handle unauthorized case - send single error event then end
            match stream_state {
                StreamState::Unauthorized => {
                    let error_event = Event::default()
                        .event("error")
                        .data(r#"{"error": "unauthorized", "message": "Authentication failed or insufficient permissions"}"#);
                    // Return error event and transition to done state
                    return Some((
                        Ok(error_event),
                        (state, project_id, filters, last_seen, StreamState::UnauthorizedDone),
                    ));
                }
                StreamState::UnauthorizedDone => {
                    // End the stream by returning None
                    return None;
                }
                StreamState::Active => {}
            }

            // Poll every 2 seconds
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let rows = state
                .db
                .list_audit_logs_filtered(project_id, filters.clone(), 20, 0)
                .await
                .unwrap_or_default();

            // Filter to only new entries since last_seen
            let new_rows: Vec<&AuditLogRow> = if let Some(last) = last_seen {
                rows.iter().filter(|r| r.created_at > last).collect()
            } else {
                // First poll: send nothing, just record the cursor
                vec![]
            };

            let next_cursor = rows.first().map(|r| r.created_at).or(last_seen);

            if new_rows.is_empty() {
                // Send a heartbeat comment to keep connection alive
                Some((
                    Ok(Event::default().comment("heartbeat")),
                    (state, project_id, filters, next_cursor, StreamState::Active),
                ))
            } else {
                let data = serde_json::to_string(&new_rows).unwrap_or_default();
                Some((
                    Ok(Event::default().data(data).event("audit")),
                    (state, project_id, filters, next_cursor, StreamState::Active),
                ))
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}
