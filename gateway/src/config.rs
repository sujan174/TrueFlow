use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub master_key: String,
    pub admin_key: Option<String>,
    pub slack_webhook_url: Option<String>,
    /// Comma-separated list of webhook URLs to notify on policy events.
    pub webhook_urls: Vec<String>,
    /// Default per-token rate limit (requests per window). 0 = disabled.
    /// Set via TRUEFLOW_DEFAULT_RPM env var. Default: 600.
    pub default_rate_limit: u64,
    /// Window in seconds for the default rate limit.
    /// Set via TRUEFLOW_DEFAULT_RPM_WINDOW env var. Default: 60.
    pub default_rate_limit_window: u64,
    /// Comma-separated list of trusted proxy CIDRs for X-Forwarded-For validation.
    /// If empty (default), X-Forwarded-For headers are ignored for security.
    /// Example: "10.0.0.0/8,172.16.0.0/12,192.168.0.0/16"
    pub trusted_proxy_cidrs: Vec<String>,
}

impl Config {
    /// Returns the admin key for API authentication.
    /// Falls back to master_key if TRUEFLOW_ADMIN_KEY is not set.
    pub fn admin_key(&self) -> &str {
        self.admin_key.as_deref().unwrap_or(&self.master_key)
    }
}

pub fn load() -> anyhow::Result<Config> {
    dotenvy::dotenv().ok();

    let master_key =
        std::env::var("TRUEFLOW_MASTER_KEY").unwrap_or_else(|_| "CHANGE_ME_32_BYTE_HEX_KEY".into());

    if master_key == "CHANGE_ME_32_BYTE_HEX_KEY" {
        let env_mode = std::env::var("TRUEFLOW_ENV")
            .or_else(|_| std::env::var("RUST_ENV"))
            .unwrap_or_default();
        if env_mode == "production" {
            anyhow::bail!(
                "TRUEFLOW_MASTER_KEY is still the insecure placeholder. \
                 Set a proper 64-char hex key before running in production."
            );
        }
        eprintln!("⚠️  TRUEFLOW_MASTER_KEY is not set — using insecure placeholder. Set a 64-char hex key for production.");
    }

    Ok(Config {
        port: std::env::var("TRUEFLOW_PORT")
            .unwrap_or_else(|_| "8443".into())
            .parse()
            .unwrap_or(8443),
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/trueflow".into()),
        redis_url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
        master_key,
        admin_key: std::env::var("TRUEFLOW_ADMIN_KEY").ok(),
        slack_webhook_url: std::env::var("TRUEFLOW_SLACK_WEBHOOK_URL").ok(),
        webhook_urls: std::env::var("TRUEFLOW_WEBHOOK_URLS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        default_rate_limit: std::env::var("TRUEFLOW_DEFAULT_RPM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600),
        default_rate_limit_window: std::env::var("TRUEFLOW_DEFAULT_RPM_WINDOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60),
        trusted_proxy_cidrs: std::env::var("TRUSTED_PROXY_CIDRS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
    })
}
