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
    icon: React.ElementType;
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
    });

    const toggleGroup = useCallback((id: string) => {
        if (collapsed) return;
        setOpenGroups(prev => ({ ...prev, [id]: !prev[id] }));
    }, [collapsed]);

    useEffect(() => {
        let active = true;
        setTimeout(() => {
            if (active) setMounted(true);
        }, 0);

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
                    const data: Record<string, unknown>[] = await res.json();
                    setApprovalCount(data.filter(a => a.status === "pending").length);
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

        return () => {
            active = false;
            clearInterval(interval);
        };
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
            label: "Studio",
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
                { href: "/tools", label: "Tools & MCP", icon: Wrench },
                { href: "/webhooks", label: "Webhooks", icon: Webhook },
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
            style={{ width: collapsed ? 56 : 240, transition: "width 0.2s cubic-bezier(.4,0,.2,1)" }}
            className={cn(
                "flex h-full flex-col relative overflow-hidden",
                "bg-black border-r border-white/[0.06]",
                className
            )}
        >
            {/* Collapse Toggle */}
            <button
                onClick={() => setCollapsed(!collapsed)}
                aria-label="Toggle Sidebar"
                className={cn(
                    "absolute -right-3 top-7 z-50",
                    "hidden md:flex h-6 w-6 items-center justify-center rounded-full",
                    "border border-white/10 bg-black",
                    "text-zinc-500 hover:text-white hover:border-white/30",
                    "transition-all duration-200",
                )}
            >
                {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
            </button>

            {/* Logo */}
            <div className={cn(
                "flex h-12 shrink-0 items-center border-b border-white/[0.06]",
                collapsed ? "justify-center px-3" : "px-5"
            )}>
                <Link href="/" className="flex items-center gap-3 group min-w-0">
                    <div className={cn(
                        "flex h-6 w-6 shrink-0 items-center justify-center rounded-[4px]",
                        "bg-white text-black font-bold text-[11px] tracking-tight",
                        "transition-transform group-hover:scale-105"
                    )}>
                        TF
                    </div>
                    {!collapsed && (
                        <span className="text-white font-semibold text-sm tracking-tight whitespace-nowrap">
                            TrueFlow
                        </span>
                    )}
                </Link>
            </div>

            {/* Navigation */}
            <div className="flex-1 overflow-y-auto overflow-x-hidden py-4 scrollbar-none px-3">
                {groups.map((group) => {
                    const isOpen = openGroups[group.id] ?? group.defaultOpen ?? false;

                    return (
                        <div key={group.id} className="mb-2">
                            {/* Group header — clickable to collapse */}
                            {!collapsed ? (
                                <button
                                    onClick={() => toggleGroup(group.id)}
                                    className={cn(
                                        "w-full flex items-center justify-between",
                                        "px-2 py-1.5 mt-2 first:mt-0",
                                        "text-[10px] font-medium uppercase tracking-[0.1em]",
                                        "text-zinc-500 hover:text-zinc-300",
                                        "transition-colors rounded-md"
                                    )}
                                >
                                    <span>{group.label}</span>
                                    <ChevronDown
                                        size={12}
                                        className={cn(
                                            "transition-transform duration-200",
                                            !isOpen && "-rotate-90"
                                        )}
                                    />
                                </button>
                            ) : (
                                <div className="h-px bg-white/[0.06] mx-2 my-3" />
                            )}

                            {/* Routes */}
                            <div
                                className="grid overflow-hidden transition-[grid-template-rows] duration-200 ease-in-out"
                                style={{
                                    gridTemplateRows: (isOpen || collapsed) ? "1fr" : "0fr",
                                }}
                            >
                                <div className="min-h-0 space-y-[2px]">
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
                                                    "relative flex items-center gap-2.5 rounded-md py-1.5 text-[13px] font-medium",
                                                    "transition-all duration-100 group",
                                                    collapsed ? "justify-center px-3" : "px-2",
                                                    isActive
                                                        ? "text-white bg-white/10"
                                                        : "text-zinc-500 hover:text-white hover:bg-white/5"
                                                )}
                                            >
                                                <route.icon
                                                    size={16}
                                                    strokeWidth={isActive ? 2 : 1.5}
                                                    className={cn(
                                                        "shrink-0 transition-colors",
                                                        isActive ? "text-white" : "text-zinc-500 group-hover:text-zinc-300"
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
                                                        "flex h-4 min-w-[1rem] items-center justify-center rounded-sm",
                                                        "bg-white text-[9px] font-bold text-black font-mono px-1"
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
                "shrink-0 border-t border-white/[0.06] py-3",
                collapsed ? "px-3" : "px-3"
            )}>
                {/* Settings */}
                <Link
                    href="/settings"
                    title={collapsed ? "Settings" : undefined}
                    className={cn(
                        "relative flex items-center gap-2.5 rounded-md py-1.5 text-[13px] font-medium",
                        "transition-all duration-100 group",
                        collapsed ? "justify-center px-3" : "px-2",
                        pathname.startsWith("/settings")
                            ? "text-white bg-white/10"
                            : "text-zinc-400 hover:text-white hover:bg-white/5"
                    )}
                >
                    <Settings
                        size={16}
                        strokeWidth={pathname.startsWith("/settings") ? 2 : 1.5}
                        className={cn(
                            "shrink-0 transition-colors",
                            pathname.startsWith("/settings") ? "text-white" : "text-zinc-500 group-hover:text-zinc-300"
                        )}
                    />
                    {!collapsed && (
                        <span className="flex-1 whitespace-nowrap overflow-hidden text-ellipsis">
                            Settings
                        </span>
                    )}
                </Link>

                {/* Config Export */}
                <Link
                    href="/config"
                    title={collapsed ? "Config Export" : undefined}
                    className={cn(
                        "relative flex items-center gap-2.5 rounded-md py-1.5 text-[13px] font-medium",
                        "transition-all duration-100 group",
                        collapsed ? "justify-center px-3" : "px-2",
                        pathname.startsWith("/config")
                            ? "text-white bg-white/10"
                            : "text-zinc-400 hover:text-white hover:bg-white/5"
                    )}
                >
                    <FileCode2
                        size={16}
                        strokeWidth={pathname.startsWith("/config") ? 2 : 1.5}
                        className={cn(
                            "shrink-0 transition-colors",
                            pathname.startsWith("/config") ? "text-white" : "text-zinc-500 group-hover:text-zinc-300"
                        )}
                    />
                    {!collapsed && (
                        <span className="flex-1 whitespace-nowrap overflow-hidden text-ellipsis">
                            Config Export
                        </span>
                    )}
                </Link>

                {/* Account */}
                {!collapsed ? (
                    <div className="mt-3 rounded-md border border-white/[0.06] bg-white/[0.02] p-2">
                        <div className="flex items-center gap-2 mb-2 px-1">
                            <div className="flex h-5 w-5 items-center justify-center rounded-[4px] bg-white text-black">
                                <User size={12} strokeWidth={2.5} />
                            </div>
                            <span className="text-[12px] font-medium text-zinc-300 truncate">Admin</span>
                        </div>
                        <button
                            onClick={() => window.location.href = "/login"}
                            className={cn(
                                "w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-[12px] font-medium",
                                "text-zinc-400 hover:text-white hover:bg-white/5",
                                "transition-colors"
                            )}
                        >
                            <LogOut size={14} strokeWidth={1.5} />
                            Log out
                        </button>
                    </div>
                ) : (
                    <button
                        onClick={() => window.location.href = "/login"}
                        title="Log out"
                        className={cn(
                            "flex w-full items-center justify-center rounded-md px-2 py-2 mt-2",
                            "text-zinc-500 hover:text-white hover:bg-white/5",
                            "transition-colors"
                        )}
                    >
                        <LogOut size={16} strokeWidth={1.5} />
                    </button>
                )}

                {/* Health + Version */}
                <div className={cn(
                    "flex items-center mt-3",
                    collapsed ? "justify-center" : "justify-between px-2"
                )}>
                    <div className={cn(
                        "h-1.5 w-1.5 rounded-full transition-colors duration-500",
                        health === "online" ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]" :
                            health === "offline" ? "bg-rose-500 shadow-[0_0_8px_rgba(244,63,94,0.5)]" :
                                "bg-amber-500 animate-pulse shadow-[0_0_8px_rgba(245,158,11,0.5)]"
                    )} />
                    {!collapsed && (
                        <span className="font-mono text-[10px] text-zinc-600 tracking-widest uppercase">v0.8.0</span>
                    )}
                </div>
            </div>
        </div>
    );
}
