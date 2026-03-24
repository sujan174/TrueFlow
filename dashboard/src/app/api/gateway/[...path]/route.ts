import { NextRequest, NextResponse } from "next/server"

const GATEWAY_URL = process.env.GATEWAY_URL || "http://localhost:8443"
const ADMIN_KEY = process.env.TRUEFLOW_ADMIN_KEY

// Methods that typically send a request body
const METHODS_WITH_BODY = ["POST", "PUT", "PATCH"] as const

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyRequest(request, params, "GET")
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyRequest(request, params, "POST")
}

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyRequest(request, params, "PUT")
}

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyRequest(request, params, "PATCH")
}

export async function DELETE(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyRequest(request, params, "DELETE")
}

async function proxyRequest(
  request: NextRequest,
  params: Promise<{ path: string[] }>,
  method: string
) {
  if (!ADMIN_KEY) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 }
    )
  }

  const { path } = await params

  // Path traversal protection: filter out . and .. segments
  const safePath = path.filter(segment => segment !== '.' && segment !== '..')
  const pathString = safePath.join("/")

  const searchParams = request.nextUrl.searchParams.toString()
  const gatewayUrl = `${GATEWAY_URL}/api/v1/${pathString}${searchParams ? `?${searchParams}` : ""}`

  // Only set Content-Type for methods that send a body
  const headers: HeadersInit = {
    "X-Admin-Key": ADMIN_KEY,
  }
  if (METHODS_WITH_BODY.includes(method as typeof METHODS_WITH_BODY[number])) {
    headers["Content-Type"] = "application/json"
  }

  const fetchOptions: RequestInit = {
    method,
    headers,
  }

  // Forward body for methods that support it
  if (METHODS_WITH_BODY.includes(method as typeof METHODS_WITH_BODY[number])) {
    try {
      const body = await request.text()
      if (body) {
        fetchOptions.body = body
      }
    } catch {
      // No body
    }
  }

  try {
    const response = await fetch(gatewayUrl, fetchOptions)

    if (!response.ok) {
      // Forward gateway error details instead of generic message
      const errorText = await response.text()
      return new NextResponse(errorText, {
        status: response.status,
        headers: { "Content-Type": "application/json" }
      })
    }

    const data = await response.json()
    return NextResponse.json(data)
  } catch (error) {
    console.error("Gateway proxy error:", error)
    return NextResponse.json(
      { error: "Gateway unavailable" },
      { status: 503 }
    )
  }
}