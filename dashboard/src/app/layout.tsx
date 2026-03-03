import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";
import { Sidebar } from "@/components/sidebar";
import { Toaster } from "@/components/ui/sonner";
import { ProjectProvider } from "@/contexts/project-context";
import { ThemeProvider } from "next-themes";
import dynamic from "next/dynamic";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
  display: "swap",
  weight: ["400", "500", "600"],
});

export const metadata: Metadata = {
  title: "AIlink Dashboard",
  description: "Manage tokens, approvals, and audit logs for the AIlink Gateway",
};

import { Breadcrumbs } from "@/components/breadcrumbs";
import { MobileNav } from "@/components/mobile-nav";

// Lazy-load non-critical header components (code-split, not needed for first paint)
const CommandPalette = dynamic(() => import("@/components/command-palette").then(m => m.CommandPalette), { loading: () => null });
const NotificationBell = dynamic(() => import("@/components/notification-bell").then(m => m.NotificationBell), { loading: () => null });
const ProjectSwitcher = dynamic(() => import("@/components/project-switcher").then(m => m.ProjectSwitcher), { loading: () => null });
const OnboardingModal = dynamic(() => import("@/components/onboarding-modal").then(m => m.OnboardingModal), { loading: () => null });

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`${inter.variable} ${jetbrainsMono.variable} flex h-screen w-full bg-background font-sans antialiased text-foreground`}>
        <ThemeProvider attribute="class" defaultTheme="dark" enableSystem disableTransitionOnChange>
          <ProjectProvider>
            <div className="flex h-full w-full">
              <Sidebar className="hidden border-r md:flex" />
              <main className="flex-1 flex flex-col overflow-hidden">
                <header className="flex h-11 shrink-0 items-center justify-between gap-3 border-b border-border bg-background/90 px-4 lg:px-5 backdrop-blur-sm">
                  <div className="flex items-center gap-2 flex-1">
                    <MobileNav />
                    <Breadcrumbs />
                  </div>
                  <div className="flex items-center gap-2.5">
                    <ProjectSwitcher className="w-[180px] h-7" />
                    <NotificationBell />
                  </div>
                </header>
                <div className="flex-1 overflow-y-auto p-4 lg:p-5">
                  <div className="container mx-auto max-w-[1440px] page-enter">
                    {children}
                  </div>
                </div>
              </main>
            </div>
            <Toaster />
            <CommandPalette />
            <OnboardingModal />
          </ProjectProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
