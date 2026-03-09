//! Background job: auto-expire pending HITL approval requests.
//!
//! Runs every 60 seconds. Marks pending approvals as 'expired'
//! when their expires_at timestamp is in the past. This prevents
//! tokens from being permanently locked when approval requests are
//! never acted upon.

use sqlx::PgPool;
use std::time::Duration;
use tokio::time;

/// Spawn the background approval expiry task. Call this once at startup.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            let pool = pool.clone();
            let result = tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(60)); // every 60s
                loop {
                    interval.tick().await;
                    if let Err(e) = expire_approvals(&pool).await {
                        tracing::error!("approval expiry job failed: {}", e);
                    }
                }
            })
            .await;
            if let Err(e) = result {
                tracing::error!("Approval expiry job panicked: {:?}", e);
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
}

/// Mark expired pending approvals as 'expired'.
async fn expire_approvals(pool: &PgPool) -> anyhow::Result<()> {
    let updated = sqlx::query(
        r#"
        UPDATE approval_requests
        SET status = 'expired'
        WHERE status = 'pending' AND expires_at < NOW()
        "#,
    )
    .execute(pool)
    .await?;

    if updated.rows_affected() > 0 {
        tracing::info!(
            count = updated.rows_affected(),
            "marked expired pending approvals as expired"
        );
    }

    Ok(())
}
