pub mod types;
mod projects;
mod credentials;
mod tokens;
mod policies;
mod approvals;
mod audit;
mod analytics;
mod notifications;
mod services;
mod sessions;
mod oidc;
mod api_keys;
mod usage;
mod pricing;
mod settings;
mod prompts;

#[cfg(test)]
mod tests;

pub use self::types::*;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let max_conns: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);
        let pool = PgPoolOptions::new()
            .max_connections(max_conns)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run pending migrations from the migrations/ directory.
    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }
}
