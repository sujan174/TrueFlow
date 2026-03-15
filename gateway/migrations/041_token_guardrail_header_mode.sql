-- Add guardrail_header_mode to tokens for security control over X-TrueFlow-Guardrails header
-- Options: 'disabled' (default, security), 'append', 'override'

ALTER TABLE tokens ADD COLUMN IF NOT EXISTS guardrail_header_mode VARCHAR(20) DEFAULT 'disabled';

-- Add comment for documentation
COMMENT ON COLUMN tokens.guardrail_header_mode IS 'Controls how X-TrueFlow-Guardrails header is processed: disabled (ignore, default), append (add to policies), override (replace policies)';