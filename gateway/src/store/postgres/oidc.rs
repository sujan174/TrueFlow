use super::PgStore;
use super::types::OidcProviderRow;

impl PgStore {
    /// Find an enabled OIDC provider matching the given issuer URL.
    /// Used by the auth middleware to validate JWT Bearer tokens.
    pub async fn get_oidc_provider_by_issuer(
        &self,
        issuer_url: &str,
    ) -> anyhow::Result<Option<OidcProviderRow>> {
        let row = sqlx::query_as::<_, OidcProviderRow>(
            r#"
            SELECT id, org_id, name, issuer_url, client_id, jwks_uri,
                   audience, claim_mapping, default_role, default_scopes, enabled
            FROM oidc_providers
            WHERE issuer_url = $1 AND enabled = true
            LIMIT 1
            "#,
        )
        .bind(issuer_url)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}
