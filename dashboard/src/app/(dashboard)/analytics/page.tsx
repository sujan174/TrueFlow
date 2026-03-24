"use client"

import { useSearchParams, useRouter } from "next/navigation"
import { TabBar } from "@/components/analytics/tab-bar"
import { RequestVolumeChart } from "@/components/analytics/request-volume-chart"
import { KPICards } from "@/components/analytics/kpi-cards"
import { GatewayHealthCard } from "@/components/analytics/gateway-health-card"
import { ModelUsageCard } from "@/components/analytics/model-usage-card"
import { SpendByProviderCard } from "@/components/analytics/spend-by-provider-card"
import { LatencyByProviderCard } from "@/components/analytics/latency-by-provider-card"
import { TrafficTabContent } from "@/components/analytics/traffic"
import { CostTabContent } from "@/components/analytics/cost"
import { UsersTokensTabContent } from "@/components/analytics/users-tokens"
import { CacheTabContent } from "@/components/analytics/cache"
import { ModelsTabContent } from "@/components/analytics/models"
import { SecurityTabContent } from "@/components/analytics/security"
import { HitlTabContent } from "@/components/analytics/hitl"
import { ErrorsTabContent } from "@/components/analytics/errors"
import { RefreshCw, Download, Radio } from "lucide-react"
import { Button } from "@/components/ui/button"
import { StatusBadge } from "@/components/ui/status-badge"

export default function AnalyticsPage() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const activeTab = searchParams.get("tab") || "Overview"

  const handleTabChange = (tabId: string) => {
    const params = new URLSearchParams(searchParams.toString())
    params.set("tab", tabId)
    router.push(`/analytics?${params.toString()}`)
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header Row */}
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Analytics
            </h1>
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <RefreshCw className="h-3 w-3" />
              <span>Last refreshed 30s ago</span>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <StatusBadge variant="success" pulse className="gap-1.5">
              <Radio className="h-3 w-3" />
              LIVE
            </StatusBadge>
            <Button variant="outline" size="sm" className="gap-2">
              <Download className="h-4 w-4" />
              Export
            </Button>
          </div>
        </div>

        {/* Tab Bar */}
        <TabBar activeTab={activeTab} onTabChange={handleTabChange} />

        {/* Tab Content */}
        {activeTab === "Overview" && (
          <>
            {/* Charts Row */}
            <div className="flex flex-col lg:flex-row gap-4 min-h-[320px]">
              {/* Request Volume Chart */}
              <div className="flex-1 min-h-[280px]">
                <RequestVolumeChart />
              </div>

              {/* KPI Column */}
              <div className="w-full lg:w-[300px] flex flex-row lg:flex-col gap-4">
                <KPICards />
              </div>
            </div>

            {/* Bottom Row */}
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
              <GatewayHealthCard />
              <ModelUsageCard />
              <SpendByProviderCard />
              <LatencyByProviderCard />
            </div>
          </>
        )}

        {activeTab === "Traffic" && (
          <TrafficTabContent />
        )}

        {activeTab === "Cost" && (
          <CostTabContent />
        )}

        {activeTab === "UsersTokens" && (
          <UsersTokensTabContent />
        )}

        {activeTab === "Cache" && (
          <CacheTabContent />
        )}

        {activeTab === "Models" && (
          <ModelsTabContent />
        )}

        {activeTab === "Security" && (
          <SecurityTabContent />
        )}

        {activeTab === "HITL" && (
          <HitlTabContent />
        )}

        {activeTab === "Errors" && (
          <ErrorsTabContent />
        )}

        {/* Placeholder for other tabs */}
        {!["Overview", "Traffic", "Cost", "UsersTokens", "Cache", "Models", "Security", "HITL", "Errors"].includes(activeTab) && (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              {activeTab} tab coming soon
            </span>
          </div>
        )}
      </div>
    </div>
  )
}