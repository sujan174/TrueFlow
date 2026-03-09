"use client";

import { useState, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useTheme } from "next-themes";
import { cn } from "@/lib/utils";
import {
    Key,
    ShieldAlert,
    CheckCircle,
    ClipboardList,
    Fingerprint,
    BarChart3,
    LayoutDashboard,
    Moon,
    Sun,
    Menu,
    X,
    FlaskConical,
    Activity,
    LockKeyhole,
    Settings,
    Layers,
    FlaskRound,
    Webhook,
    Wrench,
    CreditCard,
    FileCode2,
    Database,
    ShieldCheck,
    MessageSquareText,
    ScrollText,
    LogOut,
    User,
} from "lucide-react";
import { ProjectSwitcher } from "@/components/project-switcher";

const groups = [
    {
        label: "Home",
        routes: [
            { href: "/", label: "Dashboard", icon: LayoutDashboard },
            { href: "/analytics", label: "Analytics", icon: BarChart3 },
            { href: "/billing", label: "Billing", icon: CreditCard },
        ]
    },
    {
        label: "Agents & Keys",
        routes: [
            { href: "/virtual-keys", label: "Virtual Keys", icon: Key },
            { href: "/api-keys", label: "API Keys", icon: LockKeyhole },
            { href: "/upstreams", label: "Upstreams", icon: Activity },
            { href: "/vault", label: "Vault", icon: Fingerprint },
        ]
    },
    {
        label: "Safety & Policies",
        routes: [
            { href: "/policies", label: "Policies", icon: ScrollText },
            { href: "/guardrails", label: "Guardrails", icon: ShieldAlert },
            { href: "/model-access-groups", label: "Model Access", icon: ShieldCheck },
            { href: "/approvals", label: "Approvals", icon: CheckCircle },
        ]
    },
    {
        label: "Studio",
        routes: [
            { href: "/prompts", label: "Prompts", icon: MessageSquareText },
            { href: "/playground", label: "Playground", icon: FlaskConical },
            { href: "/experiments", label: "Experiments", icon: FlaskRound },
        ]
    },
    {
        label: "Observe",
        routes: [
            { href: "/audit", label: "Audit Logs", icon: ClipboardList },
            { href: "/sessions", label: "Sessions", icon: Layers },
            { href: "/cache", label: "Cache", icon: Database },
            { href: "/tools", label: "Tools & MCP", icon: Wrench },
            { href: "/webhooks", label: "Webhooks", icon: Webhook },
            { href: "/config", label: "Config Export", icon: FileCode2 },
        ]
    },
];

