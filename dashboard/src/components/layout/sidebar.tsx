"use client"

import { useState, useEffect, useCallback } from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import {
  LayoutDashboard,
  BarChart3,
  Key,
  Shield,
  FileCheck,
  Filter,
  ChevronLeft,
  ChevronRight,
  Menu,
  X,
  Wrench,
  ClipboardCheck,
  FileText,
  Settings2,
  FileCode,
  FlaskConical,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"

const navigation = [
  { name: "Dashboard", href: "/", icon: LayoutDashboard },
  { name: "Analytics", href: "/analytics", icon: BarChart3 },
  { name: "Tokens", href: "/tokens", icon: Key },
  { name: "Credentials", href: "/credentials", icon: Shield },
  { name: "Policies", href: "/policies", icon: FileCheck },
  { name: "Prompts", href: "/prompts", icon: FileCode },
  { name: "Experiments", href: "/experiments", icon: FlaskConical },
  { name: "Guardrails", href: "/guardrails", icon: Filter },
  { name: "Approvals", href: "/approvals", icon: ClipboardCheck },
  { name: "Request Log", href: "/traces", icon: FileText },
  { name: "MCP Servers", href: "/mcp/servers", icon: Wrench },
]

const settingsNavigation = [
  { name: "Settings", href: "/settings", icon: Settings2 },
]

export function Sidebar() {
  const pathname = usePathname()
  const [collapsed, setCollapsed] = useState(false)
  const [mobileOpen, setMobileOpen] = useState(false)

  // Keyboard shortcut: Cmd/Ctrl + B to toggle sidebar
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "b") {
        e.preventDefault()
        setCollapsed((prev) => !prev)
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [])

  // Close mobile drawer on route change
  useEffect(() => {
    setMobileOpen(false)
  }, [pathname])

  const isActive = useCallback(
    (href: string) => {
      if (href === "/") {
        return pathname === "/"
      }
      return pathname.startsWith(href)
    },
    [pathname]
  )

  const NavItem = ({ item, collapsed }: { item: typeof navigation[0]; collapsed: boolean }) => {
    const active = isActive(item.href)
    return (
      <Link
        href={item.href}
        className={cn(
          "flex items-center gap-2.5 h-9 px-3 text-[13px] font-medium transition-all rounded-lg",
          active
            ? "bg-primary/10 text-primary border-l-2 border-primary rounded-none"
            : "text-muted-foreground hover:bg-accent hover:text-foreground",
          collapsed && "justify-center px-0"
        )}
        title={collapsed ? item.name : undefined}
      >
        <item.icon
          className={cn(
            "w-4 h-4 shrink-0",
            active ? "text-primary" : "text-muted-foreground"
          )}
        />
        {!collapsed && <span>{item.name}</span>}
      </Link>
    )
  }

  return (
    <>
      {/* Mobile Menu Button */}
      <Button
        variant="ghost"
        size="icon"
        className="fixed top-4 left-4 z-50 lg:hidden"
        onClick={() => setMobileOpen(true)}
      >
        <Menu className="h-5 w-5" />
      </Button>

      {/* Mobile Drawer Overlay */}
      {mobileOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-40 lg:hidden"
          onClick={() => setMobileOpen(false)}
        />
      )}

      {/* Mobile Drawer */}
      <aside
        className={cn(
          "fixed inset-y-0 left-0 z-50 w-[240px] bg-sidebar transform transition-transform duration-300 ease-out lg:hidden",
          mobileOpen ? "translate-x-0" : "-translate-x-full"
        )}
      >
        {/* Mobile Header */}
        <div className="h-[56px] flex items-center justify-between px-5 border-b border-sidebar-border">
          <div className="flex items-center gap-3">
            <div className="w-[28px] h-[28px] rounded-[6px] bg-primary" />
            <span className="text-sidebar-foreground font-semibold text-[14px]">
              TrueFlow
            </span>
          </div>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => setMobileOpen(false)}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>

        {/* Mobile Navigation */}
        <nav className="flex-1 p-[14px] space-y-1 overflow-y-auto">
          {navigation.map((item) => (
            <NavItem key={item.name} item={item} collapsed={false} />
          ))}
        </nav>

        {/* Mobile Settings Section */}
        <div className="p-[14px] pt-0 border-t border-sidebar-border">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase px-3 mb-2 block">
            Settings
          </span>
          <div className="space-y-1">
            {settingsNavigation.map((item) => (
              <NavItem key={item.name} item={item} collapsed={false} />
            ))}
          </div>
        </div>

        </aside>

      {/* Desktop Sidebar */}
      <aside
        className={cn(
          "hidden lg:flex flex-col h-screen bg-sidebar border-r border-sidebar-border sidebar-transition",
          collapsed ? "w-[64px]" : "w-[240px]"
        )}
      >
        {/* Logo */}
        <div
          className={cn(
            "h-[56px] flex items-center border-b border-sidebar-border",
            collapsed ? "justify-center px-0" : "px-5 gap-3"
          )}
        >
          <div className="w-[28px] h-[28px] rounded-[6px] bg-primary shrink-0" />
          {!collapsed && (
            <span className="text-sidebar-foreground font-semibold text-[14px]">
              TrueFlow
            </span>
          )}
        </div>

        {/* Main Navigation */}
        <nav className="flex-1 p-[14px] space-y-1 overflow-y-auto scrollbar-thin">
          {navigation.map((item) => (
            <NavItem key={item.name} item={item} collapsed={collapsed} />
          ))}
        </nav>

        {/* Settings Section - at bottom */}
        <div className="p-[14px] pt-0 border-t border-sidebar-border">
          {!collapsed && (
            <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase px-3 mb-2 block">
              Settings
            </span>
          )}
          <div className="space-y-1">
            {settingsNavigation.map((item) => (
              <NavItem key={item.name} item={item} collapsed={collapsed} />
            ))}
          </div>
        </div>

        {/* Collapse Toggle - subtle button at bottom */}
        <div className="py-2 border-t border-sidebar-border">
          <button
            onClick={() => setCollapsed(!collapsed)}
            className={cn(
              "w-full flex items-center h-8 text-muted-foreground hover:text-foreground transition-colors",
              collapsed ? "justify-center" : "justify-end px-4"
            )}
            title={collapsed ? "Expand sidebar (⌘B)" : "Collapse sidebar (⌘B)"}
          >
            {collapsed ? (
              <ChevronRight className="h-4 w-4" />
            ) : (
              <ChevronLeft className="h-4 w-4" />
            )}
          </button>
        </div>
      </aside>
    </>
  )
}