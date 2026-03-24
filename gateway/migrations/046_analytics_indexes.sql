-- Analytics Performance Indexes
-- Adds missing indexes for dashboard analytics queries

-- HITL Analytics: approval_requests queries filter by project_id and created_at
-- Without this index, these queries cause full table scans
CREATE INDEX IF NOT EXISTS idx_approval_requests_project_time
ON approval_requests(project_id, created_at DESC);

-- HITL Latency Stats: queries filter by reviewed_at IS NOT NULL
-- Used for calculating approval latency (reviewed_at - created_at)
CREATE INDEX IF NOT EXISTS idx_approval_requests_reviewed
ON approval_requests(reviewed_at) WHERE reviewed_at IS NOT NULL;

-- Security Analytics: PII breakdown queries unnest fields_redacted JSONB
-- GIN index enables efficient containment queries
CREATE INDEX IF NOT EXISTS idx_audit_fields_redacted
ON audit_logs USING GIN (fields_redacted);

-- Security Analytics: Shadow policy queries unnest shadow_violations JSONB
-- GIN index enables efficient containment queries
CREATE INDEX IF NOT EXISTS idx_audit_shadow_violations
ON audit_logs USING GIN (shadow_violations);