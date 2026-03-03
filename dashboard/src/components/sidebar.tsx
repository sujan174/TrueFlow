"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import {
    Key,
    ShieldAlert,
    CheckCircle,
    ClipboardList,
    Fingerprint,
    BarChart3,
    LayoutDashboard,
    Activity,
    LockKeyhole,
    FlaskConical,
    Settings,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
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
    Eye,
    LogOut,
    User,
} from "lucide-react";
import { useEffect, useState, useCallback, useRef } from "react";

type SidebarProps = React.HTMLAttributes<HTMLDivElement>;

interface Route {
    href: string;
    label: string;
    icon: any;
    badge?: number | null;
}

interface Group {
    id: string;
    label: string;
    routes: Route[];
    defaultOpen?: boolean;
}

export function Sidebar({ className }: SidebarProps) {
    const pathname = usePathname();

    const [collapsed, setCollapsed] = useState(false);
    const [mounted, setMounted] = useState(false);
    const [health, setHealth] = useState<"online" | "offline" | "checking">("checking");
    const [approvalCount, setApprovalCount] = useState(0);
    const [openGroups, setOpenGroups] = useState<Record<string, boolean>>({
        home: true,
        agents: false,
        safety: false,
        prompts: false,
        observe: false,
        system: false,
    });

    const toggleGroup = useCallback((id: string) => {
        if (collapsed) return;
        setOpenGroups(prev => ({ ...prev, [id]: !prev[id] }));
    }, [collapsed]);

    useEffect(() => {
        setMounted(true);

        const checkHealth = async () => {
            try {
                await fetch("/api/proxy/healthz");
                setHealth("online");
            } catch {
                setHealth("offline");
            }
        };

        const checkApprovals = async () => {
            try {
                const res = await fetch("/api/proxy/approvals");
                if (res.ok) {
                    const data = await res.json();
                    setApprovalCount(data.filter((a: any) => a.status === "pending").length);
                }
            } catch (e) {
                console.error("Failed to fetch approvals", e);
            }
        };

        checkHealth();
        checkApprovals();

        const interval = setInterval(() => {
            checkHealth();
            checkApprovals();
        }, 15000);

        return () => clearInterval(interval);
    }, []);

    const groups: Group[] = [
        {
            id: "home",
            label: "Home",
            defaultOpen: true,
            routes: [
                { href: "/", label: "Dashboard", icon: LayoutDashboard },
                { href: "/analytics", label: "Analytics", icon: BarChart3 },
                { href: "/billing", label: "Billing", icon: CreditCard },
            ]
        },
        {
            id: "agents",
            label: "Agents & Keys",
            routes: [
                { href: "/virtual-keys", label: "Virtual Keys", icon: Key },
                { href: "/api-keys", label: "API Keys", icon: LockKeyhole },
                { href: "/upstreams", label: "Upstreams", icon: Activity },
                { href: "/vault", label: "Vault", icon: Fingerprint },
            ]
        },
        {
            id: "safety",
            label: "Safety & Policies",
            routes: [
                { href: "/policies", label: "Policies", icon: ScrollText },
                { href: "/guardrails", label: "Guardrails", icon: ShieldAlert },
                { href: "/model-access-groups", label: "Model Access", icon: ShieldCheck },
                { href: "/approvals", label: "Approvals", icon: CheckCircle, badge: approvalCount > 0 ? approvalCount : null },
            ]
        },
        {
            id: "prompts",
            label: "Prompts & Experiments",
            routes: [
                { href: "/prompts", label: "Prompts", icon: MessageSquareText },
                { href: "/playground", label: "Playground", icon: FlaskConical },
                { href: "/experiments", label: "Experiments", icon: FlaskRound },
            ]
        },
        {
            id: "observe",
            label: "Observe",
            routes: [
                { href: "/audit", label: "Audit Logs", icon: ClipboardList },
                { href: "/sessions", label: "Sessions", icon: Layers },
                { href: "/cache", label: "Cache", icon: Database },
            ]
        },
        {
            id: "system",
            label: "System",
            routes: [
                { href: "/tools", label: "Tools & MCP", icon: Wrench },
                { href: "/webhooks", label: "Webhooks", icon: Webhook },
                { href: "/config", label: "Config Export", icon: FileCode2 },
            ]
        }
    ];

    // Auto-open group containing current route
    useEffect(() => {
        groups.forEach((group) => {
            const hasActive = group.routes.some(r =>
                r.href === "/" ? pathname === "/" : pathname.startsWith(r.href)
            );
            if (hasActive) {
                setOpenGroups(prev => ({ ...prev, [group.id]: true }));
            }
        });
    }, [pathname]);

    return (
        <div
            style={{ width: collapsed ? 56 : 220, transition: "width 0.2s cubic-bezier(.4,0,.2,1)" }}
            className={cn(
                "flex h-full flex-col relative overflow-hidden",
                "bg-[var(--sidebar-bg)] border-r border-[var(--sidebar-border)]",
                className
            )}
        >
            {/* Collapse Toggle */}
            <button
                onClick={() => setCollapsed(!collapsed)}
                aria-label="Toggle Sidebar"
                className={cn(
                    "absolute -right-3 top-7 z-50",
                    "flex h-5 w-5 items-center justify-center rounded-full",
                    "border border-[var(--border)] bg-[var(--card)]",
                    "text-muted-foreground hover:text-foreground",
                    "transition-all duration-200",
                    "hover:border-[var(--primary)]/30"
                )}
            >
                {collapsed ? <ChevronRight size={10} /> : <ChevronLeft size={10} />}
            </button>

            {/* Logo */}
            <div className={cn(
                "flex h-12 shrink-0 items-center border-b border-[var(--sidebar-border)]",
                collapsed ? "justify-center px-3" : "px-4"
            )}>
                <Link href="/" className="flex items-center gap-2 group min-w-0">
                    <div className={cn(
                        "flex h-6 w-6 shrink-0 items-center justify-center rounded-md",
                        "bg-gradient-to-br from-[#8e2137] to-[#49111c]",
                        "text-white font-bold text-[10px] tracking-tight",
                        "group-hover:shadow-[0_0_12px_rgba(142,33,55,0.3)] transition-shadow"
                    )}>
                        A
                    </div>
                    {!collapsed && (
                        <span className="gradient-text font-semibold text-sm tracking-tight whitespace-nowrap">
                            AIlink
                        </span>
                    )}
                </Link>
            </div>

            {/* Navigation */}
            <div className="flex-1 overflow-y-auto overflow-x-hidden py-2 scrollbar-none px-2">
                {groups.map((group) => {
                    const isOpen = openGroups[group.id] ?? group.defaultOpen ?? false;

                    return (
                        <div key={group.id} className="mb-0.5">
                            {/* Group header — clickable to collapse */}
                            {!collapsed ? (
                                <button
                                    onClick={() => toggleGroup(group.id)}
                                    className={cn(
                                        "w-full flex items-center justify-between",
                                        "px-2 py-1.5 mt-2 first:mt-0",
                                        "text-[10px] font-medium uppercase tracking-[0.08em]",
                                        "text-muted-foreground/50 hover:text-muted-foreground",
                                        "transition-colors rounded-md"
                                    )}
                                >
                                    <span>{group.label}</span>
                                    <ChevronDown
                                        size={10}
                                        className={cn(
                                            "transition-transform duration-200",
                                            !isOpen && "-rotate-90"
                                        )}
                                    />
                                </button>
                            ) : (
                                <div className="h-px bg-[var(--border)] mx-2 my-2" />
                            )}

                            {/* Routes — CSS grid transition instead of framer-motion */}
                            <div
                                className="grid overflow-hidden transition-[grid-template-rows] duration-200 ease-in-out"
                                style={{
                                    gridTemplateRows: (isOpen || collapsed) ? "1fr" : "0fr",
                                }}
                            >
                                <div className="min-h-0 space-y-px">
                                    {group.routes.map((route) => {
                                        const isActive = route.href === "/"
                                            ? pathname === "/"
                                            : pathname.startsWith(route.href);
                                        return (
                                            <Link
                                                key={route.href}
                                                href={route.href}
                                                title={collapsed ? route.label : undefined}
                                                className={cn(
                                                    "relative flex items-center gap-2 rounded-md py-1.5 text-[13px] font-medium",
                                                    "transition-all duration-100 group",
                                                    collapsed ? "justify-center px-2" : "px-2",
                                                    isActive
                                                        ? "text-foreground bg-[var(--primary)]/8 border border-[var(--primary)]/12"
                                                        : "text-muted-foreground hover:text-foreground/80 hover:bg-[var(--card)] border border-transparent"
                                                )}
                                            >
                                                {/* Active left indicator */}
                                                {isActive && (
                                                    <span className="absolute left-0 top-1/2 -translate-y-1/2 h-3.5 w-[2px] rounded-r-full bg-[var(--primary)]" />
                                                )}
                                                <route.icon
                                                    size={14}
                                                    strokeWidth={isActive ? 2 : 1.5}
                                                    className={cn(
                                                        "shrink-0 transition-colors",
                                                        isActive ? "text-[var(--primary)]" : "text-muted-foreground group-hover:text-foreground/60"
                                                    )}
                                                />
                                                {!collapsed && (
                                                    <span className="flex-1 whitespace-nowrap overflow-hidden text-ellipsis">
                                                        {route.label}
                                                    </span>
                                                )}
                                                {/* Badge */}
                                                {!collapsed && route.badge && (
                                                    <span className={cn(
                                                        "flex h-4 min-w-[1rem] items-center justify-center rounded-full",
                                                        "bg-amber-500/12 px-1 text-[9px] font-bold text-amber-400 font-mono"
                                                    )}>
                                                        {route.badge}
                                                    </span>
                                                )}
                                            </Link>
                                        );
                                    })}
                                </div>
                            </div>
                        </div>
                    );
                })}
            </div>

            {/* Footer — Settings, Account, Health */}
            <div className={cn(
                "shrink-0 border-t border-[var(--sidebar-border)] py-2",
                collapsed ? "px-2" : "px-2"
            )}>
                {/* Settings */}
                <Link
                    href="/settings"
                    title={collapsed ? "Settings" : undefined}
                    className={cn(
                        "relative flex items-center gap-2 rounded-md py-1.5 text-[13px] font-medium",
                        "transition-all duration-100 group",
                        collapsed ? "justify-center px-2" : "px-2",
                        pathname.startsWith("/settings")
                            ? "text-foreground bg-[var(--primary)]/8 border border-[var(--primary)]/12"
                            : "text-muted-foreground hover:text-foreground/80 hover:bg-[var(--card)] border border-transparent"
                    )}
                >
                    {pathname.startsWith("/settings") && (
                        <span className="absolute left-0 top-1/2 -translate-y-1/2 h-3.5 w-[2px] rounded-r-full bg-[var(--primary)]" />
                    )}
                    <Settings
                        size={14}
                        strokeWidth={pathname.startsWith("/settings") ? 2 : 1.5}
                        className={cn(
                            "shrink-0 transition-colors",
                            pathname.startsWith("/settings") ? "text-[var(--primary)]" : "text-muted-foreground group-hover:text-foreground/60"
                        )}
                    />
                    {!collapsed && (
                        <span className="flex-1 whitespace-nowrap overflow-hidden text-ellipsis">
                            Settings
                        </span>
                    )}
                </Link>

                {/* Account */}
                {!collapsed ? (
                    <div className="mt-1.5 rounded-md border border-border/40 bg-muted/20 px-2.5 py-2">
                        <div className="flex items-center gap-2 mb-2">
                            <div className="flex h-6 w-6 items-center justify-center rounded-full bg-[var(--primary)]/10">
                                <User size={12} className="text-[var(--primary)]" />
                            </div>
                            <span className="text-[11px] font-medium text-foreground/70 truncate">Admin</span>
                        </div>
                        <button
                            onClick={() => window.location.href = "/login"}
                            className={cn(
                                "w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-[12px] font-medium",
                                "text-muted-foreground hover:text-rose-400 hover:bg-rose-500/8",
                                "transition-colors"
                            )}
                        >
                            <LogOut size={13} strokeWidth={1.5} />
                            Log out
                        </button>
                    </div>
                ) : (
                    <button
                        onClick={() => window.location.href = "/login"}
                        title="Log out"
                        className={cn(
                            "flex w-full items-center justify-center rounded-md px-2 py-1.5 mt-1",
                            "text-muted-foreground hover:text-rose-400 hover:bg-rose-500/8",
                            "transition-colors"
                        )}
                    >
                        <LogOut size={14} strokeWidth={1.5} />
                    </button>
                )}

                {/* Health + Version */}
                <div className={cn(
                    "flex items-center text-[11px] text-muted-foreground mt-2",
                    collapsed ? "justify-center" : "gap-2 px-2"
                )}>
                    <div className={cn(
                        "h-1.5 w-1.5 rounded-full transition-colors duration-500",
                        health === "online" ? "bg-emerald-500" :
                            health === "offline" ? "bg-rose-500" :
                                "bg-amber-500 animate-pulse"
                    )} />
                    {!collapsed && (
                        <span className="font-mono text-[10px] text-muted-foreground/60">v0.8.0</span>
                    )}
                </div>
            </div>
        </div>
    );
}
