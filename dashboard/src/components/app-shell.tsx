"use client";

import { usePathname } from "next/navigation";
import { Sidebar } from "@/components/sidebar";
import { Breadcrumbs } from "@/components/breadcrumbs";
import { MobileNav } from "@/components/mobile-nav";
import dynamic from "next/dynamic";

const CommandPalette = dynamic(() => import("@/components/command-palette").then(m => m.CommandPalette), { loading: () => null });
const NotificationBell = dynamic(() => import("@/components/notification-bell").then(m => m.NotificationBell), { loading: () => null });
const ProjectSwitcher = dynamic(() => import("@/components/project-switcher").then(m => m.ProjectSwitcher), { loading: () => null });
const OnboardingModal = dynamic(() => import("@/components/onboarding-modal").then(m => m.OnboardingModal), { loading: () => null });

/**
 * Renders the full app chrome (sidebar + topbar) on authenticated pages,
 * or a bare canvas on /login.
 */
export function AppShell({ children }: { children: React.ReactNode }) {
    const pathname = usePathname();
    const isLogin = pathname === "/login";

    if (isLogin) {
        return <>{children}</>;
    }

    return (
        <>
            <div className="flex h-full w-full bg-black">
                <Sidebar className="hidden border-r border-white/10 md:flex bg-black" />
                <main className="flex-1 flex flex-col overflow-hidden bg-black">
                    <header className="flex h-12 shrink-0 items-center justify-between gap-2 border-b border-white/[0.06] bg-black/90 px-5 backdrop-blur-md sticky top-0 z-50">
                        <div className="flex items-center gap-3 flex-1">
                            <MobileNav />
                            <Breadcrumbs />
                        </div>
                        <div className="flex items-center gap-3">
                            <ProjectSwitcher className="hidden md:flex w-[200px] h-8 bg-transparent border-white/10 hover:border-white/20 text-[13px]" />
                            <NotificationBell />
                        </div>
                    </header>
                    <div className="flex-1 overflow-y-auto p-5 pb-20 md:p-8 md:pb-8">
                        <div className="container mx-auto max-w-[1440px] page-enter">
                            {children}
                        </div>
                    </div>
                </main>
            </div>
            <CommandPalette />
            <OnboardingModal />
        </>
    );
}
