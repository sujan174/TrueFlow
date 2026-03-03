import { NextRequest, NextResponse } from "next/server";

/**
 * Next.js middleware — runs on every request server-side.
 *
 * Two auth modes (determined by env vars):
 *
 * 1. **Google SSO** (GOOGLE_CLIENT_ID is set):
 *    Checks for a valid `ailink_session` cookie. If missing, redirects to /login.
 *    The login page initiates the Google OAuth flow.
 *
 * 2. **Shared Secret** (DASHBOARD_SECRET is set, no GOOGLE_CLIENT_ID):
 *    Sets the `dashboard_token` cookie automatically so the browser includes
 *    it on every /api/proxy/* call. Legacy mode — no login page.
 *
 * Routes excluded from auth: /login, /api/auth/*, static assets.
 */
export function middleware(request: NextRequest) {
    const { pathname } = request.nextUrl;
    const response = NextResponse.next();

    const googleSsoEnabled = !!process.env.GOOGLE_CLIENT_ID;

    // ── Google SSO Mode ──
    if (googleSsoEnabled) {
        // Skip auth for login page, auth API routes, and static assets
        const isPublicRoute =
            pathname === "/login" ||
            pathname.startsWith("/api/auth/") ||
            pathname.startsWith("/_next/") ||
            pathname === "/favicon.ico";

        if (isPublicRoute) return response;

        // Check for session cookie
        const session = request.cookies.get("ailink_session")?.value;
        if (!session) {
            const loginUrl = new URL("/login", request.url);
            return NextResponse.redirect(loginUrl);
        }

        // Validate session expiry
        try {
            const payload = JSON.parse(
                Buffer.from(session, "base64url").toString()
            );
            if (payload.exp && payload.exp < Math.floor(Date.now() / 1000)) {
                const loginUrl = new URL("/login", request.url);
                loginUrl.searchParams.set("error", "Session expired — please sign in again");
                const redirect = NextResponse.redirect(loginUrl);
                redirect.cookies.delete("ailink_session");
                redirect.cookies.delete("ailink_user");
                return redirect;
            }

            // Set a client-readable mirror cookie (non-httpOnly) with display info
            const userInfo = JSON.stringify({
                email: payload.email,
                name: payload.name,
                picture: payload.picture,
            });
            const existing_user = request.cookies.get("ailink_user")?.value;
            if (!existing_user) {
                response.cookies.set("ailink_user", btoa(userInfo), {
                    httpOnly: false,
                    sameSite: "lax",
                    secure: process.env.NODE_ENV === "production",
                    path: "/",
                    maxAge: 60 * 60 * 24 * 7,
                });
            }
        } catch {
            // Malformed session — redirect to login
            const loginUrl = new URL("/login", request.url);
            const redirect = NextResponse.redirect(loginUrl);
            redirect.cookies.delete("ailink_session");
            redirect.cookies.delete("ailink_user");
            return redirect;
        }

        // Also set the dashboard_token for gateway API auth if DASHBOARD_SECRET exists
        const secret = process.env.DASHBOARD_SECRET;
        if (secret) {
            const existing = request.cookies.get("dashboard_token")?.value;
            if (existing !== secret) {
                response.cookies.set("dashboard_token", secret, {
                    httpOnly: true,
                    sameSite: "strict",
                    secure: process.env.NODE_ENV === "production",
                    path: "/",
                    maxAge: 60 * 60 * 24,
                });
            }
        }

        return response;
    }

    // ── Legacy Shared Secret Mode ──
    const secret = process.env.DASHBOARD_SECRET;
    if (!secret) return response;

    const existing = request.cookies.get("dashboard_token")?.value;
    if (existing !== secret) {
        response.cookies.set("dashboard_token", secret, {
            httpOnly: true,
            sameSite: "strict",
            secure: process.env.NODE_ENV === "production",
            path: "/",
            maxAge: 60 * 60 * 24,
        });
    }

    return response;
}

export const config = {
    matcher: ["/((?!api/proxy|_next/static|_next/image|favicon.ico).*)"],
};
