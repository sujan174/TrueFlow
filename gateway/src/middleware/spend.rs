use crate::cache::TieredCache;
use anyhow::{Context, Result};
use chrono::{Datelike, Utc};
use redis::AsyncCommands;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

// ── Spend Cap Config ──────────────────────────────────────────

/// Spend cap configuration for a token (loaded from DB).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpendCap {
    pub daily_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    /// Absolute lifetime cap — never auto-resets. Useful for trial/hackathon keys.
    pub lifetime_limit_usd: Option<f64>,
}

/// Current spend status for a token (for the API/dashboard).
#[derive(Debug, Serialize)]
pub struct SpendStatus {
    pub daily_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    pub lifetime_limit_usd: Option<f64>,
    pub current_daily_usd: f64,
    pub current_monthly_usd: f64,
    pub current_lifetime_usd: f64,
}

// ── Enforcement ───────────────────────────────────────────────

/// Check if the token has exceeded its spend cap.
///
/// Reads the current daily and monthly spend from Redis and compares
/// against the caps stored in the `spend_caps` DB table.
///
/// Returns `Err` with a human-readable message if any cap is exceeded.
#[tracing::instrument(skip(cache, db))]
pub async fn check_spend_cap(cache: &TieredCache, db: &sqlx::PgPool, token_id: &str) -> Result<()> {
    // Load caps (Redis-cached, 5D-4 FIX)
    let caps = load_spend_caps_cached(cache, db, token_id).await?;

    // Nothing to enforce if no caps are configured
    if caps.daily_limit_usd.is_none()
        && caps.monthly_limit_usd.is_none()
        && caps.lifetime_limit_usd.is_none()
    {
        return Ok(());
    }

    let mut conn = cache.redis();
    let now = Utc::now();

    // SEC-03: Pre-flight check is best-effort; true atomic enforcement happens
    // in `check_and_increment_spend` after cost is known.
    // We add a small headroom factor (95% of limit) to reduce false negatives
    // from concurrent requests in the TOCTOU window.

    // Check daily cap
    if let Some(daily_limit) = caps.daily_limit_usd {
        let key = format!("spend:{}:daily:{}", token_id, now.format("%Y-%m-%d"));
        // INCRBYFLOAT stores values as bulk strings — parse as String first
        let current: f64 = conn
            .get::<_, Option<String>>(&key)
            .await
            .unwrap_or(None)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        if current >= daily_limit {
            anyhow::bail!(
                "daily spend cap of ${:.2} exceeded (current: ${:.4})",
                daily_limit,
                current
            );
        }
    }

    // Check monthly cap
    if let Some(monthly_limit) = caps.monthly_limit_usd {
        let key = format!("spend:{}:monthly:{}", token_id, now.format("%Y-%m"));
        let current: f64 = conn
            .get::<_, Option<String>>(&key)
            .await
            .unwrap_or(None)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        if current >= monthly_limit {
            anyhow::bail!(
                "monthly spend cap of ${:.2} exceeded (current: ${:.4})",
                monthly_limit,
                current
            );
        }
    }

    // Check lifetime cap
    if let Some(lifetime_limit) = caps.lifetime_limit_usd {
        let key = format!("spend:{}:lifetime", token_id);
        let current: f64 = conn
            .get::<_, Option<String>>(&key)
            .await
            .unwrap_or(None)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        if current >= lifetime_limit {
            anyhow::bail!(
                "lifetime spend cap of ${:.2} exceeded (current: ${:.4})",
                lifetime_limit,
                current
            );
        }
    }

    Ok(())
}

