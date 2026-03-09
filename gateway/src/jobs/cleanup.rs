//! Background job: auto-expire Level 2 (full debug) audit log bodies.
//!
//! Runs hourly. Downgrades Level 2 → Level 0 (metadata-only) and strips
//! bodies from `audit_log_bodies`, preserving cost/token/latency metadata
//! for billing accuracy.

use sqlx::PgPool;
use std::time::Duration;
use tokio::time;

/// Spawn the background cleanup task. Call this once at startup.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            let pool = pool.clone();
            let result = tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(3600)); // every hour
                loop {
                    interval.tick().await;
                    if let Err(e) = expire_debug_logs(&pool).await {
                        tracing::error!("cleanup job failed: {}", e);
                    }
                }
            })
            .await;
            if let Err(e) = result {
                tracing::error!("Cleanup job panicked: {:?}", e);
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
}

/// Downgrade Level 2 logs older than 24 hours.
/// IMPORTANT: UPDATE not DELETE — preserves billing metadata.
async fn expire_debug_logs(pool: &PgPool) -> anyhow::Result<()> {
    // Step 1: Downgrade log level in the main table
    let updated = sqlx::query(
        r#"
        UPDATE audit_logs
        SET log_level = 0
        WHERE log_level = 2 AND created_at < NOW() - INTERVAL '24 hours'
        "#,
    )
    .execute(pool)
    .await?;

    if updated.rows_affected() > 0 {
        tracing::info!(
            rows = updated.rows_affected(),
            "downgraded Level 2 audit logs to Level 0"
        );
    }

    // Step 2: Strip bodies from the bodies table for downgraded entries
    let stripped = sqlx::query(
        r#"
        UPDATE audit_log_bodies
        SET request_body = '[EXPIRED]',
            response_body = '[EXPIRED]',
            request_headers = NULL,
            response_headers = NULL
        WHERE audit_id IN (
            SELECT id FROM audit_logs
            WHERE log_level = 0 AND created_at < NOW() - INTERVAL '24 hours'
        )
        AND request_body IS DISTINCT FROM '[EXPIRED]'
        "#,
    )
    .execute(pool)
    .await?;

    if stripped.rows_affected() > 0 {
        tracing::info!(
            rows = stripped.rows_affected(),
            "stripped expired debug bodies"
        );
    }

    Ok(())
}
