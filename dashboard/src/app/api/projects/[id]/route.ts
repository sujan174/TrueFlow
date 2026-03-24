import { NextResponse } from "next/server"

const GATEWAY_URL = process.env.GATEWAY_URL || "http://localhost:8443"
const ADMIN_KEY = process.env.TRUEFLOW_ADMIN_KEY

export async function PUT(
  request: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  const { id } = await params
  const body = await request.json()

  const response = await fetch(`${GATEWAY_URL}/api/v1/projects/${id}`, {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
      "X-Admin-Key": ADMIN_KEY,
    },
    body: JSON.stringify(body),
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

export async function DELETE(
  request: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  const { id } = await params

  const response = await fetch(`${GATEWAY_URL}/api/v1/projects/${id}`, {
    method: "DELETE",
    headers: {
      "Content-Type": "application/json",
      "X-Admin-Key": ADMIN_KEY,
    },
  })

  if (!response.ok) {
    return NextResponse.json(
      { error: `Gateway error: ${response.status}` },
      { status: response.status }
    )
  }

  return new NextResponse(null, { status: 204 })
}