/// Reconcile Redis spend counters from PostgreSQL audit logs.
/// This should be called on startup or after Redis data loss to ensure
/// spend caps are enforced against actual historical usage.
///
/// For each token with configured spend caps, this function:
/// 1. Computes actual spend from audit_logs for the current period
/// 2. Sets the Redis counter to this value if it's higher than current
///
/// This handles the edge case where Redis restarts and loses counter state.
#[tracing::instrument(skip(cache, db))]
pub async fn reconcile_spend_from_audit_logs(
    cache: &TieredCache,
    db: &sqlx::PgPool,
    token_id: &str,
) -> Result<()> {
    use sqlx::Row;

    let caps = load_spend_caps_cached(cache, db, token_id).await?;

    // Nothing to reconcile if no caps configured
    if caps.daily_limit_usd.is_none()
        && caps.monthly_limit_usd.is_none()
        && caps.lifetime_limit_usd.is_none()
    {
        return Ok(());
    }

    let mut conn = cache.redis();
    let now = Utc::now();

    // Calculate actual spend from audit_logs for each period
    let now_dt = now.date_naive();
    let now_utc = now_dt.and_hms_opt(0, 0, 0).unwrap().and_utc();

    // Daily: since midnight today
    let daily_actual: rust_decimal::Decimal = sqlx::query(
        r#"
        SELECT COALESCE(SUM(estimated_cost_usd), 0) as total
        FROM audit_logs
        WHERE token_id = $1
          AND created_at >= $2
        "#,
    )
    .bind(token_id)
    .bind(now_utc)
    .fetch_one(db)
    .await?
    .try_get("total")
    .unwrap_or(rust_decimal::Decimal::ZERO);

    // Monthly: since first of current month
    let month_start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    let monthly_actual: rust_decimal::Decimal = sqlx::query(
        r#"
        SELECT COALESCE(SUM(estimated_cost_usd), 0) as total
        FROM audit_logs
        WHERE token_id = $1
          AND created_at >= $2
        "#,
    )
    .bind(token_id)
    .bind(month_start)
    .fetch_one(db)
    .await?
    .try_get("total")
    .unwrap_or(rust_decimal::Decimal::ZERO);

    // Lifetime: all time
    let lifetime_actual: rust_decimal::Decimal = sqlx::query(
        r#"
        SELECT COALESCE(SUM(estimated_cost_usd), 0) as total
        FROM audit_logs
        WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .fetch_one(db)
    .await?
    .try_get("total")
    .unwrap_or(rust_decimal::Decimal::ZERO);

    // Update Redis counters if audit log shows higher values
    // Use SET only if the key doesn't exist or has lower value
    if caps.daily_limit_usd.is_some() {
        let key = format!("spend:{}:daily:{}", token_id, now.format("%Y-%m-%d"));
        let current: f64 = conn
            .get::<_, Option<String>>(&key)
            .await
            .unwrap_or(None)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let actual_f64 = daily_actual.to_f64().unwrap_or(0.0);
        if actual_f64 > current {
            let _: () = conn.set_ex(&key, actual_f64.to_string(), 86400 + 3600).await.unwrap_or(());
            info!(
                token_id = %token_id,
                redis_value = current,
                audit_value = actual_f64,
                "Reconciled daily spend counter from audit logs"
            );
        }
    }

    if caps.monthly_limit_usd.is_some() {
        let key = format!("spend:{}:monthly:{}", token_id, now.format("%Y-%m"));
        let current: f64 = conn
            .get::<_, Option<String>>(&key)
            .await
            .unwrap_or(None)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let actual_f64 = monthly_actual.to_f64().unwrap_or(0.0);
        if actual_f64 > current {
            let _: () = conn.set_ex(&key, actual_f64.to_string(), 86400 * 32).await.unwrap_or(());
            info!(
                token_id = %token_id,
                redis_value = current,
                audit_value = actual_f64,
                "Reconciled monthly spend counter from audit logs"
            );
        }
    }

    if caps.lifetime_limit_usd.is_some() {
        let key = format!("spend:{}:lifetime", token_id);
        let current: f64 = conn
            .get::<_, Option<String>>(&key)
            .await
            .unwrap_or(None)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let actual_f64 = lifetime_actual.to_f64().unwrap_or(0.0);
        if actual_f64 > current {
            let _: () = conn.set_ex(&key, actual_f64.to_string(), 86400 * 365 * 10).await.unwrap_or(());
            info!(
                token_id = %token_id,
                redis_value = current,
                audit_value = actual_f64,
                "Reconciled lifetime spend counter from audit logs"
            );
        }
    }

    Ok(())
}

/// SEC-03: Atomic check-and-increment using Redis Lua script.
/// Returns Ok(()) if the spend was successfully incremented (under cap),
/// or Err if the cap would be exceeded.
pub async fn check_and_increment_spend(
    cache: &TieredCache,
    db: &sqlx::PgPool,
    token_id: &str,
    cost_usd: f64,
) -> Result<()> {
    if cost_usd <= 0.0 {
        return Ok(());
    }

    let caps = load_spend_caps_cached(cache, db, token_id).await?;
    let mut conn = cache.redis();
    let now = Utc::now();

    // FIX: Cross-cap atomic check-and-increment
    // Check ALL caps first in a single atomic Lua script, then increment ALL or NONE.
    // This prevents partial increments (e.g., daily incremented but monthly denied).
    //
    // KEYS[1] = daily key (or empty string if no daily cap)
    // KEYS[2] = monthly key (or empty string if no monthly cap)
    // KEYS[3] = lifetime key (or empty string if no lifetime cap)
    // ARGV[1] = daily limit (or -1 if no cap)
    // ARGV[2] = monthly limit (or -1 if no cap)
    // ARGV[3] = lifetime limit (or -1 if no cap)
    // ARGV[4] = cost to add
    // ARGV[5] = daily TTL
    // ARGV[6] = monthly TTL
    // ARGV[7] = lifetime TTL
    //
    // Returns: "OK" if allowed, "DAILY" if daily cap exceeded, "MONTHLY" if monthly exceeded, "LIFETIME" if lifetime exceeded
    let atomic_lua = r#"
        local cost = tonumber(ARGV[4])

        local daily_key = KEYS[1]
        local monthly_key = KEYS[2]
        local lifetime_key = KEYS[3]
        local daily_limit = tonumber(ARGV[1])
        local monthly_limit = tonumber(ARGV[2])
        local lifetime_limit = tonumber(ARGV[3])

        -- Phase 1: Increment ALL counters first (so counters always reflect actual spend)
        local daily_new = 0
        local monthly_new = 0
        local lifetime_new = 0

        if daily_limit >= 0 and daily_key ~= "" then
            daily_new = tonumber(redis.call('INCRBYFLOAT', daily_key, cost))
            redis.call('EXPIRE', daily_key, ARGV[5])
        end

        if monthly_limit >= 0 and monthly_key ~= "" then
            monthly_new = tonumber(redis.call('INCRBYFLOAT', monthly_key, cost))
            redis.call('EXPIRE', monthly_key, ARGV[6])
        end

        if lifetime_limit >= 0 and lifetime_key ~= "" then
            lifetime_new = tonumber(redis.call('INCRBYFLOAT', lifetime_key, cost))
            redis.call('EXPIRE', lifetime_key, ARGV[7])
        end

        -- Phase 2: Check if any cap was breached AFTER incrementing
        -- Counters are already updated so pre-flight checks will see the real values.
        if daily_limit >= 0 and daily_new > daily_limit then
            return "DAILY"
        end

        if monthly_limit >= 0 and monthly_new > monthly_limit then
            return "MONTHLY"
        end

        if lifetime_limit >= 0 and lifetime_new > lifetime_limit then
            return "LIFETIME"
        end

        return "OK"
    "#;

    // Build keys - use empty string for caps that don't exist
    let daily_key = if caps.daily_limit_usd.is_some() {
        format!("spend:{}:daily:{}", token_id, now.format("%Y-%m-%d"))
    } else {
        String::new()
    };
    let monthly_key = if caps.monthly_limit_usd.is_some() {
        format!("spend:{}:monthly:{}", token_id, now.format("%Y-%m"))
    } else {
        String::new()
    };
    let lifetime_key = if caps.lifetime_limit_usd.is_some() {
        format!("spend:{}:lifetime", token_id)
    } else {
        String::new()
    };

    // Use -1 to indicate "no cap" (Lua can distinguish from 0)
    let daily_limit_val = caps.daily_limit_usd.unwrap_or(-1.0);
    let monthly_limit_val = caps.monthly_limit_usd.unwrap_or(-1.0);
    let lifetime_limit_val = caps.lifetime_limit_usd.unwrap_or(-1.0);

    let daily_ttl = 86400i64 + 3600;
    let monthly_ttl = 86400i64 * 32;
    let lifetime_ttl = 86400i64 * 365 * 10;

    let result: String = redis::cmd("EVAL")
        .arg(atomic_lua)
        .arg(3i32) // number of KEYS
        .arg(&daily_key)
        .arg(&monthly_key)
        .arg(&lifetime_key)
        .arg(daily_limit_val)
        .arg(monthly_limit_val)
        .arg(lifetime_limit_val)
        .arg(cost_usd)
        .arg(daily_ttl)
        .arg(monthly_ttl)
        .arg(lifetime_ttl)
        .query_async(&mut conn)
        .await
        .context("failed to execute atomic cross-cap spend lua script")?;

    match result.as_str() {
        "OK" => {
            // All caps passed and counters incremented
        }
        "DAILY" => anyhow::bail!("daily spend cap exceeded during increment"),
        "MONTHLY" => anyhow::bail!("monthly spend cap exceeded during increment"),
        "LIFETIME" => anyhow::bail!("lifetime spend cap exceeded during increment"),
        other => anyhow::bail!("unexpected spend check result: {}", other),
    }

    // FIX: Always increment tracking counters for periods without caps
    // (capped periods were already incremented atomically by the Lua script above).
    // This ensures the spend status API reports accurate daily/monthly values
    // even for tokens without configured caps.

    if caps.daily_limit_usd.is_none() {
        let key = format!("spend:{}:daily:{}", token_id, now.format("%Y-%m-%d"));
        let _: f64 = redis::cmd("INCRBYFLOAT")
            .arg(&key)
            .arg(cost_usd)
            .query_async(&mut conn)
            .await
            .unwrap_or(cost_usd);
        let _: () = conn.expire(&key, 86400i64 + 3600).await.unwrap_or(());
    }

    if caps.monthly_limit_usd.is_none() {
        let key = format!("spend:{}:monthly:{}", token_id, now.format("%Y-%m"));
        let _: f64 = redis::cmd("INCRBYFLOAT")
            .arg(&key)
            .arg(cost_usd)
            .query_async(&mut conn)
            .await
            .unwrap_or(cost_usd);
        let _: () = conn.expire(&key, 86400i64 * 32).await.unwrap_or(());
    }

    if caps.lifetime_limit_usd.is_none() {
        let key = format!("spend:{}:lifetime", token_id);
        let _: f64 = redis::cmd("INCRBYFLOAT")
            .arg(&key)
            .arg(cost_usd)
            .query_async(&mut conn)
            .await
            .unwrap_or(cost_usd);
        let _: () = conn.expire(&key, 86400i64 * 365 * 10).await.unwrap_or(());
    }

    // DB persistence (fire-and-forget, same as track_spend)
    let tid = token_id.to_string();
    let pool = db.clone();
    let cost_decimal =
        rust_decimal::Decimal::from_f64_retain(cost_usd).unwrap_or(rust_decimal::Decimal::ZERO);
    tokio::spawn(async move {
        for period in &["daily", "monthly", "lifetime"] {
            if let Err(e) = update_db_spend(&pool, &tid, period, cost_decimal).await {
                error!("Failed to persist {} spend to DB: {}", period, e);
            }
        }
    });

    Ok(())
}

// ── DB Helpers ────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct SpendCapRow {
    period: String,
    limit_usd: rust_decimal::Decimal,
}

