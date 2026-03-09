//! Background job: clean up orphaned sessions.
//!
//! Sessions that have been "active" or "paused" for more than 24 hours
//! without any updates are considered orphaned and are automatically
//! marked as "expired".

use sqlx::postgres::PgPool;
use tracing::info;

/// Mark orphaned sessions as expired.
/// A session is considered orphaned if:
/// - status is "active" or "paused"
/// - updated_at is more than 24 hours ago
pub async fn expire_orphaned_sessions(pool: &PgPool) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE sessions
        SET status = 'expired', updated_at = NOW()
        WHERE status IN ('active', 'paused')
        AND updated_at < NOW() - INTERVAL '24 hours'
        "#,
    )
    .execute(pool)
    .await?;

    let count = result.rows_affected();
    if count > 0 {
        info!(
            count,
            "session_cleanup: marked orphaned sessions as expired"
        );
    }

    Ok(count)
}

/// Spawn a background task that runs every 15 minutes.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(900)); // 15 minutes
        loop {
            interval.tick().await;
            if let Err(e) = expire_orphaned_sessions(&pool).await {
                tracing::error!(error = %e, "session_cleanup: failed to expire orphaned sessions");
            }
        }
    });
    info!("Session cleanup job started (every 15 minutes)");
}
