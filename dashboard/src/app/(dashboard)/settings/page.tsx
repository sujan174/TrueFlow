"use client"

import { useState } from "react"
import Link from "next/link"
import {
  Settings2,
  DollarSign,
  Webhook,
  FileCode,
  Bell,
  FolderOpen,
  Users,
  Key,
  Building2,
  Globe,
  ChevronRight,
} from "lucide-react"
import { GatewaySettingsTab } from "./_components/gateway-settings-tab"
import { PricingTab } from "./_components/pricing-tab"
import { WebhooksTab } from "./_components/webhooks-tab"
import { ConfigTab } from "./_components/config-tab"
import { NotificationsTab } from "./_components/notifications-tab"
import { SettingsSidebar } from "./_components/settings-sidebar"
import { cn } from "@/lib/utils"

const organizationLinks = [
  {
    label: "Projects",
    description: "Multi-tenant isolation",
    href: "/settings/projects",
    icon: FolderOpen,
  },
  {
    label: "Teams",
    description: "Budget & access groups",
    href: "/settings/teams",
    icon: Users,
  },
  {
    label: "Access Control",
    description: "API keys & permissions",
    href: "/settings/access",
    icon: Key,
  },
]

const gatewayTabs = [
  { value: "gateway", label: "Gateway", icon: Settings2 },
  { value: "pricing", label: "Pricing", icon: DollarSign },
  { value: "webhooks", label: "Webhooks", icon: Webhook },
  { value: "config", label: "Config", icon: FileCode },
  { value: "notifications", label: "Notifications", icon: Bell },
]

export default function SettingsPage() {
  return (
    <div className="flex-1 flex min-w-0">
      <SettingsSidebar />

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-w-0 overflow-auto">
        <div className="flex-1 p-6 lg:p-8">
          {/* Header */}
          <header className="mb-8">
            <h1 className="text-xl font-semibold tracking-tight">Settings</h1>
            <p className="text-sm text-muted-foreground mt-1">
              Manage your organization and gateway configuration
            </p>
          </header>

          {/* Organization Cards - Mobile/Tablet View */}
          <section className="mb-10 lg:hidden">
            <div className="flex items-center gap-2 mb-4">
              <Building2 className="h-4 w-4 text-muted-foreground" />
              <h2 className="text-sm font-medium">Organization</h2>
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              {organizationLinks.map((item) => (
                <Link
                  key={item.href}
                  href={item.href}
                  className="group flex items-center gap-4 p-4 border rounded-lg hover:border-foreground/20 transition-colors"
                >
                  <div className="p-2 bg-muted rounded-md">
                    <item.icon className="h-4 w-4 text-muted-foreground" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium">{item.label}</div>
                    <div className="text-xs text-muted-foreground">{item.description}</div>
                  </div>
                  <ChevronRight className="h-4 w-4 text-muted-foreground group-hover:text-foreground transition-colors" />
                </Link>
              ))}
            </div>
          </section>

          {/* Gateway Configuration */}
          <section>
            <div className="flex items-center gap-2 mb-6">
              <Globe className="h-4 w-4 text-muted-foreground" />
              <h2 className="text-sm font-medium">Gateway Configuration</h2>
            </div>

            {/* Tabs with URL sync */}
            <TabsWithContent />
          </section>
        </div>
      </div>
    </div>
  )
}

function TabsWithContent() {
  const [activeTab, setActiveTab] = useState("gateway")

  return (
    <div>
      {/* Tab Navigation */}
      <div className="border-b mb-6">
        <nav className="flex gap-1 -mb-px overflow-x-auto" role="tablist">
          {gatewayTabs.map((tab) => (
            <button
              key={tab.value}
              role="tab"
              aria-selected={activeTab === tab.value}
              onClick={() => setActiveTab(tab.value)}
              className={cn(
                "flex items-center gap-2 px-4 py-2.5 text-sm font-medium border-b-2 transition-colors whitespace-nowrap",
                activeTab === tab.value
                  ? "border-foreground text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground hover:border-muted-foreground/30"
              )}
            >
              <tab.icon className="h-4 w-4" />
              <span>{tab.label}</span>
            </button>
          ))}
        </nav>
      </div>

      {/* Tab Content */}
      <div role="tabpanel">
        {activeTab === "gateway" && <GatewaySettingsTab />}
        {activeTab === "pricing" && <PricingTab />}
        {activeTab === "webhooks" && <WebhooksTab />}
        {activeTab === "config" && <ConfigTab />}
        {activeTab === "notifications" && <NotificationsTab />}
      </div>
    </div>
  )
}