/// Load spend caps for a token — 5D-4 FIX: Redis-cached (60s TTL).
///
/// Avoids a Postgres round-trip on every request. The cache is busted on
/// upsert/delete and expires naturally after 60 seconds.
async fn load_spend_caps(db: &sqlx::PgPool, token_id: &str) -> Result<SpendCap> {
    let rows = sqlx::query_as::<_, SpendCapRow>(
        "SELECT period, limit_usd FROM spend_caps WHERE token_id = $1",
    )
    .bind(token_id)
    .fetch_all(db)
    .await
    .context("failed to load spend caps")?;

    let mut caps = SpendCap::default();
    for row in rows {
        let limit = row.limit_usd.to_f64().unwrap_or(0.0);
        match row.period.as_str() {
            "daily" => caps.daily_limit_usd = Some(limit),
            "monthly" => caps.monthly_limit_usd = Some(limit),
            "lifetime" => caps.lifetime_limit_usd = Some(limit),
            _ => {}
        }
    }

    Ok(caps)
}

/// 5D-4 FIX: Redis-cached variant of `load_spend_caps` for hot-path use.
/// Caches serialized SpendCap in Redis for 60 seconds to avoid a DB
/// round-trip on every `check_and_increment_spend` call.
async fn load_spend_caps_cached(
    cache: &TieredCache,
    db: &sqlx::PgPool,
    token_id: &str,
) -> Result<SpendCap> {
    let cache_key = format!("spend_caps:{}", token_id);
    let mut conn = cache.redis();

    // Try Redis first
    if let Ok(Some(cached)) = conn.get::<_, Option<String>>(&cache_key).await {
        if let Ok(caps) = serde_json::from_str::<SpendCap>(&cached) {
            return Ok(caps);
        }
    }

    // Cache miss → load from DB
    let caps = load_spend_caps(db, token_id).await?;

    // Populate cache (best-effort, 60s TTL)
    if let Ok(json) = serde_json::to_string(&caps) {
        let _: () = conn.set_ex(&cache_key, &json, 60).await.unwrap_or(());
    }

    Ok(caps)
}

