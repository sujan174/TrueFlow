-- Migration: 049_vault_backend.sql
-- Add vault_backend column to credentials for external KMS support

-- Add vault_backend column to credentials
ALTER TABLE credentials
ADD COLUMN IF NOT EXISTS vault_backend VARCHAR(50) NOT NULL DEFAULT 'builtin';

-- Create index for vault_backend queries
CREATE INDEX IF NOT EXISTS idx_credentials_vault_backend ON credentials(vault_backend);

-- Add vault_config to system_settings for global configuration
INSERT INTO system_settings (key, value, description)
VALUES (
    'vault_config',
    '{"default_backend": "builtin"}',
    'Vault backend configuration (builtin, aws_kms, hashicorp_vault)'
) ON CONFLICT (key) DO NOTHING;

-- Create audit trail table for vault migrations
CREATE TABLE IF NOT EXISTS vault_migrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_id UUID NOT NULL REFERENCES credentials(id) ON DELETE CASCADE,
    source_backend VARCHAR(50) NOT NULL,
    target_backend VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending, completed, failed
    error_message TEXT,
    migrated_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_vault_migrations_credential ON vault_migrations(credential_id);
CREATE INDEX IF NOT EXISTS idx_vault_migrations_status ON vault_migrations(status);