-- Add last_project_id to users table for cross-device project persistence
-- This stores the user's preferred/last used project

ALTER TABLE users ADD COLUMN IF NOT EXISTS last_project_id UUID REFERENCES projects(id) ON DELETE SET NULL;

-- Index for faster lookups
CREATE INDEX IF NOT EXISTS idx_users_last_project ON users(last_project_id);

-- Comment for documentation
COMMENT ON COLUMN users.last_project_id IS 'The last project the user had selected, for cross-device persistence';