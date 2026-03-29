-- Migration: 054_azure_user_assigned_identity.sql
-- Add support for user-assigned managed identity in Azure Key Vault

-- Add managed_identity_client_id column for user-assigned managed identity support
ALTER TABLE project_vault_configs
ADD COLUMN IF NOT EXISTS azure_kv_managed_identity_client_id TEXT;

-- Comment for documentation
COMMENT ON COLUMN project_vault_configs.azure_kv_managed_identity_client_id IS
'Client ID for user-assigned managed identity (optional). If not set, system-assigned managed identity is used when azure_kv_use_managed_identity is true';