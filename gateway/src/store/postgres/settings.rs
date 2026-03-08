use super::PgStore;

impl PgStore {
    // -- System Settings Operations --

    pub async fn get_system_setting<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<T>> {
        let row = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT value FROM system_settings WHERE key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(val) = row {
            let parsed: T = serde_json::from_value(val)?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    pub async fn set_system_setting<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        let json_val = serde_json::to_value(value)?;
        
        sqlx::query(
            r#"
            INSERT INTO system_settings (key, value, description)
            VALUES ($1, $2, $3)
            ON CONFLICT (key) DO UPDATE
            SET value = EXCLUDED.value,
                description = COALESCE(EXCLUDED.description, system_settings.description),
                updated_at = NOW()
            "#
        )
        .bind(key)
        .bind(json_val)
        .bind(description)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn get_all_system_settings(&self) -> anyhow::Result<std::collections::HashMap<String, serde_json::Value>> {
        let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
            "SELECT key, value FROM system_settings"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut settings = std::collections::HashMap::new();
        for (k, v) in rows {
            settings.insert(k, v);
        }
        Ok(settings)
    }
}
