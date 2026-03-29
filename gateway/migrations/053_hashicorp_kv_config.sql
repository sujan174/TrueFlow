-- Migration: 053_hashicorp_kv_config.sql
-- Add HashiCorp Vault KV v2 configuration columns to project_vault_configs
-- This allows customers to fetch secrets at runtime from their own Vault KV store.

-- Add HashiCorp Vault KV configuration columns
ALTER TABLE project_vault_configs
ADD COLUMN IF NOT EXISTS hc_kv_address TEXT,
ADD COLUMN IF NOT EXISTS hc_kv_mount_path VARCHAR(100) DEFAULT 'secret',
ADD COLUMN IF NOT EXISTS hc_kv_namespace TEXT,
ADD COLUMN IF NOT EXISTS hc_kv_auth_method VARCHAR(20) DEFAULT 'approle',
ADD COLUMN IF NOT EXISTS hc_kv_approle_role_id TEXT,
ADD COLUMN IF NOT EXISTS hc_kv_approle_secret_id TEXT,
ADD COLUMN IF NOT EXISTS hc_kv_k8s_role TEXT,
ADD COLUMN IF NOT EXISTS hc_kv_k8s_jwt_path TEXT DEFAULT '/var/run/secrets/kubernetes.io/serviceaccount/token',
ADD COLUMN IF NOT EXISTS hc_kv_skip_tls_verify BOOLEAN DEFAULT false;

-- Comments for documentation
COMMENT ON COLUMN project_vault_configs.hc_kv_address IS 'HashiCorp Vault server address for KV secrets (e.g., https://vault.example.com:8200)';
COMMENT ON COLUMN project_vault_configs.hc_kv_mount_path IS 'KV v2 secrets engine mount path (default: secret)';
COMMENT ON COLUMN project_vault_configs.hc_kv_namespace IS 'Vault namespace (Enterprise feature)';
COMMENT ON COLUMN project_vault_configs.hc_kv_auth_method IS 'Authentication method: approle or kubernetes';
COMMENT ON COLUMN project_vault_configs.hc_kv_approle_role_id IS 'AppRole role ID for Vault authentication';
COMMENT ON COLUMN project_vault_configs.hc_kv_approle_secret_id IS 'AppRole secret ID for Vault authentication';
COMMENT ON COLUMN project_vault_configs.hc_kv_k8s_role IS 'Kubernetes auth role for Vault authentication';
COMMENT ON COLUMN project_vault_configs.hc_kv_k8s_jwt_path IS 'Path to Kubernetes service account JWT token';
COMMENT ON COLUMN project_vault_configs.hc_kv_skip_tls_verify IS 'Skip TLS verification (not recommended for production)';