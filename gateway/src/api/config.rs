//! Config-as-Code: YAML/JSON export and import of policies and tokens.
//!
//! Endpoints:
//!   GET  /api/v1/config/export         — export all policies + tokens as YAML
//!   POST /api/v1/config/import         — import (upsert) config from YAML/JSON body
//!   GET  /api/v1/config/export/policies — export policies only
//!   GET  /api/v1/config/export/tokens   — export tokens only (no secrets)
//!
//! The YAML schema is stable across gateway versions. It is explicitly versioned
//! so that future breaking changes can be detected and rejected gracefully.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;

// Default project ID used when no project_id is specified in the query.
// For MVP this is the hardcoded default; route-level auth is via admin_auth
// middleware which already checks the admin API key, so no AuthContext needed.
const DEFAULT_PROJECT: &str = "00000000-0000-0000-0000-000000000001";

// ── Config Document Schema ────────────────────────────────────

/// The top-level exported config document.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigDocument {
    /// Schema version — currently "1". Must be "1" to import.
    pub version: String,
    /// All policies in the project.
    #[serde(default)]
    pub policies: Vec<PolicyExport>,
    /// All tokens in the project (no credentials — those are managed separately).
    #[serde(default)]
    pub tokens: Vec<TokenExport>,
}

/// Serialized representation of a policy for export/import.
#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyExport {
    /// Policy name — used as the unique key on import (upsert).
    pub name: String,
    /// "enforce" or "shadow"
    pub mode: String,
    /// "request" or "response"
    pub phase: String,
    /// Raw rules array (same schema as the create/update API).
    pub rules: serde_json::Value,
    /// Optional retry configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<serde_json::Value>,
}

/// Serialized representation of a token for export/import.
/// Credentials are intentionally excluded — they must be managed separately.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenExport {
    /// Token display name — used as the unique key on import.
    pub name: String,
    /// Primary upstream URL.
    pub upstream_url: String,
    /// Policy names attached to this token (resolved on import).
    #[serde(default)]
    pub policies: Vec<String>,
    /// Optional log level: "metadata" | "redacted" | "full"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
}

// ── Query Params ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExportQuery {
    /// Output format: "yaml" (default) or "json"
    #[serde(default = "default_format")]
    pub format: String,
    /// Optional project ID filter. Defaults to the default project.
    pub project_id: Option<Uuid>,
}

fn default_format() -> String {
    "yaml".to_string()
}

// ── Handlers ──────────────────────────────────────────────────

/// GET /api/v1/config/export
/// Export the complete config (policies + tokens) as YAML or JSON.
pub async fn export_config(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportQuery>,
) -> Result<Response, StatusCode> {
    let project_id = params.project_id
        .unwrap_or_else(|| Uuid::parse_str(DEFAULT_PROJECT).unwrap());
    let doc = build_config_document(&state, project_id).await?;
    serialize_and_respond(doc, &params.format)
}

