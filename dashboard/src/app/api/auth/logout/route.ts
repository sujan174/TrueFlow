import { NextRequest, NextResponse } from "next/server";

/**
 * POST /api/auth/logout — destroys the session cookie and redirects to /login.
 */
export async function POST(request: NextRequest) {
    const baseUrl =
        process.env.NEXTAUTH_URL ||
        `${request.nextUrl.protocol}//${request.nextUrl.host}`;

    const response = NextResponse.redirect(`${baseUrl}/login`);
    response.cookies.delete("ailink_session");
    return response;
}
