-- Migration: 052_azure_key_vault_config.sql
-- Add Azure Key Vault configuration columns to project_vault_configs
-- This allows customers to fetch secrets at runtime from their own Azure Key Vault.

-- Add Azure Key Vault configuration columns
ALTER TABLE project_vault_configs
ADD COLUMN IF NOT EXISTS azure_kv_vault_url TEXT,
ADD COLUMN IF NOT EXISTS azure_kv_tenant_id TEXT,
ADD COLUMN IF NOT EXISTS azure_kv_client_id TEXT,
ADD COLUMN IF NOT EXISTS azure_kv_client_secret TEXT,
ADD COLUMN IF NOT EXISTS azure_kv_use_managed_identity BOOLEAN DEFAULT false;

-- Create index for faster lookups by vault URL
CREATE INDEX IF NOT EXISTS idx_project_vault_configs_azure_kv_vault_url
ON project_vault_configs(azure_kv_vault_url) WHERE azure_kv_vault_url IS NOT NULL;

-- Comments for documentation
COMMENT ON COLUMN project_vault_configs.azure_kv_vault_url IS 'Azure Key Vault URL (e.g., https://my-vault.vault.azure.net/)';
COMMENT ON COLUMN project_vault_configs.azure_kv_tenant_id IS 'Azure AD tenant ID for service principal authentication';
COMMENT ON COLUMN project_vault_configs.azure_kv_client_id IS 'Azure AD client (application) ID for service principal authentication';
COMMENT ON COLUMN project_vault_configs.azure_kv_client_secret IS 'Azure AD client secret for service principal authentication';
COMMENT ON COLUMN project_vault_configs.azure_kv_use_managed_identity IS 'Use Azure Managed Identity instead of service principal (for VM/Container deployment)';