/// GET /api/v1/config/export/policies
/// Export policies only.
pub async fn export_policies(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportQuery>,
) -> Result<Response, StatusCode> {
    let project_id = params.project_id
        .unwrap_or_else(|| Uuid::parse_str(DEFAULT_PROJECT).unwrap());
    let policies = fetch_policies(&state, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let doc = ConfigDocument {
        version: "1".to_string(),
        policies,
        tokens: vec![],
    };
    serialize_and_respond(doc, &params.format)
}

/// GET /api/v1/config/export/tokens
/// Export tokens only (policy names resolved, no credentials).
pub async fn export_tokens(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportQuery>,
) -> Result<Response, StatusCode> {
    let project_id = params.project_id
        .unwrap_or_else(|| Uuid::parse_str(DEFAULT_PROJECT).unwrap());
    let (tokens, _policies) =
        tokio::try_join!(fetch_tokens(&state, project_id), fetch_policies(&state, project_id))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let doc = ConfigDocument {
        version: "1".to_string(),
        policies: vec![],
        tokens,
    };
    serialize_and_respond(doc, &params.format)
}

/// POST /api/v1/config/import
/// Import (upsert) policies and tokens from a YAML or JSON body.
///
/// Content-Type detection:
///   - `application/yaml` or `text/yaml` → parse as YAML
///   - `application/json`                → parse as JSON
///   - anything else                      → try YAML first, then JSON
pub async fn import_config(
    State(state): State<Arc<AppState>>,
    req: axum::http::Request<Body>,
) -> Result<Json<ImportResult>, StatusCode> {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let bytes = axum::body::to_bytes(req.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let doc: ConfigDocument = if content_type.contains("json") {
        serde_json::from_slice(&bytes).map_err(|e| {
            tracing::warn!("config import: JSON parse error: {}", e);
            StatusCode::UNPROCESSABLE_ENTITY
        })?
    } else {
        // Parse as YAML (also handles JSON since JSON is valid YAML)
        serde_yaml::from_slice(&bytes).map_err(|e| {
            tracing::warn!("config import: YAML parse error: {}", e);
            StatusCode::UNPROCESSABLE_ENTITY
        })?
    };

    if doc.version != "1" {
        tracing::warn!("config import: unsupported schema version: {}", doc.version);
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let project_id = Uuid::parse_str(DEFAULT_PROJECT).unwrap();
    import_document(&state, project_id, doc).await
}

// ── Implementation Helpers ────────────────────────────────────

async fn build_config_document(
    state: &AppState,
    project_id: Uuid,
) -> Result<ConfigDocument, StatusCode> {
    let (policies, tokens) = tokio::try_join!(
        fetch_policies(state, project_id),
        fetch_tokens(state, project_id)
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(ConfigDocument {
        version: "1".to_string(),
        policies,
        tokens,
    })
}

async fn fetch_policies(
    state: &AppState,
    project_id: Uuid,
) -> anyhow::Result<Vec<PolicyExport>> {
    let rows = state.db.list_policies(project_id).await?;
    let exports = rows
        .into_iter()
        .filter(|r| r.is_active)
        .map(|r| PolicyExport {
            name: r.name,
            mode: r.mode,
            phase: r.phase,
            rules: r.rules,
            retry: r.retry,
        })
        .collect();
    Ok(exports)
}

async fn fetch_tokens(
    state: &AppState,
    project_id: Uuid,
) -> anyhow::Result<Vec<TokenExport>> {
    // Fetch tokens
    let token_rows = state.db.list_tokens(project_id).await?;

    // Fetch all policies in the project to build an id→name map
    let policy_rows = state.db.list_policies(project_id).await?;
    let policy_name_map: std::collections::HashMap<Uuid, String> = policy_rows
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();

    let exports = token_rows
        .into_iter()
        .map(|t| {
            let policy_names: Vec<String> = t
                .policy_ids
                .iter()
                .filter_map(|id| policy_name_map.get(id).cloned())
                .collect();

            let log_level_name = match t.log_level {
                0 => Some("metadata".to_string()),
                1 => Some("redacted".to_string()),
                2 => Some("full".to_string()),
                _ => None,
            };

            TokenExport {
                name: t.name,
                upstream_url: t.upstream_url,
                policies: policy_names,
                log_level: log_level_name,
            }
        })
        .collect();
    Ok(exports)
}

async fn import_document(
    state: &AppState,
    project_id: Uuid,
    doc: ConfigDocument,
) -> Result<Json<ImportResult>, StatusCode> {
    let mut result = ImportResult::default();

    // ── 1. Upsert policies ─────────────────────────────────────
    // Build a map of name→id for resolving token→policy references later.
    let existing_policies = state
        .db
        .list_policies(project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut policy_id_map: std::collections::HashMap<String, Uuid> = existing_policies
        .iter()
        .map(|p| (p.name.clone(), p.id))
        .collect();

    for policy in &doc.policies {
        let rules_val = policy.rules.clone();

        if let Some(&existing_id) = policy_id_map.get(&policy.name) {
            // Update existing policy
            let updated = state
                .db
                .update_policy(
                    existing_id,
                    project_id,
                    Some(&policy.mode),
                    Some(&policy.phase),
                    Some(rules_val),
                    policy.retry.clone(),
                    Some(&policy.name),
                )
                .await
                .map_err(|e| {
                    tracing::error!("config import: update policy '{}': {}", policy.name, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            if updated {
                result.policies_updated += 1;
            }
        } else {
            // Insert new policy
            let new_id = state
                .db
                .insert_policy(
                    project_id,
                    &policy.name,
                    &policy.mode,
                    &policy.phase,
                    rules_val,
                    policy.retry.clone(),
                )
                .await
                .map_err(|e| {
                    tracing::error!("config import: insert policy '{}': {}", policy.name, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            policy_id_map.insert(policy.name.clone(), new_id);
            result.policies_created += 1;
        }
    }

    // ── 2. Upsert tokens ───────────────────────────────────────
    let existing_tokens = state
        .db
        .list_tokens(project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let existing_token_map: std::collections::HashMap<String, _> = existing_tokens
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();

    for token_export in &doc.tokens {
        // Resolve policy names → IDs
        let policy_ids: Vec<Uuid> = token_export
            .policies
            .iter()
            .filter_map(|name| policy_id_map.get(name).copied())
            .collect();

        let log_level: i16 = match token_export.log_level.as_deref() {
            Some("metadata") => 0,
            Some("full") => 2,
            _ => 1, // default: redacted
        };

        if let Some(existing) = existing_token_map.get(&token_export.name) {
            // Update token's policy bindings and log level
            let updated = state
                .db
                .update_token_config(
                    &existing.id,
                    policy_ids,
                    log_level,
                    &token_export.upstream_url,
                )
                .await
                .map_err(|e| {
                    tracing::error!("config import: update token '{}': {}", token_export.name, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            if updated {
                result.tokens_updated += 1;
            }
        } else {
            // Create a new stub token (no credential — caller must set this separately)
            let _new_id = state
                .db
                .insert_token_stub(
                    project_id,
                    &token_export.name,
                    &token_export.upstream_url,
                    policy_ids,
                    log_level,
                )
                .await
                .map_err(|e| {
                    tracing::error!("config import: insert token '{}': {}", token_export.name, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            result.tokens_created += 1;
        }
    }

    Ok(Json(result))
}

fn serialize_and_respond(doc: ConfigDocument, format: &str) -> Result<Response, StatusCode> {
    if format == "json" {
        let body = serde_json::to_vec_pretty(&doc).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"trueflow_config.json\"",
            )
            .body(Body::from(body))
            .unwrap())
    } else {
        let body = serde_yaml::to_string(&doc).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/yaml; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"trueflow_config.yaml\"",
            )
            .body(Body::from(body))
            .unwrap())
    }
}

// ── Result DTO ────────────────────────────────────────────────

#[derive(Debug, Serialize, Default)]
pub struct ImportResult {
    pub policies_created: usize,
    pub policies_updated: usize,
    pub tokens_created: usize,
    pub tokens_updated: usize,
}
