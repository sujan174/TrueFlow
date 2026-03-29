//! Secret Reference domain model.
//!
//! A SecretReference is a workspace-scoped reference to an external secret
//! stored in AWS Secrets Manager, HashiCorp Vault KV, or Azure Key Vault.
//! Unlike credentials (which store encrypted values), secret references
//! point to secrets managed externally.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::vault::VaultBackend;

/// A reference to an external secret in a vault backend.
///
/// Secret references provide a workspace-scoped abstraction over external
/// secrets, with access control via team/user allowlists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretReference {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub vault_backend: VaultBackend,
    /// External reference to the secret (ARN for AWS, path:key for HashiCorp, URI for Azure)
    pub external_ref: String,
    /// Optional link to project vault config for authentication
    pub vault_config_id: Option<Uuid>,
    /// Provider type this secret is for (e.g., "openai", "anthropic")
    pub provider: Option<String>,
    /// How to inject the secret: bearer, header, query, none
    pub injection_mode: String,
    /// Header name for injection (e.g., "Authorization", "X-API-Key")
    pub injection_header: String,
    /// Team IDs allowed to access this secret (empty = all teams)
    pub allowed_team_ids: Option<Vec<Uuid>>,
    /// User IDs allowed to access this secret (empty = all users)
    pub allowed_user_ids: Option<Vec<Uuid>>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    /// Secret version if the vault supports versioning
    pub version: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

/// Request to create a new secret reference.
#[derive(Debug, Deserialize)]
pub struct CreateSecretReferenceRequest {
    pub name: String,
    pub description: Option<String>,
    /// Vault backend type (aws_secrets_manager, hashicorp_vault_kv, azure_key_vault)
    pub vault_backend: String,
    /// External reference (ARN, path:key, or URI)
    pub external_ref: String,
    /// Optional vault config for authentication
    pub vault_config_id: Option<Uuid>,
    /// Provider this secret is for
    pub provider: Option<String>,
    /// Injection mode: bearer (default), header, query, none
    #[serde(default)]
    pub injection_mode: Option<String>,
    /// Header name for injection (default: Authorization)
    #[serde(default)]
    pub injection_header: Option<String>,
    /// Team IDs allowed to access (None = all teams)
    pub allowed_team_ids: Option<Vec<Uuid>>,
    /// User IDs allowed to access (None = all users)
    pub allowed_user_ids: Option<Vec<Uuid>>,
}

/// Request to update an existing secret reference.
#[derive(Debug, Deserialize, Default)]
pub struct UpdateSecretReferenceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub external_ref: Option<String>,
    pub vault_config_id: Option<Uuid>,
    pub provider: Option<String>,
    pub injection_mode: Option<String>,
    pub injection_header: Option<String>,
    pub allowed_team_ids: Option<Vec<Uuid>>,
    pub allowed_user_ids: Option<Vec<Uuid>>,
    pub version: Option<String>,
    pub is_active: Option<bool>,
}

/// Filter parameters for listing secret references.
#[derive(Debug, Deserialize, Default)]
pub struct SecretReferenceFilter {
    /// Filter by vault backend type
    pub vault_backend: Option<String>,
    /// Filter by provider
    pub provider: Option<String>,
    /// Filter by active status
    pub is_active: Option<bool>,
}

/// Access check result for secret references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretAccessResult {
    /// Access is granted
    Granted,
    /// Access is denied (no permission)
    Denied,
    /// Secret reference not found
    NotFound,
}