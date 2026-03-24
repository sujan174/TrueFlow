import { createClient } from "@/lib/supabase/server";
import { NextResponse } from "next/server";

export async function GET(request: Request) {
  const { searchParams, origin } = new URL(request.url);
  const code = searchParams.get("code");
  const next = searchParams.get("next") ?? "/";

  if (code) {
    const supabase = await createClient();
    const { error } = await supabase.auth.exchangeCodeForSession(code);

    if (!error) {
      // Sync user to gateway database
      const { data: { user } } = await supabase.auth.getUser();
      if (user) {
        try {
          const gatewayUrl = process.env.GATEWAY_URL || "http://localhost:8443";
          const adminKey = process.env.TRUEFLOW_ADMIN_KEY;

          if (adminKey) {
            const syncResponse = await fetch(`${gatewayUrl}/api/v1/auth/sync-user`, {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
                "X-Admin-Key": adminKey,
              },
              body: JSON.stringify({
                supabase_id: user.id,
                email: user.email,
                name: user.user_metadata?.name || user.user_metadata?.full_name,
                picture: user.user_metadata?.avatar_url || user.user_metadata?.picture,
              }),
            });

            // Capture last_project_id from response and set as cookie
            if (syncResponse.ok) {
              const userData = await syncResponse.json();
              if (userData.last_project_id) {
                const response = NextResponse.redirect(`${origin}${next}`);
                // Set cookie with last_project_id (HTTP-only for security)
                response.cookies.set("trueflow_last_project", userData.last_project_id, {
                  httpOnly: true,
                  secure: process.env.NODE_ENV === "production",
                  sameSite: "lax",
                  maxAge: 60 * 60 * 24 * 30, // 30 days
                  path: "/",
                });
                return response;
              }
            }
          }
        } catch (syncError) {
          // Log but don't fail the login
          console.error("Failed to sync user to gateway:", syncError);
        }
      }

      return NextResponse.redirect(`${origin}${next}`);
    }
  }

  // Return the user to an error page with instructions
  return NextResponse.redirect(`${origin}/login?error=auth_failed`);
}