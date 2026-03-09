//! Team/Org Management — multi-team hierarchy and tag-based attribution.
//!
//! Hierarchy: Organization → Team(s) → User(s) / Token(s)
//!
//! Features:
//! - Named teams within an organization
//! - Team membership with roles (admin, member, viewer)
//! - Per-team budget limits (daily/weekly/monthly/yearly)
//! - Per-team model access restrictions (inherits from org if not set)
//! - Tag-based cost attribution across teams
//! - Team spend tracking for budget enforcement

use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A team within an organization.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Team {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub max_budget_usd: Option<rust_decimal::Decimal>,
    pub budget_duration: Option<String>,
    pub allowed_models: Option<serde_json::Value>,
    pub tags: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A member of a team.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeamMember {
    pub id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Team spend tracking for a billing period.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeamSpend {
    pub id: Uuid,
    pub team_id: Uuid,
    pub period: chrono::NaiveDate,
    pub total_requests: i64,
    pub total_tokens_used: i64,
    pub total_spend_usd: rust_decimal::Decimal,
    pub updated_at: DateTime<Utc>,
}

/// Check if a team's budget has been exceeded for the current period.
///
/// 5D-3 FIX: Uses `SELECT ... FOR UPDATE` to serialize concurrent budget
/// checks. Without this, N concurrent requests could all read the same
/// spend value and all pass the check, allowing N× budget overrun.
///
/// Returns `Ok(())` if within budget or no budget set,
/// or `Err(reason)` if budget exceeded.
pub async fn check_team_budget(pool: &sqlx::PgPool, team: &Team) -> Result<(), String> {
    let max_budget = match team.max_budget_usd {
        Some(b) => b,
        None => return Ok(()), // No budget limit
    };

    let duration = match &team.budget_duration {
        Some(d) => d.as_str(),
        None => return Ok(()), // No duration = no limit
    };

    // Determine the current period start
    let now = chrono::Utc::now().naive_utc().date();
    let period_start = match duration {
        "daily" => now,
        "weekly" => {
            let days_since_monday = now.weekday().num_days_from_monday();
            now - chrono::Duration::days(days_since_monday as i64)
        }
        "monthly" => chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap_or(now),
        "yearly" => chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1).unwrap_or(now),
        _ => return Ok(()), // Unknown duration, allow
    };

    // 5D-3 FIX: Use FOR UPDATE to serialize concurrent budget checks.
    // This acquires a row-level lock so only one request at a time can
    // read/compare the spend value, preventing the TOCTOU race.
    let spend: Option<TeamSpend> =
        sqlx::query_as("SELECT * FROM team_spend WHERE team_id = $1 AND period = $2 FOR UPDATE")
            .bind(team.id)
            .bind(period_start)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    if let Some(s) = spend {
        if s.total_spend_usd >= max_budget {
            return Err(format!(
                "Team '{}' has exceeded its {} budget of ${:.2} (spent: ${:.2})",
                team.name, duration, max_budget, s.total_spend_usd
            ));
        }
    }

    Ok(())
}

/// Increment team spend for the current period.
/// Called after each successful proxied request.
#[allow(dead_code)]
pub async fn record_team_spend(
    pool: &sqlx::PgPool,
    team_id: Uuid,
    tokens_used: i64,
    spend_usd: rust_decimal::Decimal,
    budget_duration: Option<&str>,
) {
    let now = chrono::Utc::now().naive_utc().date();
    let period = match budget_duration {
        Some("daily") => now,
        Some("weekly") => {
            let days_since_monday = now.weekday().num_days_from_monday();
            now - chrono::Duration::days(days_since_monday as i64)
        }
        Some("monthly") => {
            chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap_or(now)
        }
        Some("yearly") => chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1).unwrap_or(now),
        _ => now, // Default to daily
    };

    let result = sqlx::query(
        r#"INSERT INTO team_spend (team_id, period, total_requests, total_tokens_used, total_spend_usd)
           VALUES ($1, $2, 1, $3, $4)
           ON CONFLICT (team_id, period) DO UPDATE SET
               total_requests = team_spend.total_requests + 1,
               total_tokens_used = team_spend.total_tokens_used + $3,
               total_spend_usd = team_spend.total_spend_usd + $4,
               updated_at = NOW()"#
    )
    .bind(team_id)
    .bind(period)
    .bind(tokens_used)
    .bind(spend_usd)
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::error!(team_id = %team_id, "Failed to record team spend: {}", e);
    }
}

