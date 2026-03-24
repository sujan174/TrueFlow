-- SaaS Builder & Enterprise User Attribution
-- Roadmap item: External user attribution for SaaS builders

-- Add customer/external user attribution to tokens
ALTER TABLE tokens
    ADD COLUMN external_user_id VARCHAR(255),
    ADD COLUMN metadata JSONB DEFAULT '{}';

-- Index for filtering by external user
CREATE INDEX idx_tokens_external_user ON tokens(external_user_id)
    WHERE external_user_id IS NOT NULL;

-- Index for metadata queries (GIN for JSONB containment)
CREATE INDEX idx_tokens_metadata ON tokens USING GIN (metadata);

-- Add external_user_id to audit_logs for request-level attribution
-- This allows SaaS builders to track spend per customer across all requests
ALTER TABLE audit_logs
    ADD COLUMN external_user_id VARCHAR(255);

CREATE INDEX idx_audit_external_user ON audit_logs(external_user_id, created_at DESC)
    WHERE external_user_id IS NOT NULL;

-- Comment for documentation
COMMENT ON COLUMN tokens.external_user_id IS 'External user/customer identifier for SaaS builders (e.g., customer ID from billing system)';
COMMENT ON COLUMN tokens.metadata IS 'Flexible JSONB for SaaS-specific data (plan tier, region, custom attributes)';
COMMENT ON COLUMN audit_logs.external_user_id IS 'Copied from token.external_user_id for request-level attribution';