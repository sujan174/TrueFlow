use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};

use crate::api::AuthContext;
use crate::AppState;

pub async fn list_teams(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<crate::middleware::teams::Team>>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let rows = sqlx::query_as::<_, crate::middleware::teams::Team>(
        "SELECT * FROM teams WHERE org_id = $1 ORDER BY name",
    )
    .bind(auth.org_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to list teams: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rows))
}

/// POST /api/v1/teams — create a new team
pub async fn create_team(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::teams::Team>, StatusCode> {
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let description = body.get("description").and_then(|v| v.as_str());
    let max_budget = body
        .get("max_budget_usd")
        .and_then(|v| v.as_f64())
        .map(|f| rust_decimal::Decimal::from_f64_retain(f).unwrap_or_default());
    let budget_duration = body.get("budget_duration").and_then(|v| v.as_str());
    let allowed_models = body.get("allowed_models");
    let default_tags = serde_json::json!({});
    let tags = body.get("tags").unwrap_or(&default_tags);

    let row = sqlx::query_as::<_, crate::middleware::teams::Team>(
        r#"INSERT INTO teams (org_id, name, description, max_budget_usd, budget_duration, allowed_models, tags)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#
    )
    .bind(auth.org_id)
    .bind(name)
    .bind(description)
    .bind(max_budget)
    .bind(budget_duration)
    .bind(allowed_models)
    .bind(tags)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to create team: {}", e);
        if e.to_string().contains("duplicate key") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    Ok(Json(row))
}

/// PUT /api/v1/teams/:id — update a team
pub async fn update_team(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::teams::Team>, StatusCode> {
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let name = body.get("name").and_then(|v| v.as_str());
    let description = body.get("description").and_then(|v| v.as_str());
    let max_budget = body
        .get("max_budget_usd")
        .and_then(|v| v.as_f64())
        .map(|f| rust_decimal::Decimal::from_f64_retain(f).unwrap_or_default());
    let budget_duration = body.get("budget_duration").and_then(|v| v.as_str());
    let allowed_models = body.get("allowed_models");
    let tags = body.get("tags");

    let row = sqlx::query_as::<_, crate::middleware::teams::Team>(
        r#"UPDATE teams SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            max_budget_usd = COALESCE($5, max_budget_usd),
            budget_duration = COALESCE($6, budget_duration),
            allowed_models = COALESCE($7, allowed_models),
            tags = COALESCE($8, tags),
            updated_at = NOW()
           WHERE id = $1 AND org_id = $2
           RETURNING *"#,
    )
    .bind(team_id)
    .bind(auth.org_id)
    .bind(name)
    .bind(description)
    .bind(max_budget)
    .bind(budget_duration)
    .bind(allowed_models)
    .bind(tags)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to update team: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        Some(r) => Ok(Json(r)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// DELETE /api/v1/teams/:id — delete a team
pub async fn delete_team(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let result = sqlx::query("DELETE FROM teams WHERE id = $1 AND org_id = $2")
        .bind(team_id)
        .bind(auth.org_id)
        .execute(state.db.pool())
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete team: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// GET /api/v1/teams/:id/members — list members of a team
pub async fn list_team_members(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let rows = sqlx::query_as::<_, crate::middleware::teams::TeamMember>(
        "SELECT tm.* FROM team_members tm JOIN teams t ON tm.team_id = t.id WHERE tm.team_id = $1 AND t.org_id = $2"
    )
    .bind(team_id)
    .bind(auth.org_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to list team members: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let members: Vec<serde_json::Value> = rows
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "team_id": m.team_id,
                "user_id": m.user_id,
                "role": m.role,
                "created_at": m.created_at,
            })
        })
        .collect();

    Ok(Json(members))
}

/// POST /api/v1/teams/:id/members — add a member to a team
pub async fn add_team_member(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::middleware::teams::TeamMember>, StatusCode> {
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let user_id: uuid::Uuid = body
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let role = body
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("member");

    // Verify team belongs to org
    let team_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM teams WHERE id = $1 AND org_id = $2)",
    )
    .bind(team_id)
    .bind(auth.org_id)
    .fetch_one(state.db.pool())
    .await
    .unwrap_or(false);

    if !team_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    let row = sqlx::query_as::<_, crate::middleware::teams::TeamMember>(
        r#"INSERT INTO team_members (team_id, user_id, role)
           VALUES ($1, $2, $3)
           RETURNING *"#,
    )
    .bind(team_id)
    .bind(user_id)
    .bind(role)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("duplicate key") {
            tracing::warn!("Duplicate team member: team={}, user={}", team_id, user_id);
            StatusCode::CONFLICT
        } else if msg.contains("foreign key") || msg.contains("violates foreign key") {
            tracing::warn!(
                "Team member FK violation (user_id not found): team={}, user={}: {}",
                team_id,
                user_id,
                msg
            );
            StatusCode::UNPROCESSABLE_ENTITY
        } else {
            tracing::error!("Failed to add team member: {}", msg);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    Ok(Json(row))
}

/// DELETE /api/v1/teams/:id/members/:user_id — remove a member from a team
pub async fn remove_team_member(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((team_id, user_id)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> Result<StatusCode, StatusCode> {
    auth.require_scope("tokens:write")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let result = sqlx::query(
        r#"DELETE FROM team_members
           WHERE team_id = $1 AND user_id = $2
           AND team_id IN (SELECT id FROM teams WHERE org_id = $3)"#,
    )
    .bind(team_id)
    .bind(user_id)
    .bind(auth.org_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to remove team member: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// GET /api/v1/teams/:id/spend — get team spend summary
pub async fn get_team_spend(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(team_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<crate::middleware::teams::TeamSpend>>, StatusCode> {
    auth.require_scope("tokens:read")
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let rows = sqlx::query_as::<_, crate::middleware::teams::TeamSpend>(
        r#"SELECT ts.* FROM team_spend ts
           JOIN teams t ON ts.team_id = t.id
           WHERE ts.team_id = $1 AND t.org_id = $2
           ORDER BY ts.period DESC LIMIT 30"#,
    )
    .bind(team_id)
    .bind(auth.org_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to get team spend: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}