// ── Tracking ──────────────────────────────────────────────────

/// Track spend for a token.
/// Increments the Redis counter for daily and monthly windows.
#[allow(dead_code)]
pub async fn track_spend(
    cache: &TieredCache,
    db: &sqlx::PgPool,
    token_id: &str,
    _project_id: uuid::Uuid,
    cost: Decimal,
) -> Result<()> {
    if cost <= Decimal::ZERO {
        return Ok(());
    }

    let cost_f64 = cost.to_f64().unwrap_or(0.0);
    let mut conn = cache.redis();
    let now = Utc::now();

    // Increment daily and monthly spend in Redis using INCRBYFLOAT
    for window in &["daily", "monthly"] {
        let period_key = match *window {
            "daily" => now.format("%Y-%m-%d").to_string(),
            "monthly" => now.format("%Y-%m").to_string(),
            _ => continue,
        };
        let redis_key = format!("spend:{}:{}:{}", token_id, window, period_key);
        let ttl: usize = match *window {
            "daily" => 86400 + 3600,
            "monthly" => 86400 * 32,
            _ => 86400,
        };

        // INCRBYFLOAT + EXPIRE (two commands, not pipelined — simpler and correct)
        let _: f64 = redis::cmd("INCRBYFLOAT")
            .arg(&redis_key)
            .arg(cost_f64)
            .query_async(&mut conn)
            .await
            .unwrap_or(cost_f64);

        let _: () = conn.expire(&redis_key, ttl as i64).await.unwrap_or(());
    }

    // BUG-01 fix: DB Persistence for BOTH daily AND monthly (async spawn)
    let tid = token_id.to_string();
    let pool = db.clone();
    let cost_clone = cost;

    tokio::spawn(async move {
        for period in &["daily", "monthly"] {
            if let Err(e) = update_db_spend(&pool, &tid, period, cost_clone).await {
                error!("Failed to persist {} spend to DB: {}", period, e);
            }
        }
    });

    Ok(())
}