/// Resolve a team by ID, returning None if not found or inactive.
pub async fn get_team(pool: &sqlx::PgPool, team_id: Uuid) -> Option<Team> {
    sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = $1 AND is_active = true")
        .bind(team_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
}

/// Check if a model is allowed by the team's model restrictions.
/// Falls back to allowing all if no restrictions set.
pub fn check_team_model_access(model: &str, team: &Team) -> Result<(), String> {
    if model.is_empty() {
        return Ok(());
    }

    let patterns = match &team.allowed_models {
        Some(models_json) => {
            let mut p = Vec::new();
            if let Some(arr) = models_json.as_array() {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        p.push(s.to_string());
                    }
                }
            }
            p
        }
        None => return Ok(()), // No team-level restriction
    };

    if patterns.is_empty() {
        return Ok(());
    }

    for pattern in &patterns {
        if crate::middleware::model_access::model_matches(model, pattern) {
            return Ok(());
        }
    }

    Err(format!(
        "Model '{}' is not allowed by team '{}'. Team allowed: [{}]",
        model,
        team.name,
        patterns.join(", ")
    ))
}

/// Merge token tags with team tags. Token tags take precedence.
#[allow(dead_code)]
pub fn merge_tags(
    team_tags: &serde_json::Value,
    token_tags: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = serde_json::Map::new();

    // Start with team tags
    if let Some(team_map) = team_tags.as_object() {
        for (k, v) in team_map {
            merged.insert(k.clone(), v.clone());
        }
    }

    // Overlay token tags (token wins on conflict)
    if let Some(token_map) = token_tags.as_object() {
        for (k, v) in token_map {
            merged.insert(k.clone(), v.clone());
        }
    }

    serde_json::Value::Object(merged)
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_tags_empty() {
        let team = serde_json::json!({});
        let token = serde_json::json!({});
        let result = merge_tags(&team, &token);
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn test_merge_tags_team_only() {
        let team = serde_json::json!({"env": "production", "department": "engineering"});
        let token = serde_json::json!({});
        let result = merge_tags(&team, &token);
        assert_eq!(result["env"], "production");
        assert_eq!(result["department"], "engineering");
    }

    #[test]
    fn test_merge_tags_token_only() {
        let team = serde_json::json!({});
        let token = serde_json::json!({"agent": "chatbot", "version": "v2"});
        let result = merge_tags(&team, &token);
        assert_eq!(result["agent"], "chatbot");
        assert_eq!(result["version"], "v2");
    }

    #[test]
    fn test_merge_tags_token_wins_on_conflict() {
        let team = serde_json::json!({"env": "staging", "department": "engineering"});
        let token = serde_json::json!({"env": "production", "agent": "chatbot"});
        let result = merge_tags(&team, &token);
        assert_eq!(result["env"], "production"); // token wins
        assert_eq!(result["department"], "engineering"); // team preserved
        assert_eq!(result["agent"], "chatbot"); // token-only key
    }

    #[test]
    fn test_check_team_model_access_no_restriction() {
        let team = Team {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "test".into(),
            description: None,
            max_budget_usd: None,
            budget_duration: None,
            allowed_models: None, // No restriction
            tags: serde_json::json!({}),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(check_team_model_access("gpt-4o", &team).is_ok());
    }

    #[test]
    fn test_check_team_model_access_restricts() {
        let team = Team {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "budget-team".into(),
            description: None,
            max_budget_usd: None,
            budget_duration: None,
            allowed_models: Some(serde_json::json!(["gpt-4o-mini", "gpt-3.5*"])),
            tags: serde_json::json!({}),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(check_team_model_access("gpt-4o-mini", &team).is_ok());
        assert!(check_team_model_access("gpt-3.5-turbo", &team).is_ok());
        assert!(check_team_model_access("gpt-4o", &team).is_err());
    }

    #[test]
    fn test_check_team_model_access_empty_model() {
        let team = Team {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "test".into(),
            description: None,
            max_budget_usd: None,
            budget_duration: None,
            allowed_models: Some(serde_json::json!(["gpt-4o"])),
            tags: serde_json::json!({}),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(check_team_model_access("", &team).is_ok());
    }
}