export function MobileNav() {
    const [open, setOpen] = useState(false);
    const [mounted, setMounted] = useState(false);
    const pathname = usePathname();
    const { theme, setTheme } = useTheme();

    // Wait for client mount before rendering portal
    useEffect(() => {
        let active = true;
        setTimeout(() => {
            if (active) setMounted(true);
        }, 0);
        return () => { active = false; };
    }, []);

    // Close on navigation
    useEffect(() => {
        let active = true;
        setTimeout(() => {
            if (active) setOpen(false);
        }, 0);
        return () => { active = false; };
    }, [pathname]);

    // Lock body scroll when open
    useEffect(() => {
        document.body.style.overflow = open ? "hidden" : "";
        return () => { document.body.style.overflow = ""; };
    }, [open]);

    // Close on Escape
    useEffect(() => {
        if (!open) return;
        const handler = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
        document.addEventListener("keydown", handler);
        return () => document.removeEventListener("keydown", handler);
    }, [open]);

    const drawer = mounted ? createPortal(
        <>
            {/* Backdrop — rendered at body level so no stacking context can trap it */}
            <div
                aria-hidden="true"
                onClick={() => setOpen(false)}
                className={cn(
                    "fixed inset-0 bg-black/60 backdrop-blur-[2px] z-[9998] md:hidden",
                    "transition-opacity duration-300",
                    open ? "opacity-100 pointer-events-auto" : "opacity-0 pointer-events-none"
                )}
            />

            {/* Drawer panel */}
            <div
                role="dialog"
                aria-modal="true"
                aria-label="Navigation menu"
                className={cn(
                    "fixed inset-y-0 left-0 z-[9999] flex flex-col md:hidden",
                    "w-[min(280px,85vw)]",
                    // Solid opaque surface — var(--background) resolved to the full color token
                    "bg-[hsl(var(--background,0_0%_100%))]",
                    "[.dark_&]:bg-[#0d0f14] bg-[#f5f6f8]",
                    "border-r border-border shadow-2xl",
                    "transition-transform duration-300 ease-out",
                    open ? "translate-x-0" : "-translate-x-full"
                )}
            >
                {/* Header */}
                <div className="flex items-center justify-between px-4 h-14 border-b border-border shrink-0">
                    <div className="flex items-center gap-2">
                        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-gradient-to-br from-[#6366f1] to-[#4338ca] text-white text-xs font-black shrink-0">
                            A
                        </div>
                        <span className="gradient-text font-semibold text-sm">TrueFlow</span>
                    </div>
                    <button
                        aria-label="Close navigation"
                        onClick={() => setOpen(false)}
                        className="flex items-center justify-center h-10 w-10 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    >
                        <X className="h-4 w-4" />
                    </button>
                </div>

                {/* Project switcher */}
                <div className="px-3 pt-2 pb-2 border-b border-border shrink-0">
                    <ProjectSwitcher />
                </div>

                {/* Scrollable nav */}
                <nav className="flex-1 overflow-y-auto px-3 py-2">
                    {groups.map((group) => (
                        <div key={group.label}>
                            <p className="px-3 pt-4 pb-1 text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.08em]">
                                {group.label}
                            </p>
                            {group.routes.map((route) => {
                                const isActive = route.href === "/"
                                    ? pathname === "/"
                                    : pathname.startsWith(route.href);
                                return (
                                    <Link
                                        key={route.href}
                                        href={route.href}
                                        className={cn(
                                            "flex items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium transition-colors",
                                            isActive
                                                ? "bg-primary/10 text-primary"
                                                : "text-muted-foreground hover:bg-muted/80 hover:text-foreground"
                                        )}
                                    >
                                        <route.icon
                                            className={cn("h-4 w-4 shrink-0", isActive ? "text-primary" : "")}
                                            strokeWidth={isActive ? 2 : 1.5}
                                        />
                                        {route.label}
                                    </Link>
                                );
                            })}
                        </div>
                    ))}
                </nav>

                {/* Footer — pinned to bottom */}
                <div className="shrink-0 border-t border-border px-3 py-3 space-y-1">
                    <Link
                        href="/settings"
                        className={cn(
                            "flex items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium transition-colors",
                            pathname.startsWith("/settings")
                                ? "bg-primary/10 text-primary"
                                : "text-muted-foreground hover:bg-muted/80 hover:text-foreground"
                        )}
                    >
                        <Settings className={cn("h-4 w-4 shrink-0", pathname.startsWith("/settings") ? "text-primary" : "")} strokeWidth={1.5} />
                        Settings
                    </Link>

                    <div className="rounded-md border border-border/40 bg-muted/20 px-3 py-2.5">
                        <div className="flex items-center gap-2 mb-2">
                            <div className="flex h-6 w-6 items-center justify-center rounded-full bg-primary/10 shrink-0">
                                <User size={12} className="text-primary" />
                            </div>
                            <span className="text-xs font-medium text-foreground/70 truncate">Admin</span>
                        </div>
                        <button
                            onClick={() => window.location.href = "/login"}
                            className="w-full flex items-center gap-2 rounded-md px-2 py-2 text-xs font-medium text-muted-foreground hover:text-rose-400 hover:bg-rose-500/8 transition-colors"
                        >
                            <LogOut size={13} strokeWidth={1.5} />
                            Log out
                        </button>
                    </div>

                    <div className="flex items-center justify-between pt-1">
                        {mounted && (
                            <button
                                onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
                                className="flex items-center gap-2 rounded-md px-2 py-2 text-xs text-muted-foreground hover:bg-muted transition-colors"
                            >
                                {theme === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
                                {theme === "dark" ? "Light Mode" : "Dark Mode"}
                            </button>
                        )}
                        <span className="text-[10px] text-muted-foreground/50 font-mono pr-1">v0.8.0</span>
                    </div>
                </div>
            </div>
        </>,
        document.body
    ) : null;

    return (
        <>
            {/* Hamburger trigger — always in the DOM for the topbar */}
            <button
                aria-label="Open navigation menu"
                aria-expanded={open}
                aria-controls="mobile-nav-drawer"
                onClick={() => setOpen(true)}
                className="md:hidden flex items-center justify-center h-11 w-11 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors shrink-0"
            >
                <Menu className="h-5 w-5" />
            </button>

            {/* Portal-rendered drawer + backdrop */}
            {drawer}
        </>
    );
}
