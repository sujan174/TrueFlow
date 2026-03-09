use super::types::{
    SpendByDimension, TokenLatencyStat, TokenStatusStat, TokenSummary, TokenVolumeStat,
};
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    // -- Analytics Operations --

    pub async fn get_request_volume_24h(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::analytics::VolumeStat>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::VolumeStat>(
            r#"
            SELECT 
                date_trunc('hour', created_at) as bucket,
                count(*) as count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            GROUP BY 1
            ORDER BY 1 ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_status_code_distribution_24h(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::analytics::StatusStat>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::StatusStat>(
            r#"
            SELECT 
                CAST(floor(COALESCE(upstream_status, 0) / 100) * 100 AS INTEGER) as status_class,
                count(*) as count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            GROUP BY 1
            ORDER BY 1 ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_latency_percentiles_24h(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<crate::models::analytics::LatencyStat> {
        // We use percentile_cont. Requires float8, response_latency_ms is int4.
        // We return a single row with p50, p90, p99, avg.
        let row = sqlx::query_as::<_, crate::models::analytics::LatencyStat>(
            r#"
            SELECT 
                COALESCE(percentile_cont(0.50) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p50,
                COALESCE(percentile_cont(0.90) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p90,
                COALESCE(percentile_cont(0.99) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p99,
                COALESCE(AVG(response_latency_ms)::float8, 0) as avg
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            "#
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    // -- Token Usage Analytics --

    pub async fn get_token_usage(
        &self,
        token_id: &str,
        project_id: Uuid,
    ) -> anyhow::Result<crate::models::analytics::TokenUsageStats> {
        // Aggregate stats
        let stats = sqlx::query_as::<_, (i64, i64, i64, f64, f64)>(
            r#"SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE upstream_status >= 200 AND upstream_status < 400) as success,
                COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL) as errors,
                COALESCE(AVG(response_latency_ms)::float8, 0) as avg_latency,
                COALESCE(SUM(estimated_cost_usd)::float8, 0) as total_cost
            FROM audit_logs
            WHERE token_id = $1 AND project_id = $2
              AND created_at > now() - interval '24 hours'"#,
        )
        .bind(token_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;

        // Hourly buckets for sparkline
        let hourly = sqlx::query_as::<_, crate::models::analytics::TokenUsageBucket>(
            r#"SELECT
                date_trunc('hour', created_at) as bucket,
                COUNT(*) as count
            FROM audit_logs
            WHERE token_id = $1 AND project_id = $2
              AND created_at > now() - interval '24 hours'
            GROUP BY 1
            ORDER BY 1 ASC"#,
        )
        .bind(token_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(crate::models::analytics::TokenUsageStats {
            total_requests: stats.0,
            success_count: stats.1,
            error_count: stats.2,
            avg_latency_ms: stats.3,
            total_cost_usd: stats.4,
            hourly,
        })
    }

    pub async fn get_analytics_summary(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<crate::models::analytics::AnalyticsSummary> {
        let row = sqlx::query_as::<_, crate::models::analytics::AnalyticsSummary>(
            r#"
            SELECT 
                count(*)::bigint as total_requests,
                count(*) filter (where upstream_status >= 200 and upstream_status < 400)::bigint as success_count,
                count(*) filter (where upstream_status >= 400 or upstream_status is null)::bigint as error_count,
                coalesce(avg(response_latency_ms), 0.0)::float8 as avg_latency,
                coalesce(sum(estimated_cost_usd), 0.0)::float8 as total_cost,
                coalesce(sum(prompt_tokens + completion_tokens), 0)::bigint as total_tokens
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_analytics_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::AnalyticsTimeseriesPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, crate::models::analytics::AnalyticsTimeseriesPoint>(
            r#"
            SELECT 
                date_trunc($3, created_at) as bucket,
                count(*)::bigint as request_count,
                count(*) filter (where upstream_status >= 400)::bigint as error_count,
                coalesce(sum(estimated_cost_usd), 0.0)::float8 as cost,
                coalesce(avg(response_latency_ms), 0.0)::float8 as lat
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY 1
            ORDER BY 1 ASC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .bind(bucket)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Spend Breakdown Queries ──────────────────────────────────────────────

    /// Spend breakdown grouped by model over a time window.
    pub async fn get_spend_by_model(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                COALESCE(model, 'unknown')              AS dimension,
                COALESCE(SUM(estimated_cost_usd), 0)::float8  AS total_cost_usd,
                COUNT(*)::bigint                        AS request_count,
                COALESCE(SUM(prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint AS total_completion_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND estimated_cost_usd IS NOT NULL
            GROUP BY model
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Spend breakdown grouped by token_id over a time window.
    pub async fn get_spend_by_token(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                token_id                                AS dimension,
                COALESCE(SUM(estimated_cost_usd), 0)::float8  AS total_cost_usd,
                COUNT(*)::bigint                        AS request_count,
                COALESCE(SUM(prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint AS total_completion_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND estimated_cost_usd IS NOT NULL
            GROUP BY token_id
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Spend breakdown grouped by a tag key extracted from custom_properties JSONB.
    /// e.g. group_by_tag = "team" → groups by custom_properties->>'team'
    pub async fn get_spend_by_tag(
        &self,
        project_id: Uuid,
        hours: i32,
        tag_key: &str,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                COALESCE(custom_properties->>$3, 'untagged') AS dimension,
                COALESCE(SUM(estimated_cost_usd), 0)::float8     AS total_cost_usd,
                COUNT(*)::bigint                             AS request_count,
                COALESCE(SUM(prompt_tokens), 0)::bigint      AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::bigint  AS total_completion_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND estimated_cost_usd IS NOT NULL
            GROUP BY custom_properties->>$3
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .bind(tag_key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Per-Token Analytics ──────────────────────────────────────

    pub async fn get_token_summary(&self, project_id: Uuid) -> anyhow::Result<Vec<TokenSummary>> {
        // sqlx doesn't map directly to struct with aggregate functions easily without `AS` aliases matching struct fields exactly.
        // We'll use query_as! with explicit mapping if needed, or ensuring column names match.
        // Note: avg returns numeric/float, COUNT returns bigint (i64).
        // latency might be null if 0 requests, COALESCE to 0.
        let rows = sqlx::query_as!(
            TokenSummary,
            r#"SELECT 
                token_id, 
                COUNT(*) as "total_requests!",
                COUNT(*) FILTER (WHERE upstream_status >= 400) as "errors!",
                COALESCE(AVG(response_latency_ms)::float8, 0.0) as "avg_latency_ms!",
                MAX(created_at) as last_active
             FROM audit_logs
             WHERE project_id = $1 AND created_at > now() - interval '24 hours'
             GROUP BY token_id 
             ORDER BY 2 DESC"#,
            project_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_token_volume_24h(
        &self,
        project_id: Uuid,
        token_id: &str,
    ) -> anyhow::Result<Vec<TokenVolumeStat>> {
        let rows = sqlx::query_as!(
            TokenVolumeStat,
            r#"SELECT 
                date_trunc('hour', created_at) as "hour!", 
                COUNT(*) as "count!"
             FROM audit_logs
             WHERE project_id = $1 AND token_id = $2
               AND created_at > now() - interval '24 hours'
             GROUP BY 1 
             ORDER BY 1"#,
            project_id,
            token_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_token_status_distribution_24h(
        &self,
        project_id: Uuid,
        token_id: &str,
    ) -> anyhow::Result<Vec<TokenStatusStat>> {
        let rows = sqlx::query_as!(
            TokenStatusStat,
            r#"SELECT 
                COALESCE(upstream_status, 0)::smallint as "status!", 
                COUNT(*) as "count!"
             FROM audit_logs
             WHERE project_id = $1 AND token_id = $2
               AND created_at > now() - interval '24 hours'
             GROUP BY 1 
             ORDER BY 2 DESC"#,
            project_id,
            token_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_token_latency_percentiles_24h(
        &self,
        project_id: Uuid,
        token_id: &str,
    ) -> anyhow::Result<TokenLatencyStat> {
        let row = sqlx::query!(
            r#"SELECT 
                percentile_cont(0.5) WITHIN GROUP (ORDER BY response_latency_ms) as p50,
                percentile_cont(0.9) WITHIN GROUP (ORDER BY response_latency_ms) as p90,
                percentile_cont(0.99) WITHIN GROUP (ORDER BY response_latency_ms) as p99
             FROM audit_logs
             WHERE project_id = $1 AND token_id = $2
               AND created_at > now() - interval '24 hours'"#,
            project_id,
            token_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(TokenLatencyStat {
            p50: row.p50.unwrap_or(0.0),
            p90: row.p90.unwrap_or(0.0),
            p99: row.p99.unwrap_or(0.0),
        })
    }

    pub async fn get_analytics_experiments(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::analytics::ExperimentSummary>> {
        // Group by experiment_name and variant_name
        // For baseline (null variants), we group them together under an empty string or 'baseline'
        let rows = sqlx::query_as::<_, crate::models::analytics::ExperimentSummary>(
            r#"SELECT 
                experiment_name as "experiment_name!",
                COALESCE(variant_name, 'baseline') as "variant_name!",
                COUNT(*) as "total_requests!",
                COALESCE(AVG(response_latency_ms)::float8, 0.0) as "avg_latency_ms!",
                COALESCE(SUM(cost_usd)::float8, 0.0) as "total_cost_usd!",
                COALESCE(AVG(prompt_tokens + completion_tokens)::float8, 0.0) as "avg_tokens!",
                COUNT(*) FILTER (WHERE upstream_status >= 400) as "error_count!"
             FROM audit_logs
             WHERE project_id = $1 AND experiment_name IS NOT NULL
             GROUP BY experiment_name, variant_name
             ORDER BY experiment_name, variant_name"#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
