-- Add spend_cap_overrun column to track billing anomalies
-- when concurrent requests race past the spend cap
ALTER TABLE audit_logs ADD COLUMN IF NOT EXISTS spend_cap_overrun BOOLEAN DEFAULT FALSE;

-- Add index for querying overrun events
CREATE INDEX IF NOT EXISTS idx_audit_spend_cap_overrun ON audit_logs(spend_cap_overrun) WHERE spend_cap_overrun = TRUE;