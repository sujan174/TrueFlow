"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"
import {
  Building2,
  Globe,
  User,
  FolderOpen,
  Users,
  Key,
  Settings2,
  Menu,
  X,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { useState, useEffect } from "react"
import { Button } from "@/components/ui/button"

interface NavItem {
  label: string
  href: string
  icon: React.ElementType
}

interface NavSection {
  title: string
  icon: React.ElementType
  items: NavItem[]
}

const sections: NavSection[] = [
  {
    title: "Organization",
    icon: Building2,
    items: [
      { label: "Projects", href: "/settings/projects", icon: FolderOpen },
      { label: "Teams", href: "/settings/teams", icon: Users },
      { label: "Access Control", href: "/settings/access", icon: Key },
    ],
  },
  {
    title: "Gateway",
    icon: Globe,
    items: [
      { label: "Configuration", href: "/settings", icon: Settings2 },
    ],
  },
  {
    title: "Personal",
    icon: User,
    items: [
      { label: "Account", href: "/settings/account", icon: User },
    ],
  },
]

export function SettingsSidebar() {
  const pathname = usePathname()
  const [mobileOpen, setMobileOpen] = useState(false)

  // Close mobile menu on route change
  useEffect(() => {
    setMobileOpen(false)
  }, [pathname])

  // Prevent body scroll when mobile menu is open
  useEffect(() => {
    if (mobileOpen) {
      document.body.style.overflow = "hidden"
    } else {
      document.body.style.overflow = ""
    }
    return () => {
      document.body.style.overflow = ""
    }
  }, [mobileOpen])

  const isActive = (href: string) => {
    if (href === "/settings") {
      return pathname === "/settings"
    }
    return pathname.startsWith(href)
  }

  return (
    <>
      {/* Mobile Menu Button */}
      <button
        onClick={() => setMobileOpen(true)}
        className="lg:hidden fixed top-4 left-4 z-40 p-2 rounded-lg bg-background border shadow-sm"
        aria-label="Open navigation menu"
      >
        <Menu className="h-5 w-5" />
      </button>

      {/* Mobile Overlay */}
      {mobileOpen && (
        <div
          className="lg:hidden fixed inset-0 bg-background/80 backdrop-blur-sm z-40"
          onClick={() => setMobileOpen(false)}
        />
      )}

      {/* Mobile Drawer */}
      <aside
        className={cn(
          "lg:hidden fixed inset-y-0 left-0 z-50 w-72 bg-background border-r transform transition-transform duration-300 ease-in-out",
          mobileOpen ? "translate-x-0" : "-translate-x-full"
        )}
      >
        <div className="flex items-center justify-between p-4 border-b">
          <span className="text-sm font-semibold">Settings</span>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => setMobileOpen(false)}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="h-full overflow-auto p-4 pb-24">
          <SidebarContent pathname={pathname} isActive={isActive} />
        </div>
      </aside>

      {/* Desktop Sidebar */}
      <aside className="w-64 shrink-0 border-r bg-muted/30 hidden lg:block">
        <div className="sticky top-0 h-full overflow-auto p-4">
          <SidebarContent pathname={pathname} isActive={isActive} />
        </div>
      </aside>
    </>
  )
}

function SidebarContent({
  pathname,
  isActive,
}: {
  pathname: string
  isActive: (href: string) => boolean
}) {
  return (
    <>
      {sections.map((section, sectionIndex) => (
        <div
          key={section.title}
          className={cn("mb-6", sectionIndex === sections.length - 1 && "mb-0")}
        >
          <div className="flex items-center gap-2 px-3 mb-2">
            <section.icon className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {section.title}
            </span>
          </div>
          <nav className="space-y-0.5">
            {section.items.map((item) => {
              const active = isActive(item.href)
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "flex items-center gap-3 px-3 py-2.5 rounded-md transition-colors",
                    active
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  )}
                >
                  <item.icon className="h-4 w-4 shrink-0" />
                  <span className="text-sm font-medium">{item.label}</span>
                </Link>
              )
            })}
          </nav>
        </div>
      ))}
    </>
  )
}