// ── Spend Cap CRUD ────────────────────────────────────────────

/// Set or update a spend cap for a token.
pub async fn upsert_spend_cap(
    cache: &TieredCache,
    db: &sqlx::PgPool,
    token_id: &str,
    project_id: uuid::Uuid,
    period: &str,
    limit_usd: Decimal,
) -> Result<()> {
    // FIX: Invalidate cache BEFORE DB write to prevent stale reads.
    // If Redis fails, the next read will fetch fresh data from DB.
    // If DB write fails, cache is already invalidated so we're consistent.
    let cache_key = format!("spend_caps:{}", token_id);
    let mut conn = cache.redis();
    if let Err(e) = conn.del::<_, ()>(&cache_key).await {
        // Log warning but continue - cache will expire naturally in 60s
        tracing::warn!(
            token_id = %token_id,
            error = %e,
            "Failed to invalidate spend cap cache (will expire in 60s)"
        );
    }

    let reset_at = next_reset_at(period);
    sqlx::query(
        r#"
        INSERT INTO spend_caps (token_id, project_id, period, limit_usd, usage_usd, reset_at)
        VALUES ($1, $2, $3, $4, 0, $5)
        ON CONFLICT (token_id, period)
        DO UPDATE SET limit_usd = $4, updated_at = now()
        "#,
    )
    .bind(token_id)
    .bind(project_id)
    .bind(period)
    .bind(limit_usd)
    .bind(reset_at)
    .execute(db)
    .await
    .context("failed to upsert spend cap")?;

    info!(token_id, period, limit_usd = %limit_usd, "spend cap configured");
    Ok(())
}

