import { NextRequest, NextResponse } from "next/server";
import crypto from "crypto";

/**
 * GET /api/auth/google — initiate Google OAuth 2.0 Authorization Code flow.
 *
 * Redirects the user's browser to Google's consent screen.
 * Stores a CSRF `state` value and optional PKCE `code_verifier` in cookies.
 *
 * Required env vars:
 *   GOOGLE_CLIENT_ID
 *   GOOGLE_CLIENT_SECRET
 *   NEXTAUTH_URL  (e.g. http://localhost:3000)
 */
export async function GET(request: NextRequest) {
    const clientId = process.env.GOOGLE_CLIENT_ID;
    if (!clientId) {
        return NextResponse.json(
            { error: "GOOGLE_CLIENT_ID not configured" },
            { status: 500 }
        );
    }

    const baseUrl =
        process.env.NEXTAUTH_URL ||
        `${request.nextUrl.protocol}//${request.nextUrl.host}`;
    const redirectUri = `${baseUrl}/api/auth/google/callback`;

    // CSRF state
    const state = crypto.randomBytes(16).toString("hex");

    const params = new URLSearchParams({
        client_id: clientId,
        redirect_uri: redirectUri,
        response_type: "code",
        scope: "openid email profile",
        state,
        access_type: "offline",
        prompt: "select_account",
    });

    const authUrl = `https://accounts.google.com/o/oauth2/v2/auth?${params}`;

    const response = NextResponse.redirect(authUrl);

    // Store state in a short-lived cookie for CSRF validation on callback
    response.cookies.set("oauth_state", state, {
        httpOnly: true,
        sameSite: "lax",
        secure: process.env.NODE_ENV === "production",
        path: "/",
        maxAge: 300, // 5 minutes
    });

    return response;
}
