-- Purge all user data to start fresh
-- Run with: psql $DATABASE_URL -f scripts/purge_all_data.sql

BEGIN;

-- Disable foreign key checks temporarily
SET session_replication_role = 'replica';

-- Truncate all user-related tables (in order of dependencies)
TRUNCATE TABLE audit_log_bodies CASCADE;
TRUNCATE TABLE audit_logs CASCADE;
TRUNCATE TABLE tokens CASCADE;
TRUNCATE TABLE credentials CASCADE;
TRUNCATE TABLE policies CASCADE;
TRUNCATE TABLE policy_versions CASCADE;
TRUNCATE TABLE projects CASCADE;
TRUNCATE TABLE users CASCADE;
TRUNCATE TABLE organizations CASCADE;
TRUNCATE TABLE pii_token_vault CASCADE;
TRUNCATE TABLE prompts CASCADE;
TRUNCATE TABLE prompt_versions CASCADE;
TRUNCATE TABLE sessions CASCADE;
TRUNCATE TABLE webhooks CASCADE;
TRUNCATE TABLE notifications CASCADE;
TRUNCATE TABLE approval_requests CASCADE;
TRUNCATE TABLE mcp_servers CASCADE;
TRUNCATE TABLE mcp_server_tools CASCADE;
TRUNCATE TABLE team_members CASCADE;
TRUNCATE TABLE teams CASCADE;
TRUNCATE TABLE team_spend CASCADE;
TRUNCATE TABLE project_spend CASCADE;
TRUNCATE TABLE spend_caps CASCADE;
TRUNCATE TABLE budget_alerts CASCADE;
TRUNCATE TABLE usage_meters CASCADE;
TRUNCATE TABLE tool_call_details CASCADE;
TRUNCATE TABLE services CASCADE;
TRUNCATE TABLE rotation_log CASCADE;
TRUNCATE TABLE oidc_providers CASCADE;
TRUNCATE TABLE api_keys CASCADE;

-- Re-enable foreign key checks
SET session_replication_role = 'origin';

-- Re-seed the default org and project (for dev/testing)
INSERT INTO organizations (id, name, plan) VALUES
    ('00000000-0000-0000-0000-000000000001', 'Default', 'free');

INSERT INTO projects (id, org_id, name) VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'default');

COMMIT;

-- Verify
SELECT 'organizations' as table_name, count(*) as count FROM organizations
UNION ALL
SELECT 'projects', count(*) FROM projects
UNION ALL
SELECT 'users', count(*) FROM users
UNION ALL
SELECT 'tokens', count(*) FROM tokens;