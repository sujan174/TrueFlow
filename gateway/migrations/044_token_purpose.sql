-- Migration: 044_token_purpose.sql
-- Purpose: Add purpose field to tokens to distinguish LLM vs Tool usage
-- Values: 'llm' (default) - Token only for LLM calls
--         'tool' - Token only for tool/MCP calls
--         'both' - Token for either (backward compatible)

-- Add purpose field to tokens
ALTER TABLE tokens
    ADD COLUMN purpose VARCHAR(16) DEFAULT 'llm'
    CHECK (purpose IN ('llm', 'tool', 'both'));

-- Index for filtering by purpose
CREATE INDEX idx_tokens_purpose ON tokens(purpose);

-- Add token_purpose to audit_logs for request-level tracking
ALTER TABLE audit_logs
    ADD COLUMN token_purpose VARCHAR(16);

CREATE INDEX idx_audit_token_purpose ON audit_logs(token_purpose, created_at DESC);

-- Comment for documentation
COMMENT ON COLUMN tokens.purpose IS 'Token purpose: llm (LLM calls only), tool (tool/MCP calls only), both (either)';
COMMENT ON COLUMN audit_logs.token_purpose IS 'Token purpose at time of request (copied from token.purpose)';