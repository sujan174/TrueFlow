-- Migration: 055_secret_references.sql
-- Add workspace-scoped secret references for external vault backends
-- Supports AWS Secrets Manager, HashiCorp Vault KV, Azure Key Vault

-- Create secret_references table for workspace-scoped secret management
CREATE TABLE IF NOT EXISTS secret_references (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,

    -- Vault backend configuration
    vault_backend VARCHAR(50) NOT NULL CHECK (vault_backend IN ('aws_secrets_manager', 'hashicorp_vault_kv', 'azure_key_vault')),
    external_ref TEXT NOT NULL,           -- ARN, path:key, or secret name/URI
    vault_config_id UUID REFERENCES project_vault_configs(id) ON DELETE SET NULL,

    -- Optional provider hint for routing (e.g., 'openai', 'anthropic')
    provider VARCHAR(50),

    -- Injection configuration
    injection_mode VARCHAR(20) NOT NULL DEFAULT 'bearer' CHECK (injection_mode IN ('bearer', 'header', 'query', 'none')),
    injection_header VARCHAR(100) DEFAULT 'Authorization',  -- Header name for header mode

    -- Access control (workspace/team scoping)
    allowed_team_ids JSONB DEFAULT '[]'::jsonb,   -- Array of team UUIDs that can access
    allowed_user_ids JSONB DEFAULT '[]'::jsonb,   -- Array of user UUIDs that can access

    -- Tracking metadata
    last_accessed_at TIMESTAMPTZ,
    last_rotated_at TIMESTAMPTZ,
    version INTEGER DEFAULT 1,
    is_active BOOLEAN NOT NULL DEFAULT true,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,

    UNIQUE(project_id, name)
);

-- Create secret_access_log for audit trail
CREATE TABLE IF NOT EXISTS secret_access_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    secret_reference_id UUID NOT NULL REFERENCES secret_references(id) ON DELETE CASCADE,
    credential_id UUID REFERENCES credentials(id) ON DELETE SET NULL,
    token_id UUID REFERENCES tokens(id) ON DELETE SET NULL,
    accessed_by UUID REFERENCES users(id) ON DELETE SET NULL,

    access_type VARCHAR(20) NOT NULL CHECK (access_type IN ('read', 'list', 'rotate')),
    vault_backend VARCHAR(50) NOT NULL CHECK (vault_backend IN ('aws_secrets_manager', 'hashicorp_vault_kv', 'azure_key_vault')),
    latency_ms INTEGER,
    status VARCHAR(20) NOT NULL CHECK (status IN ('success', 'failure')),
    error_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for secret_references
CREATE INDEX IF NOT EXISTS idx_secret_references_project ON secret_references(project_id);
CREATE INDEX IF NOT EXISTS idx_secret_references_vault_backend ON secret_references(vault_backend);
CREATE INDEX IF NOT EXISTS idx_secret_references_vault_config ON secret_references(vault_config_id);
CREATE INDEX IF NOT EXISTS idx_secret_references_provider ON secret_references(provider) WHERE provider IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_secret_references_active ON secret_references(project_id, is_active) WHERE is_active = true;

-- Index for team access lookups (GIN index for JSONB array contains)
CREATE INDEX IF NOT EXISTS idx_secret_references_team_access ON secret_references USING GIN (allowed_team_ids);
CREATE INDEX IF NOT EXISTS idx_secret_references_user_access ON secret_references USING GIN (allowed_user_ids);

-- Indexes for secret_access_log
CREATE INDEX IF NOT EXISTS idx_secret_access_log_reference ON secret_access_log(secret_reference_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_access_log_token ON secret_access_log(token_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_access_log_credential ON secret_access_log(credential_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_access_log_user ON secret_access_log(accessed_by, created_at DESC);

-- Comments for documentation
COMMENT ON TABLE secret_references IS 'Workspace-scoped references to secrets stored in external vault backends (AWS Secrets Manager, HashiCorp Vault KV, Azure Key Vault)';
COMMENT ON COLUMN secret_references.vault_backend IS 'External vault backend: aws_secrets_manager, hashicorp_vault_kv, azure_key_vault';
COMMENT ON COLUMN secret_references.external_ref IS 'External reference to the secret: ARN for AWS, path:key for HashiCorp, secret name/URI for Azure';
COMMENT ON COLUMN secret_references.injection_mode IS 'How to inject the secret: bearer (Authorization: Bearer <secret>), header (custom header), query (query param), none (manual retrieval only)';
COMMENT ON COLUMN secret_references.allowed_team_ids IS 'JSONB array of team UUIDs allowed to access this secret. Empty array means no team restrictions.';
COMMENT ON COLUMN secret_references.allowed_user_ids IS 'JSONB array of user UUIDs allowed to access this secret. Empty array means no user restrictions.';

COMMENT ON TABLE secret_access_log IS 'Audit trail for all secret access operations';
COMMENT ON COLUMN secret_access_log.access_type IS 'Type of access: read (retrieved secret), list (listed metadata), rotate (triggered rotation)';
COMMENT ON COLUMN secret_access_log.status IS 'Operation result: success or failure';