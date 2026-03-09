//! Prompt Management API handlers.
//!
//! CRUD for prompts and versions, plus the Render API that resolves
//! {{variable}} placeholders and returns ready-to-use OpenAI payloads.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AuthContext;
use crate::store::postgres::{NewPrompt, NewPromptVersion};
use crate::AppState;

// ── Request / Response types ─────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePromptRequest {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub folder: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromptRequest {
    pub name: String,
    pub description: Option<String>,
    pub folder: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVersionRequest {
    pub model: String,
    pub messages: serde_json::Value,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub tools: Option<serde_json::Value>,
    pub commit_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub version: i32,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct ListPromptsQuery {
    pub folder: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenderQuery {
    pub label: Option<String>,
    pub version: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct RenderBody {
    pub variables: Option<serde_json::Map<String, serde_json::Value>>,
    pub label: Option<String>,
    pub version: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct RenderResponse {
    pub model: String,
    pub messages: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    pub version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub prompt_id: Uuid,
    pub prompt_slug: String,
}

// ── Helpers ──────────────────────────────────────────────────

/// Generate a URL-safe slug from a name.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Replace `{{variable}}` placeholders in a JSON messages array.
pub fn render_variables(
    messages: &serde_json::Value,
    variables: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let json_str = messages.to_string();
    let mut rendered = json_str;
    for (key, value) in variables {
        let placeholder = format!("{{{{{}}}}}", key); // {{key}}
        let replacement = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        rendered = rendered.replace(&placeholder, &replacement);
    }
    serde_json::from_str(&rendered).unwrap_or_else(|_| messages.clone())
}

// ── Handlers ─────────────────────────────────────────────────

/// POST /prompts — create a new prompt
pub async fn create_prompt(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreatePromptRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    auth.require_scope("prompts:write")?;
    let project_id = auth.default_project_id();
    let slug = payload.slug.unwrap_or_else(|| slugify(&payload.name));

    let prompt = NewPrompt {
        project_id,
        name: payload.name,
        slug,
        description: payload.description.unwrap_or_default(),
        folder: payload.folder.unwrap_or_else(|| "/".to_string()),
        tags: payload.tags.unwrap_or(serde_json::json!({})),
        created_by: auth.user_id.map(|u| u.to_string()).unwrap_or_default(),
    };

    match state.db.insert_prompt(&prompt).await {
        Ok(row) => Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "id": row.id,
                "name": row.name,
                "slug": row.slug,
                "folder": row.folder,
                "message": "Prompt created"
            })),
        )),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate key") || msg.contains("unique constraint") {
                Err(StatusCode::CONFLICT)
            } else {
                tracing::error!("Failed to create prompt: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// GET /prompts — list all prompts
pub async fn list_prompts(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(q): Query<ListPromptsQuery>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();
    let prompts = state
        .db
        .list_prompts(project_id, q.folder.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to list prompts: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Enrich with latest version info
    let mut results = Vec::with_capacity(prompts.len());
    for p in prompts {
        let versions = state
            .db
            .list_prompt_versions(p.id)
            .await
            .unwrap_or_default();
        let latest = versions.first();
        results.push(serde_json::json!({
            "id": p.id,
            "name": p.name,
            "slug": p.slug,
            "description": p.description,
            "folder": p.folder,
            "tags": p.tags,
            "created_at": p.created_at,
            "updated_at": p.updated_at,
            "version_count": versions.len(),
            "latest_version": latest.map(|v| v.version),
            "latest_model": latest.map(|v| &v.model),
            "labels": latest.map(|v| &v.labels).unwrap_or(&serde_json::json!([])),
        }));
    }

    Ok(Json(results))
}

/// GET /prompts/:id — get prompt with latest version
pub async fn get_prompt(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();
    let prompt = state
        .db
        .get_prompt(id, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let versions = state.db.list_prompt_versions(id).await.unwrap_or_default();

    Ok(Json(serde_json::json!({
        "prompt": prompt,
        "versions": versions,
        "version_count": versions.len(),
    })))
}

/// PUT /prompts/:id — update prompt metadata
pub async fn update_prompt(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePromptRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("prompts:write")?;
    let project_id = auth.default_project_id();
    let updated = state
        .db
        .update_prompt(
            id,
            project_id,
            &payload.name,
            &payload.description.unwrap_or_default(),
            &payload.folder.unwrap_or_else(|| "/".to_string()),
            &payload.tags.unwrap_or(serde_json::json!({})),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if updated {
        Ok(Json(serde_json::json!({"message": "Prompt updated"})))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// DELETE /prompts/:id — soft-delete
pub async fn delete_prompt(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("prompts:write")?;
    let project_id = auth.default_project_id();
    let deleted = state
        .db
        .delete_prompt(id, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if deleted {
        Ok(Json(serde_json::json!({"message": "Prompt deleted"})))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// POST /prompts/:id/versions — publish a new version
pub async fn create_version(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateVersionRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    auth.require_scope("prompts:write")?;
    let project_id = auth.default_project_id();

    // Verify prompt exists
    state
        .db
        .get_prompt(id, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let version = NewPromptVersion {
        prompt_id: id,
        model: payload.model,
        messages: payload.messages,
        temperature: payload.temperature,
        max_tokens: payload.max_tokens,
        top_p: payload.top_p,
        tools: payload.tools,
        commit_message: payload.commit_message.unwrap_or_default(),
        created_by: auth.user_id.map(|u| u.to_string()).unwrap_or_default(),
    };

    let row = state
        .db
        .insert_prompt_version(&version)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": row.id,
            "version": row.version,
            "model": row.model,
            "message": format!("Version {} created", row.version)
        })),
    ))
}

/// GET /prompts/:id/versions — list all versions
pub async fn list_versions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();

    // Verify prompt exists
    state
        .db
        .get_prompt(id, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let versions = state
        .db
        .list_prompt_versions(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<serde_json::Value> = versions
        .into_iter()
        .map(|v| {
            serde_json::json!({
                "id": v.id,
                "version": v.version,
                "model": v.model,
                "messages": v.messages,
                "temperature": v.temperature,
                "max_tokens": v.max_tokens,
                "top_p": v.top_p,
                "tools": v.tools,
                "commit_message": v.commit_message,
                "created_at": v.created_at,
                "created_by": v.created_by,
                "labels": v.labels,
            })
        })
        .collect();

    Ok(Json(results))
}

/// GET /prompts/:id/versions/:version
pub async fn get_version(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((id, version)): Path<(Uuid, i32)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();

    state
        .db
        .get_prompt(id, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let v = state
        .db
        .get_prompt_version(id, version)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!(v)))
}

/// POST /prompts/:id/deploy — atomically promote a version to a label
pub async fn deploy_version(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<DeployRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth.require_scope("prompts:write")?;
    let project_id = auth.default_project_id();

    state
        .db
        .get_prompt(id, project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let deployed = state
        .db
        .deploy_prompt_version(id, payload.version, &payload.label)
        .await
        .map_err(|e| {
            tracing::error!("Failed to deploy version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if deployed {
        tracing::info!(
            prompt_id = %id,
            version = payload.version,
            label = %payload.label,
            "Deployed prompt version"
        );
        Ok(Json(serde_json::json!({
            "message": format!("Version {} deployed to '{}'", payload.version, payload.label),
            "version": payload.version,
            "label": payload.label,
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// GET /prompts/by-slug/:slug/render — render a prompt with query params
pub async fn render_prompt_get(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(slug): Path<String>,
    Query(q): Query<RenderQuery>,
) -> Result<Json<RenderResponse>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();
    let empty_vars = serde_json::Map::new();
    render_prompt_inner(
        &state,
        project_id,
        &slug,
        q.label.as_deref(),
        q.version,
        &empty_vars,
    )
    .await
}

/// POST /prompts/by-slug/:slug/render — render with variables in body
pub async fn render_prompt_post(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(slug): Path<String>,
    Json(body): Json<RenderBody>,
) -> Result<Json<RenderResponse>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();
    let variables = body.variables.unwrap_or_default();
    render_prompt_inner(
        &state,
        project_id,
        &slug,
        body.label.as_deref(),
        body.version,
        &variables,
    )
    .await
}

async fn render_prompt_inner(
    state: &Arc<AppState>,
    project_id: Uuid,
    slug: &str,
    label: Option<&str>,
    version: Option<i32>,
    variables: &serde_json::Map<String, serde_json::Value>,
) -> Result<Json<RenderResponse>, StatusCode> {
    let (prompt, pv) = state
        .db
        .get_prompt_for_render(project_id, slug, label, version)
        .await
        .map_err(|e| {
            tracing::error!("Render query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Resolve label for the response
    let resolved_label = label.map(String::from).or_else(|| {
        pv.labels
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(String::from)
    });

    // Replace {{variable}} placeholders
    let rendered_messages = if variables.is_empty() {
        pv.messages.clone()
    } else {
        render_variables(&pv.messages, variables)
    };

    Ok(Json(RenderResponse {
        model: pv.model,
        messages: rendered_messages,
        temperature: pv.temperature,
        max_tokens: pv.max_tokens,
        top_p: pv.top_p,
        tools: pv.tools,
        version: pv.version,
        label: resolved_label,
        prompt_id: prompt.id,
        prompt_slug: prompt.slug,
    }))
}

/// GET /prompts/folders — list unique folders
pub async fn list_folders(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<String>>, StatusCode> {
    auth.require_scope("prompts:read")?;
    let project_id = auth.default_project_id();
    let folders = state
        .db
        .list_prompt_folders(project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(folders))
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Customer Support Agent"), "customer-support-agent");
        assert_eq!(slugify("my prompt! v2"), "my-prompt-v2");
        assert_eq!(slugify("  hello   world  "), "hello-world");
        assert_eq!(slugify("abc-def"), "abc-def");
    }

    #[test]
    fn test_render_variables_basic() {
        let messages = serde_json::json!([
            {"role": "system", "content": "You are helping {{user_name}} with {{topic}}."},
            {"role": "user", "content": "Help me with {{topic}} please."}
        ]);

        let mut vars = serde_json::Map::new();
        vars.insert("user_name".to_string(), serde_json::json!("Alice"));
        vars.insert("topic".to_string(), serde_json::json!("billing"));

        let rendered = render_variables(&messages, &vars);
        let msgs = rendered.as_array().unwrap();
        assert_eq!(
            msgs[0]["content"].as_str().unwrap(),
            "You are helping Alice with billing."
        );
        assert_eq!(
            msgs[1]["content"].as_str().unwrap(),
            "Help me with billing please."
        );
    }

    #[test]
    fn test_render_variables_empty() {
        let messages = serde_json::json!([
            {"role": "user", "content": "Hello world"}
        ]);
        let vars = serde_json::Map::new();
        let rendered = render_variables(&messages, &vars);
        assert_eq!(rendered, messages);
    }

    #[test]
    fn test_render_variables_unused_placeholder() {
        let messages = serde_json::json!([
            {"role": "system", "content": "Hello {{name}}, your ticket is {{ticket_id}}."}
        ]);
        let mut vars = serde_json::Map::new();
        vars.insert("name".to_string(), serde_json::json!("Bob"));
        // ticket_id not provided — placeholder stays
        let rendered = render_variables(&messages, &vars);
        assert_eq!(
            rendered[0]["content"].as_str().unwrap(),
            "Hello Bob, your ticket is {{ticket_id}}."
        );
    }

    #[test]
    fn test_render_variables_numeric() {
        let messages = serde_json::json!([
            {"role": "user", "content": "Show me the top {{count}} results."}
        ]);
        let mut vars = serde_json::Map::new();
        vars.insert("count".to_string(), serde_json::json!(5));
        let rendered = render_variables(&messages, &vars);
        assert_eq!(
            rendered[0]["content"].as_str().unwrap(),
            "Show me the top 5 results."
        );
    }
}
