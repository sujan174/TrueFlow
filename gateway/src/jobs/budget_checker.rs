//! Budget alert checking job.
//!
//! This module provides a background task that periodically:
//! 1. Aggregates monthly spend per project from `audit_logs`.
//! 2. Updates `project_spend` with the running total.
//! 3. Compares against `budget_alerts` thresholds.
//! 4. Fires warning or hard-cap webhook notifications.
//!
//! The job runs every 15 minutes.

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, info, warn};

use crate::cache::TieredCache;
use crate::notification::webhook::{WebhookEvent, WebhookNotifier};

/// Run the budget check once. Called by the periodic job scheduler.
pub async fn run_budget_check(pool: &PgPool) -> Result<()> {
    debug!("budget_check: starting periodic budget check");

    // 1. Aggregate monthly spend per project and upsert into project_spend
    sqlx::query(
        r#"
        INSERT INTO project_spend (project_id, period_start, period_key, spend_usd, updated_at)
        SELECT
            project_id,
            DATE_TRUNC('month', NOW()) AS period_start,
            'monthly'                  AS period_key,
            COALESCE(SUM(estimated_cost_usd), 0) AS spend_usd,
            NOW()
        FROM audit_logs
        WHERE
            estimated_cost_usd IS NOT NULL
            AND created_at >= DATE_TRUNC('month', NOW())
        GROUP BY project_id
        ON CONFLICT (project_id) DO UPDATE
            SET spend_usd    = EXCLUDED.spend_usd,
                period_start = EXCLUDED.period_start,
                updated_at   = NOW()
        "#,
    )
    .execute(pool)
    .await?;

    // 2. Fetch all active budgets joined with current spend
    let rows = sqlx::query(
        r#"
        SELECT
            ba.id::text            AS alert_id,
            ba.project_id::text    AS project_id,
            ba.warn_threshold_usd  AS warn_threshold,
            ba.hard_cap_usd        AS hard_cap,
            ba.notify_webhooks     AS webhooks,
            ba.warn_fired_at IS NOT NULL AS warn_fired,
            ba.cap_fired_at  IS NOT NULL AS cap_fired,
            COALESCE(ps.spend_usd, 0)    AS spend
        FROM budget_alerts ba
        LEFT JOIN project_spend ps ON ps.project_id = ba.project_id
        WHERE ba.is_active = TRUE
        "#,
    )
    .fetch_all(pool)
    .await?;

    debug!(
        count = rows.len(),
        "budget_check: checking {} active budget(s)",
        rows.len()
    );

    if rows.is_empty() {
        return Ok(());
    }

    let notifier = WebhookNotifier::new();

    for row in &rows {
        use sqlx::Row;

        let alert_id: &str = row.try_get("alert_id")?;
        let project_id: &str = row.try_get("project_id")?;
        let warn_threshold: Decimal = row.try_get("warn_threshold")?;
        let hard_cap: Option<Decimal> = row.try_get("hard_cap")?;
        let webhooks: serde_json::Value = row.try_get("webhooks")?;
        let warn_fired: bool = row.try_get("warn_fired")?;
        let cap_fired: bool = row.try_get("cap_fired")?;
        let spend: Decimal = row.try_get("spend")?;

        // Parse webhook URLs from JSON array
        let webhook_urls: Vec<String> = webhooks
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // --- Warn threshold check ---
        if spend >= warn_threshold && !warn_fired {
            warn!(
                project_id,
                spend = %spend,
                warn_threshold = %warn_threshold,
                "budget_check: WARNING threshold exceeded"
            );

            if !webhook_urls.is_empty() {
                let event = WebhookEvent {
                    event_type: "budget_warning".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    token_id: String::new(),
                    token_name: String::new(),
                    project_id: project_id.to_string(),
                    details: serde_json::json!({
                        "spend_usd": spend.to_string(),
                        "warn_threshold_usd": warn_threshold.to_string(),
                        "period": "monthly",
                    }),
                };
                notifier.dispatch(&webhook_urls, event).await;
            }

            sqlx::query("UPDATE budget_alerts SET warn_fired_at = NOW() WHERE id = $1::uuid")
                .bind(alert_id)
                .execute(pool)
                .await?;

            info!(project_id, spend = %spend, "budget_check: warn alert fired");
        }

        // --- Hard cap check ---
        if let Some(hard_cap) = hard_cap {
            if spend >= hard_cap && !cap_fired {
                warn!(
                    project_id,
                    spend = %spend,
                    hard_cap = %hard_cap,
                    "budget_check: HARD CAP exceeded"
                );

                if !webhook_urls.is_empty() {
                    let event = WebhookEvent {
                        event_type: "budget_cap_exceeded".to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        token_id: String::new(),
                        token_name: String::new(),
                        project_id: project_id.to_string(),
                        details: serde_json::json!({
                            "spend_usd": spend.to_string(),
                            "hard_cap_usd": hard_cap.to_string(),
                            "period": "monthly",
                        }),
                    };
                    notifier.dispatch(&webhook_urls, event).await;
                }

                sqlx::query("UPDATE budget_alerts SET cap_fired_at = NOW() WHERE id = $1::uuid")
                    .bind(alert_id)
                    .execute(pool)
                    .await?;

                info!(project_id, spend = %spend, "budget_check: hard cap alert fired");
            }
        }
    }

    debug!("budget_check: complete");
    Ok(())
}

