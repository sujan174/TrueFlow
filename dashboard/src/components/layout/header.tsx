"use client"

import { useTheme } from "next-themes"
import { useEffect, useState } from "react"
import { useRouter, usePathname } from "next/navigation"
import { createClient } from "@/lib/supabase/client"
import {
  LogOut,
  User as UserIcon,
  Sun,
  Moon,
  Search,
  ChevronRight,
  Home,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { ProjectDropdown } from "@/components/layout/project-dropdown"
import { cn } from "@/lib/utils"
import type { User } from "@supabase/supabase-js"

interface HeaderProps {
  user: User | null
}

// Page title mapping
const pageTitles: Record<string, string> = {
  "/": "Dashboard",
  "/analytics": "Analytics",
  "/virtual-keys": "Virtual Keys",
  "/policies": "Policies",
  "/audit": "Audit Logs",
  "/settings/team": "Team Settings",
  "/settings/sso": "SSO Settings",
  "/settings/billing": "Billing",
}

export function Header({ user }: HeaderProps) {
  const router = useRouter()
  const pathname = usePathname()
  const supabase = createClient()
  const { theme, setTheme } = useTheme()
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  const handleSignOut = async () => {
    // Clear project selection from localStorage to prevent stale data
    localStorage.removeItem("trueflow_project_id")
    await supabase.auth.signOut()
    router.push("/login")
    router.refresh()
  }

  // Generate breadcrumbs
  const breadcrumbs = pathname
    .split("/")
    .filter(Boolean)
    .map((segment, index, arr) => {
      const href = "/" + arr.slice(0, index + 1).join("/")
      const title = pageTitles[href] || segment.charAt(0).toUpperCase() + segment.slice(1)
      return { href, title }
    })

  const pageTitle = pageTitles[pathname] || breadcrumbs[breadcrumbs.length - 1]?.title || "Dashboard"

  return (
    <header className="h-14 bg-background/80 backdrop-blur-sm border-b border-border flex items-center justify-between px-6 sticky top-0 z-30">
      {/* Left: Breadcrumbs */}
      <div className="flex items-center gap-2">
        <nav className="flex items-center text-sm">
          <a
            href="/"
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <Home className="h-4 w-4" />
          </a>
          {breadcrumbs.map((crumb, index) => (
            <div key={crumb.href} className="flex items-center">
              <ChevronRight className="h-4 w-4 mx-1.5 text-muted-foreground/50" />
              {index === breadcrumbs.length - 1 ? (
                <span className="font-medium text-foreground">{crumb.title}</span>
              ) : (
                <a
                  href={crumb.href}
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  {crumb.title}
                </a>
              )}
            </div>
          ))}
        </nav>
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-3">
        {/* Project Dropdown */}
        <ProjectDropdown />

        {/* Search Button (Cmd+K placeholder) */}
        <Button
          variant="outline"
          size="sm"
          className="hidden md:flex gap-2 text-muted-foreground w-48 justify-start"
          onClick={() => {
            // TODO: Implement command palette
          }}
        >
          <Search className="h-4 w-4" />
          <span className="text-xs">Search...</span>
          <kbd className="pointer-events-none ml-auto text-[10px] bg-muted px-1.5 py-0.5 rounded font-mono">
            ⌘K
          </kbd>
        </Button>

        {/* Theme Toggle */}
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          title="Toggle theme"
        >
          {mounted ? (
            theme === "dark" ? (
              <Sun className="h-4 w-4" />
            ) : (
              <Moon className="h-4 w-4" />
            )
          ) : (
            <div className="h-4 w-4" />
          )}
        </Button>

        {/* User Dropdown */}
        {user && (
          <DropdownMenu>
            <DropdownMenuTrigger className="relative rounded-md p-1.5 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              <Avatar className="h-7 w-7">
                <AvatarImage
                  src={user.user_metadata?.avatar_url}
                  alt={user.email || ""}
                />
                <AvatarFallback className="bg-primary/10 text-primary text-xs font-medium">
                  {user.email?.charAt(0).toUpperCase() || "U"}
                </AvatarFallback>
              </Avatar>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              <DropdownMenuLabel className="font-normal">
                <div className="flex flex-col space-y-1">
                  <p className="text-sm font-medium">{user.user_metadata?.name || "User"}</p>
                  <p className="text-xs text-muted-foreground">{user.email}</p>
                </div>
              </DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => router.push("/settings/team")}>
                <UserIcon className="mr-2 h-4 w-4" />
                Profile
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={handleSignOut}
                className="text-destructive focus:text-destructive"
              >
                <LogOut className="mr-2 h-4 w-4" />
                Sign out
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </header>
  )
}