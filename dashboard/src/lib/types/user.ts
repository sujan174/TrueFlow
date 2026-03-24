export interface User {
  id: string
  org_id: string
  email: string
  role: string
  supabase_id?: string
  name?: string
  picture_url?: string
  last_login_at?: string
  last_project_id?: string
  created_at: string
}

export interface SyncUserResponse {
  user_id: string
  org_id: string
  role: string
  is_new_user: boolean
  last_project_id?: string
}

export interface UpdateLastProjectRequest {
  project_id: string
}