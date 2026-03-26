// Team types - from gateway/src/middleware/teams.rs

export interface Team {
  id: string
  org_id: string
  name: string
  description: string | null
  max_budget_usd: number | null
  budget_duration: string | null // 'daily' | 'weekly' | 'monthly' | 'yearly'
  allowed_models: string[] | null
  tags: Record<string, unknown>
  is_active: boolean
  created_at: string
  updated_at: string
}

export interface TeamMember {
  id: string
  team_id: string
  user_id: string
  role: 'admin' | 'member' | 'viewer'
  created_at: string
}

export interface TeamSpend {
  id: string
  team_id: string
  period: string // Date string
  total_requests: number
  total_tokens_used: number
  total_spend_usd: number
  updated_at: string
}

export interface CreateTeamRequest {
  name: string
  description?: string
  max_budget_usd?: number
  budget_duration?: 'daily' | 'weekly' | 'monthly' | 'yearly'
  allowed_models?: string[]
  tags?: Record<string, unknown>
}

export interface UpdateTeamRequest {
  name?: string
  description?: string
  max_budget_usd?: number
  budget_duration?: 'daily' | 'weekly' | 'monthly' | 'yearly'
  allowed_models?: string[]
  tags?: Record<string, unknown>
}

// API Key types - from gateway/src/store/postgres/api_keys.rs

export interface ApiKey {
  id: string
  org_id: string
  user_id: string | null
  name: string
  key_prefix: string // First 8 chars of key
  role: 'SuperAdmin' | 'Admin' | 'Member' | 'ReadOnly'
  scopes: string[]
  is_active: boolean
  last_used_at: string | null
  expires_at: string | null
  created_at: string
}

export interface CreateApiKeyRequest {
  name: string
  role: 'Admin' | 'Member' | 'ReadOnly'
  scopes?: string[]
  expires_at?: string
}

export interface CreateApiKeyResponse {
  id: string
  key: string // Full key - only shown once!
  message: string
}

export interface WhoAmIResponse {
  org_id: string
  user_id: string
  role: string
  scopes: string[]
}

// Model Access Group types - from gateway/src/middleware/model_access.rs

export interface ModelAccessGroup {
  id: string
  project_id: string
  name: string
  description: string | null
  models: string[] // Glob patterns like "gpt-4*", "claude-*"
  created_at: string
  updated_at: string
}

export interface CreateModelAccessGroupRequest {
  name: string
  description?: string
  models: string[]
}

export interface UpdateModelAccessGroupRequest {
  name?: string
  description?: string
  models?: string[]
}

// User types for team member selection

export interface User {
  id: string
  org_id: string
  email: string
  role: string
  name: string | null
  picture_url: string | null
  created_at: string
}