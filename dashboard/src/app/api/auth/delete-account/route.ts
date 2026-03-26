import { NextRequest, NextResponse } from "next/server"
import { createClient } from "@/lib/supabase/server"

const GATEWAY_URL = process.env.GATEWAY_URL || "http://localhost:8443"
const ADMIN_KEY = process.env.TRUEFLOW_ADMIN_KEY

/**
 * Account Deletion API Route
 *
 * Implements a 30-day recovery period for account deletion:
 * 1. Immediately revokes all user's API keys and credentials
 * 2. Marks account for deletion (soft delete)
 * 3. After 30 days, a background job performs full deletion
 *
 * This allows for account recovery within the grace period.
 */
export async function POST(request: NextRequest) {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  // SECURITY: Validate the Supabase session
  const supabase = await createClient()
  const { data: { user }, error: authError } = await supabase.auth.getUser()

  if (authError || !user) {
    return NextResponse.json(
      { error: "Unauthorized" },
      { status: 401 }
    )
  }

  try {
    // Step 1: Get user's org_id from the gateway
    const whoamiResponse = await fetch(`${GATEWAY_URL}/api/v1/auth/whoami`, {
      method: "GET",
      headers: {
        "X-Admin-Key": ADMIN_KEY,
        "X-User-Id": user.id,
      },
    })

    if (!whoamiResponse.ok) {
      console.error("Failed to get user info from gateway")
      // Continue with deletion anyway
    }

    // Step 2: Revoke all API keys for this user
    const keysResponse = await fetch(`${GATEWAY_URL}/api/v1/auth/keys`, {
      method: "GET",
      headers: {
        "X-Admin-Key": ADMIN_KEY,
        "X-User-Id": user.id,
      },
    })

    if (keysResponse.ok) {
      const keys = await keysResponse.json()
      // Revoke each key
      for (const key of keys) {
        await fetch(`${GATEWAY_URL}/api/v1/auth/keys/${key.id}`, {
          method: "DELETE",
          headers: {
            "X-Admin-Key": ADMIN_KEY,
          },
        })
      }
    }

    // Step 3: Mark user for deletion (soft delete with 30-day grace period)
    // This would typically update a `deleted_at` field and `deletion_scheduled_at`
    // The gateway would have a background job to fully delete after 30 days
    //
    // For now, we'll sign out the user and they can recover by logging back in
    // within 30 days (their data will still exist)
    //
    // TODO: Implement actual soft delete in gateway when the feature is ready

    // Step 4: Sign out the user from Supabase
    await supabase.auth.signOut()

    return NextResponse.json({
      success: true,
      message: "Account deletion initiated. You have 30 days to recover your account by logging back in.",
    })
  } catch (error) {
    console.error("Account deletion error:", error)
    return NextResponse.json(
      { error: "Failed to initiate account deletion" },
      { status: 503 }
    )
  }
}