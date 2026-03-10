import { NextRequest, NextResponse } from "next/server";
import { SignJWT } from "jose";

/**
 * GET /api/auth/google/callback — Google OAuth 2.0 callback handler.
 *
 * Exchanges the authorization code for tokens, extracts user info,
 * creates a signed session cookie, and redirects to the dashboard.
 */
export async function GET(request: NextRequest) {
    const { searchParams } = request.nextUrl;
    const code = searchParams.get("code");
    const state = searchParams.get("state");
    const error = searchParams.get("error");

    const baseUrl =
        process.env.NEXTAUTH_URL ||
        `${request.nextUrl.protocol}//${request.nextUrl.host}`;

    // Error from Google
    if (error) {
        return NextResponse.redirect(
            `${baseUrl}/login?error=${encodeURIComponent(error)}`
        );
    }

    // Validate CSRF state
    const storedState = request.cookies.get("oauth_state")?.value;
    if (!code || !state || state !== storedState) {
        return NextResponse.redirect(
            `${baseUrl}/login?error=${encodeURIComponent("Invalid OAuth state — please try again")}`
        );
    }

    // Exchange code for tokens
    const clientId = process.env.GOOGLE_CLIENT_ID!;
    const clientSecret = process.env.GOOGLE_CLIENT_SECRET!;
    const redirectUri = `${baseUrl}/api/auth/google/callback`;

    try {
        const tokenRes = await fetch("https://oauth2.googleapis.com/token", {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body: new URLSearchParams({
                code,
                client_id: clientId,
                client_secret: clientSecret,
                redirect_uri: redirectUri,
                grant_type: "authorization_code",
            }),
        });

        if (!tokenRes.ok) {
            const errBody = await tokenRes.text();
            console.error("Google token exchange failed:", errBody);
            return NextResponse.redirect(
                `${baseUrl}/login?error=${encodeURIComponent("Token exchange failed")}`
            );
        }

        const tokens = await tokenRes.json();

        // Get user info
        const userRes = await fetch(
            "https://www.googleapis.com/oauth2/v2/userinfo",
            { headers: { Authorization: `Bearer ${tokens.access_token}` } }
        );

        if (!userRes.ok) {
            return NextResponse.redirect(
                `${baseUrl}/login?error=${encodeURIComponent("Failed to fetch user info")}`
            );
        }

        const user = await userRes.json();

        // Create session payload
        const sessionSecret = process.env.SESSION_SECRET;
        if (!sessionSecret) {
            console.error("SESSION_SECRET is not set");
            return NextResponse.redirect(
                `${baseUrl}/login?error=${encodeURIComponent("Server configuration error")}`
            );
        }

        const secretKey = new TextEncoder().encode(sessionSecret);
        const now = Math.floor(Date.now() / 1000);

        // Sign session as a JWT (HMAC-SHA256)
        const sessionToken = await new SignJWT({
            email: user.email,
            name: user.name,
            picture: user.picture,
            sub: user.id,
        })
            .setProtectedHeader({ alg: "HS256" })
            .setIssuedAt(now)
            .setExpirationTime(now + 60 * 60 * 24 * 7) // 7 days
            .sign(secretKey);

        const response = NextResponse.redirect(`${baseUrl}/`);

        // Set session cookie
        response.cookies.set("trueflow_session", sessionToken, {
            httpOnly: true,
            sameSite: "lax",
            secure: process.env.NODE_ENV === "production",
            path: "/",
            maxAge: 60 * 60 * 24 * 7, // 7 days
        });

        // Clean up oauth state cookie
        response.cookies.delete("oauth_state");

        return response;
    } catch (err) {
        console.error("Google OAuth callback error:", err);
        return NextResponse.redirect(
            `${baseUrl}/login?error=${encodeURIComponent("Authentication failed")}`
        );
    }
}
