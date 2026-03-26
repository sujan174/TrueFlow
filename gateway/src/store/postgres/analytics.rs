use super::types::{
    EngagementTiersResponse, RateLimitedToken, RequestsPerUserPoint, SpendByDimension,
    TokenAlertsResponse, TokenLatencyStat, TokenStatusStat, TokenSummary, TokenVolumeStat,
    UserGrowthPoint, UserSpendSummary,
};
use crate::models::analytics::{
    CacheHitRatePoint, CachedQueryRow, CacheLatencyComparison, CacheSummaryStats,
    CostLatencyScatterPoint, DataResidencyStats, ErrorLogRow, ErrorTimeseriesPoint,
    ErrorTypeBreakdown, GuardrailTriggerStat, ModelCacheEfficiency,
    ModelErrorRate, ModelLatencyStat, ModelStatsRow, ModelUsageTimeseriesPoint, PiiBreakdownStat,
    PolicyActionStat, SecuritySummaryStats, ShadowPolicyStat,
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

    /// Get experiment timeseries data for charts.
    /// Groups audit logs by hour and variant for the specified experiment.
    pub async fn get_experiment_timeseries(
        &self,
        project_id: Uuid,
        experiment_name: &str,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::ExperimentTimeseriesPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, crate::models::analytics::ExperimentTimeseriesPoint>(
            r#"
            SELECT
                date_trunc($4, created_at) as bucket,
                COALESCE(variant_name, 'baseline') as variant_name,
                COUNT(*)::bigint as request_count,
                COALESCE(AVG(response_latency_ms)::float8, 0.0) as avg_latency_ms,
                COALESCE(SUM(cost_usd)::float8, 0.0) as total_cost_usd
            FROM audit_logs
            WHERE project_id = $1
              AND experiment_name = $2
              AND created_at > now() - ($3 || ' hours')::interval
            GROUP BY 1, 2
            ORDER BY 1 ASC, 2 ASC
            "#,
        )
        .bind(project_id)
        .bind(experiment_name)
        .bind(hours.to_string())
        .bind(bucket)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── User Attribution Analytics (SaaS Builder Support) ─────────────────────────

    /// Spend breakdown grouped by external_user_id (customer-level analytics).
    /// Joins audit_logs with tokens to get the external_user_id from the token.
    pub async fn get_spend_by_external_user(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                COALESCE(t.external_user_id, 'unknown') AS dimension,
                COALESCE(SUM(a.estimated_cost_usd), 0)::float8 AS total_cost_usd,
                COUNT(*)::bigint AS request_count,
                COALESCE(SUM(a.prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(a.completion_tokens), 0)::bigint AS total_completion_tokens
            FROM audit_logs a
            LEFT JOIN tokens t ON t.id = a.token_id
            WHERE a.project_id = $1
              AND a.created_at > now() - ($2 || ' hours')::interval
              AND a.estimated_cost_usd IS NOT NULL
            GROUP BY t.external_user_id
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Spend breakdown grouped by team_id.
    /// Joins audit_logs with tokens to get the team_id from the token.
    pub async fn get_spend_by_team(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<SpendByDimension>> {
        let rows = sqlx::query_as::<_, SpendByDimension>(
            r#"
            SELECT
                COALESCE(t.team_id::text, 'no-team') AS dimension,
                COALESCE(SUM(a.estimated_cost_usd), 0)::float8 AS total_cost_usd,
                COUNT(*)::bigint AS request_count,
                COALESCE(SUM(a.prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(a.completion_tokens), 0)::bigint AS total_completion_tokens
            FROM audit_logs a
            LEFT JOIN tokens t ON t.id = a.token_id
            WHERE a.project_id = $1
              AND a.created_at > now() - ($2 || ' hours')::interval
              AND a.estimated_cost_usd IS NOT NULL
            GROUP BY t.team_id
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Detailed user spend summary for GET /api/v1/analytics/users.
    /// Returns aggregated spend per external_user_id with token count.
    pub async fn get_user_spend_summary(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<UserSpendSummary>> {
        let rows = sqlx::query_as::<_, UserSpendSummary>(
            r#"
            SELECT
                COALESCE(t.external_user_id, 'unknown') AS external_user_id,
                COALESCE(SUM(a.estimated_cost_usd), 0)::float8 AS total_cost_usd,
                COUNT(*)::bigint AS request_count,
                COALESCE(SUM(a.prompt_tokens), 0)::bigint AS total_prompt_tokens,
                COALESCE(SUM(a.completion_tokens), 0)::bigint AS total_completion_tokens,
                COUNT(DISTINCT a.token_id)::bigint AS token_count
            FROM audit_logs a
            LEFT JOIN tokens t ON t.id = a.token_id
            WHERE a.project_id = $1
              AND a.created_at > now() - ($2 || ' hours')::interval
              AND a.estimated_cost_usd IS NOT NULL
            GROUP BY t.external_user_id
            ORDER BY total_cost_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Provider Analytics for Dashboard ────────────────────────────────────

    /// Model usage breakdown - requests and cost per model.
    /// Used by the Model Usage card on the Analytics dashboard.
    pub async fn get_model_usage_stats(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::ModelUsageStat>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::ModelUsageStat>(
            r#"
            SELECT
                COALESCE(model, 'unknown') AS model,
                COUNT(*)::bigint AS request_count,
                COALESCE(SUM(estimated_cost_usd), 0)::float8 AS cost_usd
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY model
            ORDER BY request_count DESC
            LIMIT 10
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Spend by provider - derives provider from model name prefix.
    /// Used by the Spend by Provider card on the Analytics dashboard.
    pub async fn get_spend_by_provider(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::ProviderSpendStat>> {
        // Derive provider from model name using SQL CASE expressions
        // This matches the detect_provider() logic in Rust
        let rows = sqlx::query_as::<_, crate::models::analytics::ProviderSpendStat>(
            r#"
            SELECT
                provider,
                SUM(spend_usd)::float8 AS spend_usd,
                CASE
                    WHEN SUM(tokens) > 0 THEN ROUND((SUM(spend_usd) / SUM(tokens) * 1000)::numeric, 2)::float8
                    ELSE 0
                END AS rate_per_1k
            FROM (
                SELECT
                    CASE
                        WHEN model ILIKE 'gpt-%' OR model ILIKE 'o1-%' OR model ILIKE 'o3-%' THEN 'openai'
                        WHEN model ILIKE 'claude-%' THEN 'anthropic'
                        WHEN model ILIKE 'gemini-%' THEN 'google'
                        WHEN model ILIKE 'mistral-%' OR model ILIKE 'mixtral-%' THEN 'mistral'
                        WHEN model ILIKE 'command-%' THEN 'cohere'
                        WHEN model ILIKE 'groq-%' OR model ILIKE 'llama-%' THEN 'groq'
                        WHEN model ILIKE 'bedrock-%' OR model ILIKE '%.claude-%' OR model ILIKE '%.llama%' THEN 'bedrock'
                        WHEN model ILIKE '%/%' THEN 'together'
                        ELSE 'other'
                    END AS provider,
                    COALESCE(estimated_cost_usd, 0) AS spend_usd,
                    COALESCE(prompt_tokens, 0) + COALESCE(completion_tokens, 0) AS tokens
                FROM audit_logs
                WHERE project_id = $1
                  AND created_at > now() - ($2 || ' hours')::interval
                  AND model IS NOT NULL
            ) sub
            GROUP BY provider
            ORDER BY spend_usd DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Latency by provider - derives provider from model name prefix.
    /// Used by the Latency by Provider card on the Analytics dashboard.
    pub async fn get_latency_by_provider(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::ProviderLatencyStat>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::ProviderLatencyStat>(
            r#"
            SELECT
                provider,
                ROUND(AVG(latency_ms))::float8 AS latency_ms
            FROM (
                SELECT
                    CASE
                        WHEN model ILIKE 'gpt-%' OR model ILIKE 'o1-%' OR model ILIKE 'o3-%' THEN 'openai'
                        WHEN model ILIKE 'claude-%' THEN 'anthropic'
                        WHEN model ILIKE 'gemini-%' THEN 'google'
                        WHEN model ILIKE 'mistral-%' OR model ILIKE 'mixtral-%' THEN 'mistral'
                        WHEN model ILIKE 'command-%' THEN 'cohere'
                        WHEN model ILIKE 'groq-%' OR model ILIKE 'llama-%' THEN 'groq'
                        WHEN model ILIKE 'bedrock-%' OR model ILIKE '%.claude-%' OR model ILIKE '%.llama%' THEN 'bedrock'
                        WHEN model ILIKE '%/%' THEN 'together'
                        ELSE 'other'
                    END AS provider,
                    response_latency_ms AS latency_ms
                FROM audit_logs
                WHERE project_id = $1
                  AND created_at > now() - ($2 || ' hours')::interval
                  AND model IS NOT NULL
                  AND response_latency_ms IS NOT NULL
            ) sub
            GROUP BY provider
            ORDER BY latency_ms DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Traffic Analytics (Traffic Tab) ────────────────────────────────────

    /// Traffic timeseries with status breakdown by policy_result.
    /// Returns bucketed counts for: passed, throttled, blocked, hitl-paused.
    pub async fn get_traffic_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::TrafficTimeseriesPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, crate::models::analytics::TrafficTimeseriesPoint>(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                COUNT(*)::bigint as total_count,
                COUNT(*) FILTER (WHERE policy_result::text = 'Allow' OR policy_result::text LIKE 'Allow%')::bigint as passed_count,
                COUNT(*) FILTER (WHERE policy_result::text LIKE 'ShadowDeny%')::bigint as throttled_count,
                COUNT(*) FILTER (WHERE policy_result::text LIKE 'Deny%' AND policy_result::text NOT LIKE 'ShadowDeny%')::bigint as blocked_count,
                COUNT(*) FILTER (WHERE policy_result::text LIKE 'Hitl%')::bigint as hitl_paused_count
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

    /// Latency timeseries with percentile breakdown.
    /// Returns p50, p90, p99 latency for each time bucket.
    pub async fn get_latency_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::LatencyTimeseriesPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, crate::models::analytics::LatencyTimeseriesPoint>(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                COALESCE(percentile_cont(0.50) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p50,
                COALESCE(percentile_cont(0.90) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p90,
                COALESCE(percentile_cont(0.99) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p99
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

    // ── Cost Analytics (Cost Tab) ────────────────────────────────────

    /// Budget health status - counts tokens above 80% cap and without cap.
    /// Used by the Budget Health Strip on the Cost tab.
    pub async fn get_budget_health_status(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<crate::models::analytics::BudgetHealthStatus> {
        let row = sqlx::query_as::<_, crate::models::analytics::BudgetHealthStatus>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE sc.usage_usd::float8 / NULLIF(sc.limit_usd::float8, 0) > 0.8)::bigint as tokens_above_80_percent,
                COUNT(*) FILTER (WHERE sc.id IS NULL)::bigint as tokens_without_cap,
                COUNT(*)::bigint as total_tokens
            FROM tokens t
            LEFT JOIN spend_caps sc ON t.id = sc.token_id AND sc.period = 'monthly'
            WHERE t.project_id = $1 AND t.is_active = true
            "#,
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Spend timeseries grouped by dimension (provider/model/token).
    /// Used by the Spend Over Time chart on the Cost tab.
    pub async fn get_spend_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
        group_by: &str,
    ) -> anyhow::Result<Vec<crate::models::analytics::SpendTimeseriesPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        // Build dimension expression based on group_by
        let dimension_expr = match group_by {
            "provider" => r#"
                CASE
                    WHEN a.model ILIKE 'gpt-%' OR a.model ILIKE 'o1-%' OR a.model ILIKE 'o3-%' THEN 'OpenAI'
                    WHEN a.model ILIKE 'claude-%' THEN 'Anthropic'
                    WHEN a.model ILIKE 'gemini-%' THEN 'Google'
                    WHEN a.model ILIKE 'mistral-%' OR a.model ILIKE 'mixtral-%' THEN 'Mistral'
                    WHEN a.model ILIKE 'command-%' THEN 'Cohere'
                    WHEN a.model ILIKE 'groq-%' OR a.model ILIKE 'llama-%' THEN 'Groq'
                    WHEN a.model ILIKE 'bedrock-%' OR a.model ILIKE '%.claude-%' OR a.model ILIKE '%.llama%' THEN 'Bedrock'
                    WHEN a.model ILIKE '%/%' THEN 'Together'
                    ELSE 'Other'
                END"#,
            "model" => "COALESCE(a.model, 'unknown')",
            "token" => "COALESCE(a.token_id, 'unknown')",
            _ => "COALESCE(a.model, 'unknown')",
        };

        let query = format!(
            r#"
            SELECT
                date_trunc($3, a.created_at) as bucket,
                {} as dimension,
                COALESCE(SUM(a.estimated_cost_usd), 0)::float8 as spend_usd,
                COUNT(*)::bigint as request_count
            FROM audit_logs a
            WHERE a.project_id = $1 AND a.created_at > now() - ($2 || ' hours')::interval
            GROUP BY 1, 2
            ORDER BY 1 ASC, 3 DESC
            "#,
            dimension_expr
        );

        let rows = sqlx::query_as::<_, crate::models::analytics::SpendTimeseriesPoint>(&query)
            .bind(project_id)
            .bind(hours.to_string())
            .bind(bucket)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Cost efficiency trend by model over time.
    /// Returns cost per 1K tokens per model per time bucket.
    pub async fn get_cost_efficiency_trend(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::CostEfficiencyPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, crate::models::analytics::CostEfficiencyPoint>(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                model,
                CASE
                    WHEN SUM(prompt_tokens + completion_tokens) > 0
                    THEN ROUND((SUM(estimated_cost_usd) / (SUM(prompt_tokens + completion_tokens) / 1000.0))::numeric, 2)::float8
                    ELSE 0
                END as cost_per_1k_tokens
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND model IS NOT NULL
              AND estimated_cost_usd IS NOT NULL
            GROUP BY 1, 2
            HAVING SUM(prompt_tokens + completion_tokens) > 0
            ORDER BY 1 ASC, 3 DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .bind(bucket)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Budget burn rate calculation for the current month.
    /// Returns days elapsed/remaining, daily spend rate, and on-track status.
    pub async fn get_budget_burn_rate(
        &self,
        project_id: Uuid,
    ) -> anyhow::Result<crate::models::analytics::BudgetBurnRate> {
        #[derive(sqlx::FromRow)]
        struct BurnRateRow {
            days_elapsed: i32,
            days_remaining: i32,
            budget_usd: f64,
            spent_usd: f64,
            percent_used: f64,
            needed_per_day: f64,
            actual_per_day: f64,
        }

        let row = sqlx::query_as::<_, BurnRateRow>(
            r#"
            WITH month_start AS (
                SELECT date_trunc('month', now()) as start_date
            ),
            month_days AS (
                SELECT
                    EXTRACT(day FROM now() - (SELECT start_date FROM month_start))::int as days_elapsed,
                    EXTRACT(day FROM (SELECT start_date FROM month_start) + interval '1 month' - now())::int as days_remaining
            ),
            spend AS (
                SELECT COALESCE(SUM(estimated_cost_usd), 0)::float8 as spent_usd
                FROM audit_logs
                WHERE project_id = $1
                  AND created_at >= (SELECT start_date FROM month_start)
            ),
            caps AS (
                SELECT COALESCE(SUM(limit_usd), 0)::float8 as budget_usd
                FROM spend_caps
                WHERE token_id IN (SELECT id::text FROM tokens WHERE project_id = $1)
                  AND period = 'monthly'
            )
            SELECT
                md.days_elapsed,
                md.days_remaining,
                COALESCE(c.budget_usd, 2000) as budget_usd,
                s.spent_usd,
                CASE WHEN c.budget_usd > 0 THEN (s.spent_usd / c.budget_usd * 100) ELSE 0 END as percent_used,
                CASE WHEN md.days_remaining > 0 THEN (c.budget_usd - s.spent_usd) / md.days_remaining ELSE 0 END as needed_per_day,
                CASE WHEN md.days_elapsed > 0 THEN s.spent_usd / md.days_elapsed ELSE 0 END as actual_per_day
            FROM month_days md, spend s, caps c
            "#,
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;

        // On track if actual spend rate <= needed spend rate (or if no budget set)
        let on_track = row.budget_usd <= 0.0 || row.actual_per_day <= row.needed_per_day;

        Ok(crate::models::analytics::BudgetBurnRate {
            days_elapsed: row.days_elapsed,
            days_remaining: row.days_remaining,
            budget_usd: row.budget_usd,
            spent_usd: row.spent_usd,
            percent_used: row.percent_used,
            needed_per_day: row.needed_per_day,
            actual_per_day: row.actual_per_day,
            on_track,
        })
    }

    /// Token spend with cap usage percentages.
    /// Used by the Cost Breakdown Table on the Cost tab.
    pub async fn get_token_spend_with_caps(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::TokenSpendWithCap>> {
        let rows = sqlx::query_as::<_, crate::models::analytics::TokenSpendWithCap>(
            r#"
            SELECT
                t.id::text as token_id,
                t.name as token_name,
                CASE
                    WHEN a.model ILIKE 'gpt-%' OR a.model ILIKE 'o1-%' OR a.model ILIKE 'o3-%' THEN 'OpenAI'
                    WHEN a.model ILIKE 'claude-%' THEN 'Anthropic'
                    WHEN a.model ILIKE 'gemini-%' THEN 'Google'
                    WHEN a.model ILIKE 'mistral-%' OR a.model ILIKE 'mixtral-%' THEN 'Mistral'
                    WHEN a.model ILIKE 'command-%' THEN 'Cohere'
                    WHEN a.model ILIKE 'groq-%' OR a.model ILIKE 'llama-%' THEN 'Groq'
                    WHEN a.model ILIKE 'bedrock-%' OR a.model ILIKE '%.claude-%' OR a.model ILIKE '%.llama%' THEN 'Bedrock'
                    WHEN a.model ILIKE '%/%' THEN 'Together'
                    ELSE 'Other'
                END as provider,
                COALESCE(SUM(a.estimated_cost_usd), 0)::float8 as total_spend_usd,
                sc.limit_usd::float8 as spend_cap_usd,
                CASE WHEN sc.limit_usd > 0 THEN (SUM(a.estimated_cost_usd) / sc.limit_usd * 100) END::float8 as percent_cap_used,
                COUNT(*)::bigint as request_count,
                CASE
                    WHEN SUM(a.prompt_tokens + a.completion_tokens) > 0
                    THEN ROUND((SUM(a.estimated_cost_usd) / (SUM(a.prompt_tokens + a.completion_tokens) / 1000.0))::numeric, 2)::float8
                    ELSE 0
                END as cost_per_1k
            FROM tokens t
            LEFT JOIN audit_logs a ON a.token_id = t.id AND a.created_at > now() - ($2 || ' hours')::interval
            LEFT JOIN spend_caps sc ON sc.token_id = t.id AND sc.period = 'monthly'
            WHERE t.project_id = $1 AND t.is_active = true
            GROUP BY t.id, t.name, sc.limit_usd
            ORDER BY total_spend_usd DESC
            LIMIT 50
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Users & Tokens Analytics (Users & Tokens Tab) ────────────────────────────────────

    /// User growth timeseries - tracks new users and cumulative count over time.
    /// Used by the User Growth Chart on the Users & Tokens tab.
    /// Counts distinct external_user_ids per day from audit_logs.
    pub async fn get_user_growth_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<UserGrowthPoint>> {
        // Daily buckets for user growth
        let rows = sqlx::query_as::<_, UserGrowthPoint>(
            r#"
            WITH daily_users AS (
                SELECT
                    date_trunc('day', a.created_at) as bucket,
                    COUNT(DISTINCT t.external_user_id) as new_users
                FROM audit_logs a
                LEFT JOIN tokens t ON t.id = a.token_id
                WHERE a.project_id = $1
                  AND a.created_at > now() - ($2 || ' hours')::interval
                  AND t.external_user_id IS NOT NULL
                GROUP BY 1
            ),
            cumulative AS (
                SELECT
                    bucket,
                    new_users,
                    SUM(new_users) OVER (ORDER BY bucket) as cumulative_users
                FROM daily_users
            )
            SELECT bucket, new_users, cumulative_users
            FROM cumulative
            ORDER BY bucket ASC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Engagement tiers breakdown - classifies users by request volume.
    /// Power: >100 requests, Regular: 10-100, Light: 1-9.
    /// Used by the Engagement Tiers card on the Users & Tokens tab.
    pub async fn get_engagement_tiers(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<EngagementTiersResponse> {
        #[derive(sqlx::FromRow)]
        struct TierCounts {
            power_users: i64,
            regular_users: i64,
            light_users: i64,
            total_users: i64,
        }

        let row = sqlx::query_as::<_, TierCounts>(
            r#"
            WITH user_requests AS (
                SELECT
                    t.external_user_id,
                    COUNT(*) as request_count
                FROM audit_logs a
                LEFT JOIN tokens t ON t.id = a.token_id
                WHERE a.project_id = $1
                  AND a.created_at > now() - ($2 || ' hours')::interval
                  AND t.external_user_id IS NOT NULL
                GROUP BY t.external_user_id
            )
            SELECT
                COUNT(*) FILTER (WHERE request_count > 100)::bigint as power_users,
                COUNT(*) FILTER (WHERE request_count >= 10 AND request_count <= 100)::bigint as regular_users,
                COUNT(*) FILTER (WHERE request_count >= 1 AND request_count < 10)::bigint as light_users,
                COUNT(*)::bigint as total_users
            FROM user_requests
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(EngagementTiersResponse {
            power_users: row.power_users,
            regular_users: row.regular_users,
            light_users: row.light_users,
            total_users: row.total_users,
        })
    }

    /// Token alerts - active tokens and those hitting rate limits.
    /// Used by the Active Tokens Card and Rate Limit Alert on the Users & Tokens tab.
    pub async fn get_token_alerts(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<TokenAlertsResponse> {
        #[derive(sqlx::FromRow)]
        struct ActiveCountRow {
            count: i64,
        }

        // Get active tokens (have made requests in the time window)
        let active_row = sqlx::query_as::<_, ActiveCountRow>(
            r#"
            SELECT COUNT(DISTINCT token_id)::bigint as count
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        let active_tokens = active_row.count;

        // Get rate-limited tokens (policy_result contains 'throttled' or rate limit errors)
        let rate_limited_rows = sqlx::query_as::<_, RateLimitedToken>(
            r#"
            SELECT
                t.name as token_name,
                CASE
                    WHEN COUNT(*) > 0 THEN 100.0
                    ELSE 0.0
                END as percent
            FROM audit_logs a
            LEFT JOIN tokens t ON t.id = a.token_id
            WHERE a.project_id = $1
              AND a.created_at > now() - ($2 || ' hours')::interval
              AND (a.policy_result::text ILIKE '%throttled%' OR a.policy_result::text ILIKE '%rate%limit%')
            GROUP BY t.name
            ORDER BY percent DESC
            LIMIT 10
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;

        let tokens_at_rate_limit = rate_limited_rows.len() as i64;

        Ok(TokenAlertsResponse {
            active_tokens,
            token_limit: None, // Could be configured per-project in the future
            tokens_at_rate_limit,
            rate_limited_tokens: rate_limited_rows,
        })
    }

    /// Requests per user timeseries - shows request volume per user over time.
    /// Used by the Requests Per User chart on the Users & Tokens tab.
    pub async fn get_requests_per_user_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<RequestsPerUserPoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, RequestsPerUserPoint>(
            r#"
            SELECT
                date_trunc($3, a.created_at) as bucket,
                COUNT(DISTINCT t.external_user_id)::bigint as user_count,
                COUNT(*)::bigint as request_count,
                CASE
                    WHEN COUNT(DISTINCT t.external_user_id) > 0
                    THEN COUNT(*)::float8 / COUNT(DISTINCT t.external_user_id)::float8
                    ELSE 0
                END as avg_per_user
            FROM audit_logs a
            LEFT JOIN tokens t ON t.id = a.token_id
            WHERE a.project_id = $1
              AND a.created_at > now() - ($2 || ' hours')::interval
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

    // ── Cache Analytics (Cache Tab) ────────────────────────────────────

    /// Cache summary statistics for the Cache tab ribbon.
    pub async fn get_cache_summary_stats(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<CacheSummaryStats> {
        #[derive(sqlx::FromRow)]
        struct CacheStatsRow {
            total: i64,
            hits: i64,
            cost_avoided: f64,
        }

        let row = sqlx::query_as::<_, CacheStatsRow>(
            r#"
            SELECT
                COUNT(*)::bigint as total,
                COUNT(*) FILTER (WHERE cache_hit = true)::bigint as hits,
                COALESCE(SUM(estimated_cost_usd) FILTER (WHERE cache_hit = true), 0)::float8 as cost_avoided
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        let hit_rate = if row.total > 0 {
            (row.hits as f64 / row.total as f64) * 100.0
        } else {
            0.0
        };

        // Get top cached model
        #[derive(sqlx::FromRow)]
        struct TopModelRow {
            model: Option<String>,
        }
        let top_model_row = sqlx::query_as::<_, TopModelRow>(
            r#"
            SELECT model
            FROM audit_logs
            WHERE project_id = $1 AND cache_hit = true AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY model
            ORDER BY COUNT(*) DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(CacheSummaryStats {
            hit_rate,
            cost_avoided_usd: row.cost_avoided,
            cache_size_bytes: 0, // Would need Redis INFO command
            top_cached_model: top_model_row.and_then(|r| r.model),
        })
    }

    /// Cache hit rate timeseries for the Cache tab chart.
    /// Returns daily buckets with hit rate percentage and counts.
    pub async fn get_cache_hit_rate_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<CacheHitRatePoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, CacheHitRatePoint>(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                CASE
                    WHEN COUNT(*) > 0 THEN (COUNT(*) FILTER (WHERE cache_hit = true)::float8 / COUNT(*)::float8 * 100)
                    ELSE 0
                END as hit_rate,
                COUNT(*) FILTER (WHERE cache_hit = true)::bigint as hit_count,
                COUNT(*)::bigint as total_count
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

    /// Top cached queries for the Cache tab table.
    /// Returns the most frequently hit cache entries.
    /// Note: This uses a materialized view pattern on audit_logs since we don't have
    /// direct access to Redis cache keys. We derive from cache_hit patterns.
    pub async fn get_top_cached_queries(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<CachedQueryRow>> {
        // We derive cached queries from audit logs where cache_hit = true
        // Grouping by model and approximate request signature
        let rows = sqlx::query_as::<_, CachedQueryRow>(
            r#"
            SELECT
                'hash_' || LEFT(MD5(COALESCE(model, 'unknown') || COALESCE(token_id, '')), 8) || '…' as query_hash,
                model,
                COUNT(*) FILTER (WHERE cache_hit = true)::bigint as hits,
                MAX(created_at) FILTER (WHERE cache_hit = true) as last_hit_at,
                EXTRACT(EPOCH FROM (now() - MIN(created_at) FILTER (WHERE cache_hit = true)))::bigint as cache_age_seconds,
                NULL::bigint as expires_in_seconds
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - interval '24 hours'
            GROUP BY model, token_id
            HAVING COUNT(*) FILTER (WHERE cache_hit = true) > 0
            ORDER BY hits DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Model-level cache efficiency for the Cache tab.
    /// Returns hit rate per model.
    pub async fn get_model_cache_efficiency(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ModelCacheEfficiency>> {
        let rows = sqlx::query_as::<_, ModelCacheEfficiency>(
            r#"
            SELECT
                COALESCE(model, 'unknown') as model,
                CASE
                    WHEN COUNT(*) > 0 THEN (COUNT(*) FILTER (WHERE cache_hit = true)::float8 / COUNT(*)::float8 * 100)
                    ELSE 0
                END as hit_rate,
                COUNT(*)::bigint as total_requests,
                COUNT(*) FILTER (WHERE cache_hit = true)::bigint as cache_hits
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY model
            HAVING COUNT(*) > 10
            ORDER BY hit_rate DESC
            LIMIT 10
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Cache latency comparison for the Cache tab.
    /// Returns average latency for cached vs uncached responses.
    pub async fn get_cache_latency_comparison(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<CacheLatencyComparison> {
        #[derive(sqlx::FromRow)]
        struct LatencyRow {
            cached_latency_ms: Option<f64>,
            uncached_latency_ms: Option<f64>,
            cached_count: i64,
            uncached_count: i64,
        }

        let row = sqlx::query_as::<_, LatencyRow>(
            r#"
            SELECT
                AVG(response_latency_ms) FILTER (WHERE cache_hit = true) as cached_latency_ms,
                AVG(response_latency_ms) FILTER (WHERE cache_hit = false OR cache_hit IS NULL) as uncached_latency_ms,
                COUNT(*) FILTER (WHERE cache_hit = true)::bigint as cached_count,
                COUNT(*) FILTER (WHERE cache_hit = false OR cache_hit IS NULL)::bigint as uncached_count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        let cached_latency = row.cached_latency_ms.unwrap_or(0.0);
        let uncached_latency = row.uncached_latency_ms.unwrap_or(0.0);
        let speedup = if cached_latency > 0.0 {
            uncached_latency / cached_latency
        } else {
            0.0
        };

        Ok(CacheLatencyComparison {
            cached_latency_ms: cached_latency,
            uncached_latency_ms: uncached_latency,
            speedup_factor: speedup,
            cached_sample_count: row.cached_count,
            uncached_sample_count: row.uncached_count,
        })
    }

    // ── Model Analytics (Models Tab) ────────────────────────────────────

    /// Model usage timeseries for the Models tab.
    /// Returns usage over time grouped by model with configurable metric (requests/cost/cache_hits).
    pub async fn get_model_usage_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
        group_by: &str,
    ) -> anyhow::Result<Vec<ModelUsageTimeseriesPoint>> {
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let value_expr = match group_by {
            "cost" => "COALESCE(SUM(estimated_cost_usd), 0)::float8",
            "cache_hits" => "COUNT(*) FILTER (WHERE cache_hit = true)::float8",
            _ => "COUNT(*)::float8", // default: requests
        };

        let query = format!(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                COALESCE(model, 'unknown') as model,
                {} as value
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY 1, 2
            ORDER BY 1 ASC, 3 DESC
            "#,
            value_expr
        );

        let rows = sqlx::query_as::<_, ModelUsageTimeseriesPoint>(&query)
            .bind(project_id)
            .bind(hours.to_string())
            .bind(bucket)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Error rate by model for the Models tab.
    /// Returns error rate percentage per model.
    pub async fn get_model_error_rates(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ModelErrorRate>> {
        let rows = sqlx::query_as::<_, ModelErrorRate>(
            r#"
            SELECT
                COALESCE(model, 'unknown') as model,
                CASE
                    WHEN COUNT(*) > 0 THEN (COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL)::float8 / COUNT(*)::float8 * 100)
                    ELSE 0
                END as error_rate,
                COUNT(*)::bigint as total_requests,
                COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL)::bigint as error_count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY model
            HAVING COUNT(*) >= 5
            ORDER BY error_rate DESC
            LIMIT 15
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Latency by model for the Models tab.
    /// Returns average and percentile latency per model.
    pub async fn get_model_latency(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ModelLatencyStat>> {
        let rows = sqlx::query_as::<_, ModelLatencyStat>(
            r#"
            SELECT
                COALESCE(model, 'unknown') as model,
                COALESCE(AVG(response_latency_ms), 0)::float8 as avg_latency_ms,
                COALESCE(percentile_cont(0.50) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p50,
                COALESCE(percentile_cont(0.90) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p90,
                COALESCE(percentile_cont(0.99) WITHIN GROUP (ORDER BY response_latency_ms), 0)::float8 as p99,
                COUNT(*)::bigint as sample_count
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND response_latency_ms IS NOT NULL
            GROUP BY model
            HAVING COUNT(*) >= 5
            ORDER BY avg_latency_ms DESC
            LIMIT 15
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Combined model stats for the Models tab table.
    /// Returns model name, tokens, cost, error rate, and latency.
    pub async fn get_model_stats(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ModelStatsRow>> {
        let rows = sqlx::query_as::<_, ModelStatsRow>(
            r#"
            SELECT
                COALESCE(model, 'unknown') as model,
                COALESCE(SUM(prompt_tokens + completion_tokens), 0)::bigint as total_tokens,
                COALESCE(SUM(estimated_cost_usd), 0)::float8 as total_cost_usd,
                CASE
                    WHEN COUNT(*) > 0 THEN (COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL)::float8 / COUNT(*)::float8 * 100)
                    ELSE 0
                END as error_rate,
                COALESCE(AVG(response_latency_ms), 0)::float8 as avg_latency_ms,
                COUNT(*)::bigint as request_count
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            GROUP BY model
            ORDER BY total_cost_usd DESC
            LIMIT 20
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Cost vs latency scatter data for the Models tab bubble chart.
    /// Returns per-model averages with total spend for dot sizing.
    pub async fn get_cost_latency_scatter(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<CostLatencyScatterPoint>> {
        let rows = sqlx::query_as::<_, CostLatencyScatterPoint>(
            r#"
            SELECT
                COALESCE(model, 'unknown') as model,
                CASE
                    WHEN COUNT(*) > 0 THEN COALESCE(SUM(estimated_cost_usd), 0)::float8 / COUNT(*)::float8
                    ELSE 0
                END as avg_cost_per_request,
                COALESCE(AVG(response_latency_ms), 0)::float8 as avg_latency_ms,
                COALESCE(SUM(estimated_cost_usd), 0)::float8 as total_spend_usd,
                COUNT(*)::bigint as total_requests
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND model IS NOT NULL
            GROUP BY model
            HAVING COUNT(*) >= 5
            ORDER BY total_spend_usd DESC
            LIMIT 20
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Security Analytics (Security Tab) ────────────────────────────────────

    /// Security summary KPIs for the Security tab ribbon.
    /// Returns counts for PII redactions, guardrail blocks, shadow violations, external blocks.
    pub async fn get_security_summary(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<SecuritySummaryStats> {
        let row = sqlx::query_as::<_, SecuritySummaryStats>(
            r#"
            SELECT
                COUNT(*) FILTER (
                    WHERE fields_redacted IS NOT NULL
                    AND jsonb_array_length(fields_redacted) > 0
                )::bigint as pii_redactions,
                COUNT(*) FILTER (
                    WHERE policy_result::text LIKE 'Deny%'
                )::bigint as guardrail_blocks,
                COUNT(*) FILTER (
                    WHERE shadow_violations IS NOT NULL
                    AND jsonb_array_length(shadow_violations) > 0
                )::bigint as shadow_violations,
                COUNT(*) FILTER (
                    WHERE policy_result::text LIKE 'Deny%'
                    AND deny_reason ILIKE '%external%'
                )::bigint as external_blocks
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Guardrail triggers grouped by category for the Security tab.
    /// Parses deny_reason for guardrail category keywords.
    pub async fn get_guardrail_triggers(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<GuardrailTriggerStat>> {
        let rows = sqlx::query_as::<_, GuardrailTriggerStat>(
            r#"
            SELECT category, COUNT(*)::bigint as count
            FROM (
                SELECT
                    CASE
                        WHEN deny_reason ILIKE '%jailbreak%' THEN 'Jailbreak'
                        WHEN deny_reason ILIKE '%harmful%' OR deny_reason ILIKE '%violence%' THEN 'Harmful content'
                        WHEN deny_reason ILIKE '%injection%' OR deny_reason ILIKE '%code injection%' THEN 'Code injection'
                        WHEN deny_reason ILIKE '%profanity%' THEN 'Profanity'
                        WHEN deny_reason ILIKE '%bias%' THEN 'Bias'
                        WHEN deny_reason ILIKE '%sensitive%' THEN 'Sensitive topics'
                        WHEN deny_reason ILIKE '%competitor%' THEN 'Competitor mentions'
                        ELSE 'Other'
                    END as category
                FROM audit_logs
                WHERE project_id = $1
                  AND created_at > now() - ($2 || ' hours')::interval
                  AND policy_result::text LIKE 'Deny%'
                  AND deny_reason IS NOT NULL
            ) sub
            WHERE category != 'Other'
            GROUP BY category
            ORDER BY count DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// PII breakdown by pattern type for the Security tab.
    /// Unnests fields_redacted JSONB array and groups by pattern.
    pub async fn get_pii_breakdown(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<PiiBreakdownStat>> {
        let rows = sqlx::query_as::<_, PiiBreakdownStat>(
            r#"
            SELECT
                CASE pattern
                    WHEN 'email' THEN 'Email'
                    WHEN 'api_key' THEN 'API key'
                    WHEN 'cc' THEN 'Credit card'
                    WHEN 'ssn' THEN 'SSN'
                    WHEN 'phone' THEN 'Phone'
                    WHEN 'nlp' THEN 'NLP-detected'
                    ELSE INITCAP(pattern)
                END as pattern,
                COUNT(*)::bigint as count
            FROM audit_logs,
                 jsonb_array_elements_text(
                     COALESCE(fields_redacted, '[]'::jsonb)
                 ) as pattern
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND fields_redacted IS NOT NULL
              AND jsonb_array_length(fields_redacted) > 0
            GROUP BY pattern
            ORDER BY count DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Policy actions count for the Security tab.
    /// Parses policies_evaluated JSONB for action types.
    pub async fn get_policy_actions(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<PolicyActionStat>> {
        let rows = sqlx::query_as::<_, PolicyActionStat>(
            r#"
            SELECT action, COUNT(*)::bigint as count
            FROM (
                SELECT
                    jsonb_array_elements(
                        COALESCE(policies_evaluated, '[]'::jsonb)
                    ) ->> 'action' as action
                FROM audit_logs
                WHERE project_id = $1
                  AND created_at > now() - ($2 || ' hours')::interval
                  AND policies_evaluated IS NOT NULL
                  AND jsonb_array_length(policies_evaluated) > 0
            ) sub
            WHERE action IS NOT NULL
            GROUP BY action
            ORDER BY count DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Shadow mode policies with violation stats for the Security tab.
    /// Groups by policy name from shadow_violations and joins with tokens for top violator.
    pub async fn get_shadow_policies(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ShadowPolicyStat>> {
        let rows = sqlx::query_as::<_, ShadowPolicyStat>(
            r#"
            WITH shadow_violation_counts AS (
                SELECT
                    sv.policy_name,
                    COUNT(*) as violations,
                    a.token_id,
                    ROW_NUMBER() OVER (
                        PARTITION BY sv.policy_name
                        ORDER BY COUNT(*) DESC
                    ) as rn
                FROM audit_logs a,
                     jsonb_to_recordset(
                         COALESCE(a.shadow_violations, '[]'::jsonb)
                     ) as sv(policy_name text)
                WHERE a.project_id = $1
                  AND a.created_at > now() - ($2 || ' hours')::interval
                  AND a.shadow_violations IS NOT NULL
                  AND jsonb_array_length(a.shadow_violations) > 0
                GROUP BY sv.policy_name, a.token_id
            ),
            top_tokens AS (
                SELECT policy_name, token_id, violations
                FROM shadow_violation_counts
                WHERE rn = 1
            )
            SELECT
                svc.policy_name as policy_name,
                SUM(svc.violations)::bigint as violations,
                COALESCE(t.name, tt.token_id, 'Unknown') as top_token,
                'Monitoring' as status
            FROM shadow_violation_counts svc
            LEFT JOIN top_tokens tt ON svc.policy_name = tt.policy_name
            LEFT JOIN tokens t ON t.id = tt.token_id
            GROUP BY svc.policy_name, t.name, tt.token_id
            ORDER BY violations DESC
            LIMIT 20
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Data residency stats for the Security tab.
    /// Returns EU/US routing percentages (mock data until upstream region tracking is implemented).
    pub async fn get_data_residency(
        &self,
        _project_id: Uuid,
        _hours: i32,
    ) -> anyhow::Result<DataResidencyStats> {
        // Note: Upstream region is not currently tracked in audit_logs.
        // Return mock data (98% EU, 2% US) until this is implemented.
        // TODO: Add upstream_region column to audit_logs for real tracking.
        Ok(DataResidencyStats {
            eu_percent: 98.0,
            us_percent: 2.0,
        })
    }

    // ── HITL Analytics (HITL Tab) ────────────────────────────────────

    /// HITL KPI summary for the HITL tab ribbon.
    /// Returns pending count, average wait time, and approval rate.
    pub async fn get_hitl_summary(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<crate::models::analytics::HitlSummaryStats> {
        #[derive(sqlx::FromRow)]
        struct HitlSummaryRow {
            pending_count: i64,
            avg_wait_seconds: Option<f64>,
            approved_count: i64,
            rejected_count: i64,
        }

        let row = sqlx::query_as::<_, HitlSummaryRow>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'pending')::bigint as pending_count,
                AVG(EXTRACT(EPOCH FROM (reviewed_at - created_at))) FILTER (
                    WHERE status IN ('approved', 'rejected')
                ) as avg_wait_seconds,
                COUNT(*) FILTER (WHERE status = 'approved')::bigint as approved_count,
                COUNT(*) FILTER (WHERE status = 'rejected')::bigint as rejected_count
            FROM approval_requests
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        let total_decided = row.approved_count + row.rejected_count;
        let approval_rate = if total_decided > 0 {
            (row.approved_count as f64 / total_decided as f64) * 100.0
        } else {
            0.0
        };

        Ok(crate::models::analytics::HitlSummaryStats {
            pending_count: row.pending_count,
            avg_wait_seconds: row.avg_wait_seconds.unwrap_or(0.0),
            approval_rate,
        })
    }

    /// HITL volume timeseries for the HITL tab chart.
    /// Returns daily buckets with counts by status.
    pub async fn get_hitl_volume_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::HitlVolumePoint>> {
        // Dynamic bucket size based on range
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, crate::models::analytics::HitlVolumePoint>(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                COUNT(*) FILTER (WHERE status = 'approved')::bigint as approved_count,
                COUNT(*) FILTER (WHERE status = 'rejected')::bigint as rejected_count,
                COUNT(*) FILTER (WHERE status = 'expired')::bigint as expired_count,
                COUNT(*) FILTER (WHERE status = 'pending')::bigint as pending_count
            FROM approval_requests
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

    /// HITL latency stats for the SLA card.
    /// Returns p50, p90, p99, and avg approval times.
    pub async fn get_hitl_latency_stats(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<crate::models::analytics::HitlLatencyStats> {
        #[derive(sqlx::FromRow)]
        struct LatencyRow {
            p50: Option<f64>,
            p90: Option<f64>,
            p99: Option<f64>,
            avg: Option<f64>,
        }

        let row = sqlx::query_as::<_, LatencyRow>(
            r#"
            SELECT
                COALESCE(percentile_cont(0.50) WITHIN GROUP (
                    ORDER BY EXTRACT(EPOCH FROM (reviewed_at - created_at))
                ), 0)::float8 as p50,
                COALESCE(percentile_cont(0.90) WITHIN GROUP (
                    ORDER BY EXTRACT(EPOCH FROM (reviewed_at - created_at))
                ), 0)::float8 as p90,
                COALESCE(percentile_cont(0.99) WITHIN GROUP (
                    ORDER BY EXTRACT(EPOCH FROM (reviewed_at - created_at))
                ), 0)::float8 as p99,
                COALESCE(AVG(EXTRACT(EPOCH FROM (reviewed_at - created_at))), 0)::float8 as avg
            FROM approval_requests
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND status IN ('approved', 'rejected')
              AND reviewed_at IS NOT NULL
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(crate::models::analytics::HitlLatencyStats {
            p50_seconds: row.p50.unwrap_or(0.0),
            p90_seconds: row.p90.unwrap_or(0.0),
            p99_seconds: row.p99.unwrap_or(0.0),
            avg_seconds: row.avg.unwrap_or(0.0),
        })
    }

    /// Rejection reasons breakdown for the HITL tab.
    /// Note: No rejection_reason column exists yet. Returns mock data.
    /// TODO: Add migration for rejection_reason column to approval_requests.
    pub async fn get_hitl_rejection_reasons(
        &self,
        _project_id: Uuid,
        _hours: i32,
    ) -> anyhow::Result<Vec<crate::models::analytics::RejectionReason>> {
        // Note: rejection_reason column does not exist in approval_requests table.
        // Returning mock data until migration is added.
        Ok(vec![
            crate::models::analytics::RejectionReason {
                reason: "Hallucination".to_string(),
                percentage: 40.0,
            },
            crate::models::analytics::RejectionReason {
                reason: "Tone".to_string(),
                percentage: 34.0,
            },
            crate::models::analytics::RejectionReason {
                reason: "PII".to_string(),
                percentage: 26.0,
            },
        ])
    }

    // ── Error Analytics (Errors Tab) ────────────────────────────────────

    /// Error KPI summary for the Errors tab ribbon.
    /// Returns total errors, error rate, circuit breaker trips, rate limit hits, and top error type.
    pub async fn get_error_summary(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<crate::models::analytics::ErrorSummaryStats> {
        // Current period stats
        #[derive(sqlx::FromRow)]
        struct CurrentStats {
            total_errors: i64,
            total_requests: i64,
            circuit_breaker_trips: i64,
            rate_limit_hits: i64,
            top_error_type: Option<String>,
        }

        let current = sqlx::query_as::<_, CurrentStats>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL)::bigint as total_errors,
                COUNT(*)::bigint as total_requests,
                COUNT(*) FILTER (WHERE deny_reason = 'circuit_breaker_open')::bigint as circuit_breaker_trips,
                COUNT(*) FILTER (WHERE error_type = 'rate_limit')::bigint as rate_limit_hits,
                (SELECT error_type FROM audit_logs
                 WHERE project_id = $1
                   AND created_at > now() - ($2 || ' hours')::interval
                   AND error_type IS NOT NULL
                 GROUP BY error_type
                 ORDER BY COUNT(*) DESC
                 LIMIT 1) as top_error_type
            FROM audit_logs
            WHERE project_id = $1 AND created_at > now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        // Prior period stats (same duration before current period)
        #[derive(sqlx::FromRow)]
        struct PriorStats {
            prior_total_errors: i64,
            prior_total_requests: i64,
        }

        let prior = sqlx::query_as::<_, PriorStats>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE upstream_status >= 400 OR upstream_status IS NULL)::bigint as prior_total_errors,
                COUNT(*)::bigint as prior_total_requests
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval - ($2 || ' hours')::interval
              AND created_at <= now() - ($2 || ' hours')::interval
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_one(&self.pool)
        .await?;

        let error_rate = if current.total_requests > 0 {
            (current.total_errors as f64 / current.total_requests as f64) * 100.0
        } else {
            0.0
        };

        Ok(crate::models::analytics::ErrorSummaryStats {
            total_errors: current.total_errors,
            error_rate,
            circuit_breaker_trips: current.circuit_breaker_trips,
            rate_limit_hits: current.rate_limit_hits,
            top_error_type: current.top_error_type,
            prior_total_errors: prior.prior_total_errors,
            prior_total_requests: prior.prior_total_requests,
        })
    }

    /// Error timeseries for the Errors tab chart.
    /// Returns 4 error categories over time buckets.
    pub async fn get_error_timeseries(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ErrorTimeseriesPoint>> {
        let bucket = if hours <= 24 { "hour" } else { "day" };

        let rows = sqlx::query_as::<_, ErrorTimeseriesPoint>(
            r#"
            SELECT
                date_trunc($3, created_at) as bucket,
                COUNT(*) FILTER (WHERE error_type = 'timeout')::bigint as timeout_count,
                COUNT(*) FILTER (WHERE error_type = 'rate_limit')::bigint as rate_limit_count,
                COUNT(*) FILTER (WHERE upstream_status >= 500)::bigint as upstream_5xx_count,
                COUNT(*) FILTER (WHERE deny_reason = 'circuit_breaker_open')::bigint as circuit_breaker_count
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

    /// Error type breakdown for the Errors tab bar chart.
    /// Returns error types with count and percentage.
    pub async fn get_error_type_breakdown(
        &self,
        project_id: Uuid,
        hours: i32,
    ) -> anyhow::Result<Vec<ErrorTypeBreakdown>> {
        #[derive(sqlx::FromRow)]
        struct RawBreakdown {
            error_type: String,
            count: i64,
        }

        let rows = sqlx::query_as::<_, RawBreakdown>(
            r#"
            SELECT
                error_type,
                COUNT(*)::bigint as count
            FROM audit_logs
            WHERE project_id = $1
              AND created_at > now() - ($2 || ' hours')::interval
              AND error_type IS NOT NULL
            GROUP BY error_type
            ORDER BY count DESC
            "#,
        )
        .bind(project_id)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;

        let total: i64 = rows.iter().map(|r| r.count).sum();

        let breakdown: Vec<ErrorTypeBreakdown> = rows
            .into_iter()
            .map(|r| ErrorTypeBreakdown {
                error_type: r.error_type,
                count: r.count,
                percentage: if total > 0 {
                    (r.count as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();

        Ok(breakdown)
    }

    /// Error logs for the Errors tab table.
    /// Returns recent error entries with token name via JOIN.
    pub async fn get_error_logs(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<ErrorLogRow>> {
        let rows = sqlx::query_as::<_, ErrorLogRow>(
            r#"
            SELECT
                a.request_id,
                a.token_id,
                t.name as token_name,
                a.model,
                a.error_type,
                a.upstream_status,
                a.response_latency_ms,
                a.deny_reason,
                a.created_at
            FROM audit_logs a
            LEFT JOIN tokens t ON t.id = a.token_id
            WHERE a.project_id = $1
              AND (a.upstream_status >= 400 OR a.error_type IS NOT NULL)
            ORDER BY a.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
