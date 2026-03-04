import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";
import { Toaster } from "@/components/ui/sonner";
import { ProjectProvider } from "@/contexts/project-context";
import { ThemeProvider } from "next-themes";
import { AppShell } from "@/components/app-shell";

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
  title: "TrueFlow Dashboard",
  description: "Manage tokens, approvals, and audit logs for the TrueFlow Gateway",
};

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
            <AppShell>{children}</AppShell>
            <Toaster />
          </ProjectProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
