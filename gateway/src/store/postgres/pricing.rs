use super::types::ModelPricingRow;
use super::PgStore;
use uuid::Uuid;

impl PgStore {
    // -- Model Pricing Operations --

    pub async fn list_model_pricing(&self) -> anyhow::Result<Vec<ModelPricingRow>> {
        let rows = sqlx::query_as::<_, ModelPricingRow>(
            r#"SELECT id, provider, model_pattern, input_per_m, output_per_m, is_active, created_at, updated_at
               FROM model_pricing
               WHERE is_active = true
               ORDER BY provider ASC, model_pattern ASC"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert a pricing entry. Returns the row ID.
    pub async fn upsert_model_pricing(
        &self,
        provider: &str,
        model_pattern: &str,
        input_per_m: rust_decimal::Decimal,
        output_per_m: rust_decimal::Decimal,
    ) -> anyhow::Result<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO model_pricing (provider, model_pattern, input_per_m, output_per_m)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (provider, model_pattern) DO UPDATE
                 SET input_per_m = EXCLUDED.input_per_m,
                     output_per_m = EXCLUDED.output_per_m,
                     is_active = true,
                     updated_at = NOW()
               RETURNING id"#,
        )
        .bind(provider)
        .bind(model_pattern)
        .bind(input_per_m)
        .bind(output_per_m)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Soft-delete a pricing entry (sets is_active = false).
    pub async fn delete_model_pricing(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE model_pricing SET is_active = false, updated_at = NOW() WHERE id = $1 AND is_active = true"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
