import { createBrowserClient } from '@supabase/ssr'

export function createClient() {
  const supabaseUrl = process.env.NEXT_PUBLIC_SUPABASE_URL
  const supabaseAnonKey = process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY

  // Return a dummy client during build or when env vars are missing
  if (!supabaseUrl || !supabaseAnonKey) {
    console.warn('Supabase environment variables are missing. Auth will not work.')
    return createBrowserClient(
      'https://placeholder.supabase.co',
      'placeholder-key'
    )
  }

  return createBrowserClient(
    supabaseUrl,
    supabaseAnonKey
  )
}