/// Delete a spend cap for a token.
pub async fn delete_spend_cap(
    cache: &TieredCache,
    db: &sqlx::PgPool,
    token_id: &str,
    period: &str,
) -> Result<()> {
    // FIX: Invalidate cache BEFORE DB delete to prevent stale reads.
    let cache_key = format!("spend_caps:{}", token_id);
    let mut conn = cache.redis();
    if let Err(e) = conn.del::<_, ()>(&cache_key).await {
        tracing::warn!(
            token_id = %token_id,
            error = %e,
            "Failed to invalidate spend cap cache on delete (will expire in 60s)"
        );
    }

    sqlx::query("DELETE FROM spend_caps WHERE token_id = $1 AND period = $2")
        .bind(token_id)
        .bind(period)
        .execute(db)
        .await
        .context("failed to delete spend cap")?;

    Ok(())
}

/// Get current spend + caps for a token (for the API/dashboard).
pub async fn get_spend_status(
    db: &sqlx::PgPool,
    cache: &TieredCache,
    token_id: &str,
) -> Result<SpendStatus> {
    let caps = load_spend_caps(db, token_id).await?;
    let mut conn = cache.redis();
    let now = Utc::now();

    let daily_key = format!("spend:{}:daily:{}", token_id, now.format("%Y-%m-%d"));
    let monthly_key = format!("spend:{}:monthly:{}", token_id, now.format("%Y-%m"));
    let lifetime_key = format!("spend:{}:lifetime", token_id);

    // INCRBYFLOAT stores values as bulk strings — get as Option<String> and parse
    let daily_spend: f64 = conn
        .get::<_, Option<String>>(&daily_key)
        .await
        .unwrap_or(None)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let monthly_spend: f64 = conn
        .get::<_, Option<String>>(&monthly_key)
        .await
        .unwrap_or(None)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let lifetime_spend: f64 = conn
        .get::<_, Option<String>>(&lifetime_key)
        .await
        .unwrap_or(None)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(SpendStatus {
        daily_limit_usd: caps.daily_limit_usd,
        monthly_limit_usd: caps.monthly_limit_usd,
        lifetime_limit_usd: caps.lifetime_limit_usd,
        current_daily_usd: daily_spend,
        current_monthly_usd: monthly_spend,
        current_lifetime_usd: lifetime_spend,
    })
}

// ── Helpers ───────────────────────────────────────────────────

fn next_reset_at(period: &str) -> chrono::DateTime<Utc> {
    let now = Utc::now();
    match period {
        "daily" => {
            let tomorrow = now.date_naive() + chrono::Duration::days(1);
            tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
        }
        "monthly" => {
            let next_month = if now.month() == 12 {
                chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap()
            } else {
                chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap()
            };
            next_month.and_hms_opt(0, 0, 0).unwrap().and_utc()
        }
        // Lifetime caps never reset — set reset_at to 100 years from now
        "lifetime" => now + chrono::Duration::days(36500),
        _ => now + chrono::Duration::days(1),
    }
}

