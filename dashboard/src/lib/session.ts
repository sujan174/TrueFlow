"use client";

import { useState, useEffect } from "react";

export interface UserSession {
    email: string;
    name: string;
    picture: string;
    sub: string;
    iat: number;
    exp: number;
}

/**
 * Hook to read the user session from the trueflow_session cookie (client-side).
 * Returns null if no session or if SSO is not enabled (cookie is httpOnly,
 * so we read it via a non-httpOnly mirror set by middleware).
 *
 * For simplicity, we expose a client-readable version.
 */
export function useSession(): UserSession | null {
    const [session, setSession] = useState<UserSession | null>(null);

    useEffect(() => {
        let active = true;
        try {
            // Read from a client-visible cookie or localStorage
            const raw = document.cookie
                .split("; ")
                .find((c) => c.startsWith("trueflow_user="))
                ?.split("=")[1];

            if (raw && active) {
                const decoded = JSON.parse(
                    atob(decodeURIComponent(raw))
                );
                setTimeout(() => {
                    if (active) setSession(decoded);
                }, 0);
            }
        } catch {
            // No session or parse error
        }
        return () => {
            active = false;
        };
    }, []);

    return session;
}

/**
 * Check if Google SSO is enabled (GOOGLE_CLIENT_ID set).
 * In client components, check if the login link is accessible.
 */
export function isSsoEnabled(): boolean {
    // This runs client-side — we detect SSO by checking if the session cookie exists
    if (typeof document === "undefined") return false;
    return document.cookie.includes("trueflow_user=");
}
