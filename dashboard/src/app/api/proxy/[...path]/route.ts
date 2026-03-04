
import { NextRequest, NextResponse } from "next/server";

const GATEWAY_URL = process.env.GATEWAY_INTERNAL_URL || process.env.NEXT_PUBLIC_GATEWAY_URL || "http://localhost:8443";
const ADMIN_KEY = process.env.TRUEFLOW_ADMIN_KEY;
const DASHBOARD_SECRET = process.env.DASHBOARD_SECRET;

/**
 * Verify the caller has a valid dashboard session token.
 * Checks: cookie `dashboard_token` OR header `X-Dashboard-Token`.
 * In development (no DASHBOARD_SECRET set), all requests are allowed with a warning.
 */
function verifyDashboardAuth(req: NextRequest): boolean {
    if (!DASHBOARD_SECRET) {
        // In dev mode without a secret, allow but log
        if (process.env.NODE_ENV === "development") return true;
        return false; // In production, refuse if not configured
    }

    // Check cookie first
    const cookieToken = req.cookies.get("dashboard_token")?.value;
    if (cookieToken && timingSafeEqual(cookieToken, DASHBOARD_SECRET)) return true;

    // Check header fallback
    const headerToken = req.headers.get("x-dashboard-token");
    if (headerToken && timingSafeEqual(headerToken, DASHBOARD_SECRET)) return true;

    return false;
}

/** Constant-time string comparison to prevent timing attacks */
function timingSafeEqual(a: string, b: string): boolean {
    if (a.length !== b.length) return false;
    let mismatch = 0;
    for (let i = 0; i < a.length; i++) {
        mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
    }
    return mismatch === 0;
}

async function proxyHandler(req: NextRequest, { params }: { params: Promise<{ path: string[] }> }) {
    // ── SEC-01: Authenticate dashboard caller ──────────────────
    if (!verifyDashboardAuth(req)) {
        return NextResponse.json(
            { error: "Unauthorized: valid dashboard token required" },
            { status: 401 }
        );
    }

    const pathStr = (await params).path.join("/");
    // Health check is at root, not /api/v1
    const isHealth = pathStr === "healthz";
    const url = isHealth
        ? `${GATEWAY_URL}/healthz`
        : `${GATEWAY_URL}/api/v1/${pathStr}`;

    const searchParams = req.nextUrl.searchParams.toString();
    const finalUrl = searchParams ? `${url}?${searchParams}` : url;

    if (!ADMIN_KEY) {
        return NextResponse.json(
            { error: "Server misconfiguration: TRUEFLOW_ADMIN_KEY not set" },
            { status: 500 }
        );
    }

    try {
        const headers = new Headers(req.headers);
        headers.delete("host");
        headers.delete("connection");
        // Strip any client-supplied auth headers to prevent spoofing
        headers.delete("x-admin-key");
        headers.delete("authorization");
        headers.delete("x-dashboard-token");
        // Inject server-side Admin Key
        headers.set("X-Admin-Key", ADMIN_KEY);

        // Forward the request
        const upstreamRes = await fetch(finalUrl, {
            method: req.method,
            headers,
            body: req.body,
            // @ts-ignore: duplex is needed for streaming bodies in some node versions/fetch implementations
            duplex: "half",
        });

        const body = upstreamRes.body;

        return new NextResponse(body, {
            status: upstreamRes.status,
            statusText: upstreamRes.statusText,
            headers: upstreamRes.headers,
        });
    } catch (error) {
        console.error("Proxy error:", error);
        return NextResponse.json(
            { error: "Failed to forward request to gateway" },
            { status: 502 }
        );
    }
}

export const GET = proxyHandler;
export const POST = proxyHandler;
export const PUT = proxyHandler;
export const DELETE = proxyHandler;
export const PATCH = proxyHandler;
