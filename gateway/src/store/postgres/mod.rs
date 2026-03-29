mod analytics;
mod api_keys;
mod approvals;
mod audit;
mod credentials;
pub mod mcp;
mod notifications;
mod oidc;
mod policies;
mod pricing;
mod projects;
mod prompts;
mod secret_references;
mod services;
mod sessions;
mod settings;
mod tokens;
pub mod types;
mod usage;
mod users;
pub mod vault_config;

// SEC-10: Export LastAdminError for use in API handlers
pub use api_keys::LastAdminError;

// Export AuditFilter for audit log filtering
pub use audit::AuditFilter;

#[cfg(test)]
mod tests;

pub use self::types::*;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

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
