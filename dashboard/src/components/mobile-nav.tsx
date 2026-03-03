"use client";

import { useState, useEffect } from "react";
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
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogTrigger,
} from "@/components/ui/dialog";

export function MobileNav() {
    const [open, setOpen] = useState(false);
    const pathname = usePathname();
    const { theme, setTheme } = useTheme();
    const [mounted, setMounted] = useState(false);

    useEffect(() => {
        setMounted(true);
    }, []);

    // Close on navigation
    useEffect(() => {
        setOpen(false);
    }, [pathname]);

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
            label: "Prompts & Experiments",
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
            ]
        },
        {
            label: "System",
            routes: [
                { href: "/tools", label: "Tools & MCP", icon: Wrench },
                { href: "/webhooks", label: "Webhooks", icon: Webhook },
                { href: "/config", label: "Config Export", icon: FileCode2 },
            ]
        }
    ];

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger asChild>
                <Button variant="ghost" size="icon" className="md:hidden">
                    <Menu className="h-5 w-5" />
                    <span className="sr-only">Toggle menu</span>
                </Button>
            </DialogTrigger>
            <DialogContent className="fixed inset-y-0 left-0 z-50 h-full w-3/4 max-w-sm gap-4 border-r bg-background p-4 shadow-xl transition-transform animate-slide-right sm:max-w-xs overflow-y-auto">
                <div className="flex flex-col gap-3 h-full">
                    {/* Logo */}
                    <div className="flex items-center gap-2 font-bold text-lg">
                        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-gradient-to-br from-[#8e2137] to-[#49111c] text-white text-sm font-black">
                            A
                        </div>
                        <span className="gradient-text font-semibold">
                            AIlink
                        </span>
                    </div>

                    <ProjectSwitcher />

                    <nav className="flex flex-col gap-3 flex-1">
                        {groups.map((group) => (
                            <div key={group.label} className="flex flex-col gap-1">
                                <h4 className="px-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                                    {group.label}
                                </h4>
                                {group.routes.map((route) => {
                                    const isActive = route.href === "/"
                                        ? pathname === "/"
                                        : pathname.startsWith(route.href);
                                    return (
                                        <Link
                                            key={route.href}
                                            href={route.href}
                                            className={cn(
                                                "flex items-center gap-3 rounded-md px-2 py-2 text-sm font-medium transition-colors",
                                                isActive
                                                    ? "bg-primary/10 text-primary"
                                                    : "hover:bg-muted text-muted-foreground hover:text-foreground"
                                            )}
                                        >
                                            <route.icon className={cn("h-4 w-4", isActive && "text-primary")} />
                                            {route.label}
                                        </Link>
                                    );
                                })}
                            </div>
                        ))}
                    </nav>

                    {/* Footer */}
                    <div className="mt-auto border-t pt-4 space-y-2">
                        <Link
                            href="/settings"
                            className={cn(
                                "flex items-center gap-3 rounded-md px-2 py-2 text-sm font-medium transition-colors",
                                pathname.startsWith("/settings")
                                    ? "bg-primary/10 text-primary"
                                    : "hover:bg-muted text-muted-foreground hover:text-foreground"
                            )}
                        >
                            <Settings className={cn("h-4 w-4", pathname.startsWith("/settings") && "text-primary")} />
                            Settings
                        </Link>

                        <div className="rounded-md border border-border/40 bg-muted/20 px-3 py-2.5">
                            <div className="flex items-center gap-2 mb-2">
                                <div className="flex h-6 w-6 items-center justify-center rounded-full bg-primary/10">
                                    <User size={12} className="text-primary" />
                                </div>
                                <span className="text-xs font-medium text-foreground/70">Admin</span>
                            </div>
                            <button
                                onClick={() => window.location.href = "/login"}
                                className="w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-[13px] font-medium text-muted-foreground hover:text-rose-400 hover:bg-rose-500/8 transition-colors"
                            >
                                <LogOut size={14} strokeWidth={1.5} />
                                Log out
                            </button>
                        </div>

                        {mounted && (
                            <button
                                onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
                                className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-[13px] text-muted-foreground hover:bg-muted"
                            >
                                {theme === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
                                {theme === "dark" ? "Light Mode" : "Dark Mode"}
                            </button>
                        )}
                        <div className="px-3 text-xs text-muted-foreground">
                            v0.8.0
                        </div>
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
