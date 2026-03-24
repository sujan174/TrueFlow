import { NextRequest, NextResponse } from "next/server"

const GATEWAY_URL = process.env.GATEWAY_URL || "http://localhost:8443"
const ADMIN_KEY = process.env.TRUEFLOW_ADMIN_KEY

interface SyncUserRequest {
  supabase_id: string
  email: string
  name?: string
  picture?: string
}

export async function POST(request: NextRequest) {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  try {
    const body: SyncUserRequest = await request.json()

    // Validate required fields
    if (!body.supabase_id || !body.email) {
      return NextResponse.json(
        { error: "Missing required fields: supabase_id, email" },
        { status: 400 }
      )
    }

    const response = await fetch(`${GATEWAY_URL}/api/v1/auth/sync-user`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Admin-Key": ADMIN_KEY,
      },
      body: JSON.stringify({
        supabase_id: body.supabase_id,
        email: body.email,
        name: body.name,
        picture: body.picture,
      }),
    })

    if (!response.ok) {
      const errorText = await response.text()
      return new NextResponse(errorText, {
        status: response.status,
        headers: { "Content-Type": "application/json" }
      })
    }

    const data = await response.json()
    return NextResponse.json(data)
  } catch (error) {
    console.error("Sync user error:", error)
    return NextResponse.json(
      { error: "Failed to sync user" },
      { status: 503 }
    )
  }
}