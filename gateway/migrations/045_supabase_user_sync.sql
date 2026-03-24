-- Migration 045: Add Supabase Auth user linking
-- Adds columns to users table for Supabase Auth integration
-- Dashboard will sync users from Supabase to gateway on first login

-- Add Supabase user linking columns
ALTER TABLE users ADD COLUMN IF NOT EXISTS supabase_id UUID UNIQUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS name VARCHAR(255);
ALTER TABLE users ADD COLUMN IF NOT EXISTS picture_url TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMPTZ;

-- Indexes for efficient lookups
CREATE INDEX IF NOT EXISTS idx_users_supabase_id ON users(supabase_id) WHERE supabase_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- Comments for documentation
COMMENT ON COLUMN users.supabase_id IS 'Supabase Auth user ID for dashboard login';
COMMENT ON COLUMN users.name IS 'Display name from Supabase Auth';
COMMENT ON COLUMN users.picture_url IS 'Avatar URL from OAuth provider';
COMMENT ON COLUMN users.last_login_at IS 'Timestamp of last successful login';