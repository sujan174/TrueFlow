//! HTTP client for the TrueFlow management API.
//!
//! Used by the IaC CLI to fetch live state and apply changes.
//! Talks exclusively to the REST API — never touches the database directly.

use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;

use super::schema::{ConfigDoc, PolicySpec, TokenSpec};

/// Client for the TrueFlow management API.
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    project_id: Option<String>,
}

/// Response from GET /api/v1/tokens (list)
#[derive(Deserialize)]
struct TokenListItem {
    id: String,
    name: String,
}

/// Response from GET /api/v1/tokens/:id/spend
#[derive(Deserialize)]
struct SpendStatus {
    daily_limit_usd: Option<f64>,
    monthly_limit_usd: Option<f64>,
    lifetime_limit_usd: Option<f64>,
}

/// Response from POST /api/v1/config/import
#[derive(Debug, Deserialize)]
pub struct ImportResult {
    pub policies_created: usize,
    pub policies_updated: usize,
    pub tokens_created: usize,
    pub tokens_updated: usize,
}

impl ApiClient {
    /// Create a new API client.
    ///
    /// `api_key` is the admin key or API key (ak_live_...) sent as
    /// `Authorization: Bearer <key>`.
    pub fn new(gateway_url: &str, api_key: &str, project_id: Option<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let auth_val = format!("Bearer {}", api_key);
        headers.insert(
            "authorization",
            HeaderValue::from_str(&auth_val).context("invalid api key characters")?,
        );
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .danger_accept_invalid_certs(true) // local dev often uses self-signed
            .build()?;
        Ok(Self {
            http,
            base_url: gateway_url.trim_end_matches('/').to_string(),
            project_id,
        })
    }

    /// Fetch the live config from the gateway and return a ConfigDoc v2.
    pub async fn export(&self) -> Result<ConfigDoc> {
        // 1. Export policies + tokens via the config export endpoint (v1 format)
        let mut url = format!("{}/api/v1/config/export?format=json", self.base_url);
        if let Some(ref pid) = self.project_id {
            url.push_str(&format!("&project_id={}", pid));
        }
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("export failed ({}): {}", status, body);
        }
        let v1: V1ConfigDoc = resp.json().await?;

        // 2. Fetch token list to get IDs (we need IDs to query spend caps)
        let tokens_list = self.list_tokens().await?;

        // 3. Build v2 tokens with spend caps
        let mut tokens = Vec::new();
        for v1_token in v1.tokens {
            let mut spend_caps = std::collections::BTreeMap::new();
            // Find the token ID by name
            if let Some(item) = tokens_list.iter().find(|t| t.name == v1_token.name) {
                if let Ok(status) = self.get_spend_caps(&item.id).await {
                    if let Some(v) = status.daily_limit_usd {
                        spend_caps.insert("daily".into(), v);
                    }
                    if let Some(v) = status.monthly_limit_usd {
                        spend_caps.insert("monthly".into(), v);
                    }
                    if let Some(v) = status.lifetime_limit_usd {
                        spend_caps.insert("lifetime".into(), v);
                    }
                }
            }
            tokens.push(TokenSpec {
                name: v1_token.name,
                upstream_url: v1_token.upstream_url,
                policies: v1_token.policies,
                log_level: v1_token.log_level,
                spend_caps,
            });
        }

        let policies = v1
            .policies
            .into_iter()
            .map(|p| PolicySpec {
                name: p.name,
                mode: p.mode,
                phase: p.phase,
                rules: p.rules,
                retry: p.retry,
            })
            .collect();

        Ok(ConfigDoc {
            version: "2".into(),
            policies,
            tokens,
        })
    }

    /// Import policies + tokens via the v1 config import endpoint.
    /// Returns counts of what was created/updated.
    pub async fn import_config(&self, doc: &ConfigDoc) -> Result<ImportResult> {
        // Convert to v1 format for the server-side import
        let v1 = serde_json::json!({
            "version": "1",
            "policies": doc.policies,
            "tokens": doc.tokens.iter().map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "upstream_url": t.upstream_url,
                    "policies": t.policies,
                    "log_level": t.log_level,
                })
            }).collect::<Vec<_>>(),
        });

        let url = format!("{}/api/v1/config/import", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .json(&v1)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("import failed ({}): {}", status, body);
        }
        Ok(resp.json().await?)
    }

    /// Set a spend cap for a token (by token ID).
    pub async fn upsert_spend_cap(
        &self,
        token_id: &str,
        period: &str,
        limit_usd: f64,
    ) -> Result<()> {
        let url = format!("{}/api/v1/tokens/{}/spend", self.base_url, token_id);
        let resp = self
            .http
            .put(&url)
            .json(&serde_json::json!({
                "period": period,
                "limit_usd": limit_usd,
            }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!(
                "upsert spend cap failed for token {} ({} {}): {}",
                token_id,
                period,
                limit_usd,
                format!("{}: {}", status, body)
            );
        }
        Ok(())
    }

    /// Delete a spend cap for a token.
    #[allow(dead_code)]
    pub async fn delete_spend_cap(&self, token_id: &str, period: &str) -> Result<()> {
        let url = format!(
            "{}/api/v1/tokens/{}/spend/{}",
            self.base_url, token_id, period
        );
        let resp = self.http.delete(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!(
                "delete spend cap failed for token {} ({}): {}",
                token_id,
                period,
                format!("{}: {}", status, body)
            );
        }
        Ok(())
    }

    /// Find a token's ID by its display name.
    pub async fn find_token_id(&self, name: &str) -> Result<String> {
        let tokens = self.list_tokens().await?;
        tokens
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id.clone())
            .ok_or_else(|| anyhow::anyhow!("token '{}' not found on server", name))
    }

    // ── Private helpers ──────────────────────────────────────────

    async fn list_tokens(&self) -> Result<Vec<TokenListItem>> {
        let mut url = format!("{}/api/v1/tokens", self.base_url);
        if let Some(ref pid) = self.project_id {
            url.push_str(&format!("?project_id={}", pid));
        }
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("list tokens failed: {}", resp.status());
        }
        Ok(resp.json().await?)
    }

    async fn get_spend_caps(&self, token_id: &str) -> Result<SpendStatus> {
        let url = format!("{}/api/v1/tokens/{}/spend", self.base_url, token_id);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("get spend caps failed: {}", resp.status());
        }
        Ok(resp.json().await?)
    }
}

/// V1 config document (matches the server's export format).
#[derive(Deserialize)]
struct V1ConfigDoc {
    #[allow(dead_code)]
    version: String,
    #[serde(default)]
    policies: Vec<V1PolicyExport>,
    #[serde(default)]
    tokens: Vec<V1TokenExport>,
}

#[derive(Deserialize)]
struct V1PolicyExport {
    name: String,
    mode: String,
    phase: String,
    rules: serde_json::Value,
    retry: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct V1TokenExport {
    name: String,
    upstream_url: String,
    #[serde(default)]
    policies: Vec<String>,
    log_level: Option<String>,
}