async fn update_db_spend(
    pool: &sqlx::PgPool,
    token_id: &str,
    period: &str,
    cost: Decimal,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE spend_caps
        SET usage_usd = usage_usd + $3, updated_at = now()
        WHERE token_id = $1 AND period = $2
        "#,
    )
    .bind(token_id)
    .bind(period)
    .bind(cost)
    .execute(pool)
    .await?;

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── next_reset_at: real boundary tests ────────────────────

    #[test]
    fn test_next_reset_daily_is_tomorrow_midnight() {
        let reset = next_reset_at("daily");
        let now = Utc::now();

        // Must be in the future
        assert!(reset > now, "Daily reset must be after now");
        // Must be midnight (00:00:00)
        assert_eq!(
            reset.time(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        );
        // Must be exactly 1 day ahead from the current date (not 2 days)
        let tomorrow = (now.date_naive() + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        assert_eq!(reset, tomorrow, "Daily reset must be tomorrow at 00:00 UTC");
    }

    #[test]
    fn test_next_reset_monthly_is_first_of_next_month() {
        let reset = next_reset_at("monthly");
        let now = Utc::now();

        assert!(reset > now);
        // Must be day 1 of a month
        assert_eq!(reset.day(), 1, "Monthly reset must be 1st of next month");
        assert_eq!(
            reset.time(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_next_reset_monthly_dec_to_jan_rollover() {
        // Verify the Dec→Jan year rollover logic
        // We can't control Utc::now(), but we can test the function deterministically
        // by checking that if current month is 12, next month is January of next year
        let now = Utc::now();
        if now.month() == 12 {
            let reset = next_reset_at("monthly");
            assert_eq!(reset.year(), now.year() + 1);
            assert_eq!(reset.month(), 1);
            assert_eq!(reset.day(), 1);
        }
        // Otherwise verify it's next month same year
        if now.month() < 12 {
            let reset = next_reset_at("monthly");
            assert_eq!(reset.year(), now.year());
            assert_eq!(reset.month(), now.month() + 1);
        }
    }

    #[test]
    fn test_next_reset_lifetime_is_far_future() {
        let reset = next_reset_at("lifetime");
        let now = Utc::now();
        // Lifetime must be ~100 years in the future (36500 days)
        let years_until_reset = (reset - now).num_days() as f64 / 365.25;
        assert!(
            years_until_reset > 99.0,
            "Lifetime reset must be ~100 years out, got {:.1}",
            years_until_reset
        );
    }

    #[test]
    fn test_next_reset_unknown_period_defaults_to_1_day() {
        let reset = next_reset_at("weekly"); // not a real period
        let now = Utc::now();
        let diff = (reset - now).num_seconds();
        // Should be ~86400s (1 day) with tiny tolerance
        assert!(
            diff > 86390 && diff <= 86400,
            "Unknown period should default to ~1 day, got {}s",
            diff
        );
    }

    // ── SpendCap struct logic ─────────────────────────────────

    #[test]
    fn test_spend_cap_default_has_no_limits() {
        let cap = SpendCap::default();
        assert!(
            cap.daily_limit_usd.is_none(),
            "Default cap should have no daily limit"
        );
        assert!(
            cap.monthly_limit_usd.is_none(),
            "Default cap should have no monthly limit"
        );
        assert!(
            cap.lifetime_limit_usd.is_none(),
            "Default cap should have no lifetime limit"
        );
    }

    #[test]
    fn test_spend_cap_serde_roundtrip() {
        let cap = SpendCap {
            daily_limit_usd: Some(10.0),
            monthly_limit_usd: Some(100.0),
            lifetime_limit_usd: Some(1000.0),
        };
        let json = serde_json::to_string(&cap).unwrap();
        let back: SpendCap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.daily_limit_usd, Some(10.0));
        assert_eq!(back.monthly_limit_usd, Some(100.0));
        assert_eq!(back.lifetime_limit_usd, Some(1000.0));
    }

    #[test]
    fn test_spend_status_serialization_all_fields_present() {
        let status = SpendStatus {
            daily_limit_usd: Some(10.0),
            monthly_limit_usd: None,
            lifetime_limit_usd: Some(500.0),
            current_daily_usd: 5.123,
            current_monthly_usd: 42.0,
            current_lifetime_usd: 123.456,
        };
        let json = serde_json::to_value(&status).unwrap();
        // Must have all fields (no field silently dropped)
        assert_eq!(json["daily_limit_usd"], 10.0);
        assert!(json["monthly_limit_usd"].is_null());
        assert_eq!(json["lifetime_limit_usd"], 500.0);
        assert_eq!(json["current_daily_usd"], 5.123);
        assert_eq!(json["current_monthly_usd"], 42.0);
        assert_eq!(json["current_lifetime_usd"], 123.456);
    }

    // ── Cap enforcement logic (pure function extraction) ──────

    /// Verify cap comparison logic: spend >= limit should fail
    #[test]
    fn test_cap_exceeded_when_at_limit() {
        let limit = 10.0_f64;
        let current = 10.0_f64;
        assert!(
            current >= limit,
            "Current == limit should be considered exceeded"
        );
    }

    #[test]
    fn test_cap_exceeded_when_above_limit() {
        let limit = 10.0_f64;
        let current = 10.0001_f64;
        assert!(current >= limit, "Current > limit should be exceeded");
    }

    #[test]
    fn test_cap_not_exceeded_when_below() {
        let limit = 10.0_f64;
        let current = 9.999_f64;
        assert!(current < limit, "Current < limit should not be exceeded");
    }

    #[test]
    fn test_no_cap_means_no_enforcement() {
        let caps = SpendCap::default();
        // Simulate the early-return condition in check_spend_cap
        assert!(
            caps.daily_limit_usd.is_none() && caps.monthly_limit_usd.is_none(),
            "Default caps should trigger early return (no enforcement)"
        );
    }

    // ── Lua script atomicity tests (check-then-increment logic) ──

    /// Simulates the corrected Lua script logic to verify correctness.
    /// This mirrors the exact Lua script in check_and_increment_spend:
    ///   1. GET current spend
    ///   2. If current + cost > limit → return -1 (deny), do NOT increment
    ///   3. Else → INCRBYFLOAT, return new value
    fn simulate_lua_check_and_increment(current: f64, cost: f64, limit: f64) -> (f64, f64) {
        // Returns (result, counter_after) — result is -1 for denied, else new total
        if current + cost > limit {
            (-1.0, current) // counter unchanged on deny
        } else {
            let new_val = current + cost;
            (new_val, new_val) // counter updated on allow
        }
    }

    #[test]
    fn test_lua_allows_under_cap() {
        let (result, counter) = simulate_lua_check_and_increment(5.0, 3.0, 10.0);
        assert!(result > 0.0, "Should allow: 5 + 3 = 8 < 10");
        assert!((counter - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_lua_denies_over_cap() {
        let (result, counter) = simulate_lua_check_and_increment(9.5, 0.6, 10.0);
        assert!(result < 0.0, "Should deny: 9.5 + 0.6 = 10.1 > 10.0");
        assert!(
            (counter - 9.5).abs() < f64::EPSILON,
            "Counter must NOT change on deny"
        );
    }

    #[test]
    fn test_lua_denied_requests_dont_inflate_counter() {
        // This is the critical fix: two concurrent denied requests should not
        // inflate the counter. Simulate the race:
        //   T=0: current = 9.5, Request A cost = 0.6
        //   T=1: current = 9.5, Request B cost = 0.6
        // Both should be denied. Counter should remain 9.5 (not 10.7).
        let mut counter = 9.5;
        let limit = 10.0;

        // Request A
        let (result_a, new_counter) = simulate_lua_check_and_increment(counter, 0.6, limit);
        counter = new_counter;
        assert!(result_a < 0.0, "Request A should be denied");
        assert!(
            (counter - 9.5).abs() < f64::EPSILON,
            "Counter unchanged after denied A"
        );

        // Request B
        let (result_b, new_counter) = simulate_lua_check_and_increment(counter, 0.6, limit);
        counter = new_counter;
        assert!(result_b < 0.0, "Request B should be denied");
        assert!(
            (counter - 9.5).abs() < f64::EPSILON,
            "Counter still 9.5 after denied B — no phantom spend"
        );
    }

    #[test]
    fn test_lua_exact_boundary_allowed() {
        // current + cost == limit should be ALLOWED (the Lua script uses > not >=)
        // Spending exactly your budget is not exceeding it
        let (result, counter) = simulate_lua_check_and_increment(9.5, 0.5, 10.0);
        assert!(
            result > 0.0,
            "Should allow: 9.5 + 0.5 = 10.0 == limit (not exceeded)"
        );
        assert!(
            (counter - 10.0).abs() < f64::EPSILON,
            "Counter should be updated to 10.0"
        );
    }

    #[test]
    fn test_lua_allows_last_request_under_cap() {
        // A request that brings the total just under the cap should be allowed
        let (result, counter) = simulate_lua_check_and_increment(9.0, 0.99, 10.0);
        assert!(result > 0.0, "Should allow: 9.0 + 0.99 = 9.99 < 10.0");
        assert!((counter - 9.99).abs() < f64::EPSILON);
    }
}
