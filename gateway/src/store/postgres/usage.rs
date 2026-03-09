use super::types::UsageMeterRow;
use super::PgStore;
use chrono::NaiveDate;
use uuid::Uuid;

impl PgStore {
    // ── Usage Metering ───────────────────────────────────────────

    pub async fn increment_usage(
        &self,
        org_id: Uuid,
        period: NaiveDate,
        requests: i64,
        tokens: i64,
        spend_usd: rust_decimal::Decimal,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            r#"INSERT INTO usage_meters (org_id, period, total_requests, total_tokens_used, total_spend_usd)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (org_id, period) DO UPDATE SET
                   total_requests = usage_meters.total_requests + $3,
                   total_tokens_used = usage_meters.total_tokens_used + $4,
                   total_spend_usd = usage_meters.total_spend_usd + $5,
                   updated_at = NOW()"#,
            org_id,
            period,
            requests,
            tokens,
            spend_usd
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_usage(
        &self,
        org_id: Uuid,
        period: NaiveDate,
    ) -> anyhow::Result<Option<UsageMeterRow>> {
        let usage = sqlx::query_as!(
            UsageMeterRow,
            "SELECT * FROM usage_meters WHERE org_id = $1 AND period = $2",
            org_id,
            period
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(usage)
    }

    /// Aggregate usage directly from audit_logs for the given calendar month.
    /// Used as a fallback / live view when usage_meters has not been populated.
    pub async fn get_usage_from_audit_logs(
        &self,
        org_id: Uuid,
        period: NaiveDate,
    ) -> anyhow::Result<(i64, i64, rust_decimal::Decimal)> {
        use chrono::Datelike;
        // Calculate the first day of the next month as the exclusive upper bound
        let period_end = {
            let (y, m) = if period.month() == 12 {
                (period.year() + 1, 1u32)
            } else {
                (period.year(), period.month() + 1)
            };
            chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap()
        };

        let period_start_dt = period.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end_dt = period_end.and_hms_opt(0, 0, 0).unwrap().and_utc();

        // Use sqlx::query() (runtime form) so SQLX_OFFLINE=true builds aren't affected.
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint                                                   AS total_requests,
                COALESCE(SUM(a.prompt_tokens + a.completion_tokens), 0)::bigint    AS total_tokens,
                COALESCE(SUM(a.estimated_cost_usd), 0)                             AS total_spend_usd
            FROM audit_logs a
            JOIN projects p ON p.id = a.project_id
            WHERE p.org_id = $1
              AND a.created_at >= $2
              AND a.created_at <  $3
            "#,
        )
        .bind(org_id)
        .bind(period_start_dt)
        .bind(period_end_dt)
        .fetch_one(&self.pool)
        .await?;

        use sqlx::Row;
        let total_requests: i64 = row.try_get("total_requests").unwrap_or(0);
        let total_tokens: i64 = row.try_get("total_tokens").unwrap_or(0);
        let total_spend: rust_decimal::Decimal = row
            .try_get("total_spend_usd")
            .unwrap_or(rust_decimal::Decimal::ZERO);

        Ok((total_requests, total_tokens, total_spend))
    }
}