/// Check whether a project has exceeded its hard cap.
/// Returns `true` if capped, `false` if within budget or no budget configured.
/// Fails open on DB error to avoid blocking requests.
#[allow(dead_code)]
pub async fn is_project_over_hard_cap(pool: &PgPool, project_id: uuid::Uuid) -> bool {
    let result = sqlx::query(
        r#"
        SELECT ba.hard_cap_usd, COALESCE(ps.spend_usd, 0::numeric) AS spend
        FROM budget_alerts ba
        LEFT JOIN project_spend ps ON ps.project_id = ba.project_id
        WHERE ba.project_id = $1
          AND ba.is_active = TRUE
          AND ba.hard_cap_usd IS NOT NULL
        LIMIT 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(row)) => {
            use sqlx::Row;
            let cap: Option<Decimal> = row.try_get("hard_cap_usd").unwrap_or(None);
            let spend: Decimal = row.try_get("spend").unwrap_or_default();
            if let Some(c) = cap {
                let over = spend >= c;
                if over {
                    tracing::warn!(
                        project_id = %project_id,
                        spend = %spend,
                        hard_cap = %c,
                        "budget enforcement: project over hard cap"
                    );
                }
                return over;
            }
            false
        }
        Ok(None) => false,
        Err(e) => {
            tracing::error!(project_id = %project_id, error = %e, "budget cap check DB error - treating as over budget");
            true // fail closed - treat DB error as over budget to prevent overspending
        }
    }
}

/// Cached variant of `is_project_over_hard_cap` for use in the request hot path.
///
/// Caches the result in Redis for 60 seconds to avoid a DB round-trip on every
/// request. This means a project can over-spend by at most 60 seconds of requests
/// after hitting the hard cap — an acceptable trade-off vs a per-request DB query.
///
/// The cache key is purposely short-lived and will refresh automatically.
pub async fn is_project_over_hard_cap_cached(
    pool: &PgPool,
    cache: &TieredCache,
    project_id: uuid::Uuid,
) -> bool {
    use redis::AsyncCommands;

    let cache_key = format!("project_hardcap:{}", project_id);
    let mut conn = cache.redis();

    // Try to read from Redis first
    if let Ok(Some(cached)) = conn.get::<_, Option<String>>(&cache_key).await {
        return cached == "1";
    }

    // Cache miss: query DB and populate cache
    let is_capped = is_project_over_hard_cap(pool, project_id).await;

    // Cache for 60 seconds (intentionally short so hard caps take effect quickly)
    let value = if is_capped { "1" } else { "0" };
    let _: () = conn.set_ex(&cache_key, value, 60).await.unwrap_or(());

    is_capped
}
