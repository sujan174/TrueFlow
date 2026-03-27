-- Migration: Add allowed_providers to tokens for provider-level access control
-- This allows restricting which providers a token can use (e.g., only OpenAI and Anthropic)
-- NULL = all providers allowed (backwards compatible)

ALTER TABLE tokens
ADD COLUMN IF NOT EXISTS allowed_providers TEXT[];

COMMENT ON COLUMN tokens.allowed_providers IS
'Provider access control: list of allowed provider names. NULL = all providers allowed. Example: ["openai", "anthropic"]';

-- Create GIN index for array queries
CREATE INDEX IF NOT EXISTS idx_tokens_allowed_providers
ON tokens USING GIN(allowed_providers);