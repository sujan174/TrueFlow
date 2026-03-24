import { NextResponse } from "next/server"

const GATEWAY_URL = process.env.GATEWAY_URL || "http://localhost:8443"
const ADMIN_KEY = process.env.TRUEFLOW_ADMIN_KEY

export async function GET() {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  const response = await fetch(`${GATEWAY_URL}/api/v1/projects`, {
    headers: {
      "Content-Type": "application/json",
      "X-Admin-Key": ADMIN_KEY,
    },
    cache: "no-store",
  })

  if (!response.ok) {
    return NextResponse.json(
      { error: `Gateway error: ${response.status}` },
      { status: response.status }
    )
  }

  const data = await response.json()
  return NextResponse.json(data)
}

export async function POST(request: Request) {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  const body = await request.json()

  const response = await fetch(`${GATEWAY_URL}/api/v1/projects`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Admin-Key": ADMIN_KEY,
    },
    body: JSON.stringify(body),
  })

  if (!response.ok) {
    const error = await response.text()
    return NextResponse.json(
      { error: `Gateway error: ${response.status}` },
      { status: response.status }
    )
  }

  const data = await response.json()
  return NextResponse.json(data)
}