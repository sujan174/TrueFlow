"use client"

import { useEffect, useState } from "react"
import { getEngagementTiers, type EngagementTiersResponse, formatNumber } from "@/lib/api"

export function EngagementTiersCard() {
  const [data, setData] = useState<EngagementTiersResponse | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const tiers = await getEngagementTiers(720) // 30 days
        setData(tiers)
      } catch (error) {
        console.error("Failed to fetch engagement tiers:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const total = data?.total_users ?? 0
  const powerPercent = total > 0 ? (data?.power_users ?? 0) / total * 100 : 0
  const regularPercent = total > 0 ? (data?.regular_users ?? 0) / total * 100 : 0
  const lightPercent = total > 0 ? (data?.light_users ?? 0) / total * 100 : 0

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <div className="flex flex-col gap-0.5">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            Engagement Tiers
          </span>
          <span className="text-[12px] font-bold text-foreground">
            {formatNumber(total)} users
          </span>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col justify-center gap-4">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : total === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No users in this time period</span>
          </div>
        ) : (
          <>
            {/* Power Users */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-primary" />
                  <span className="text-[11px] font-medium text-foreground">Power Users</span>
                </div>
                <span className="text-[11px] text-muted-foreground">
                  {formatNumber(data?.power_users ?? 0)} ({powerPercent.toFixed(0)}%)
                </span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-300"
                  style={{ width: `${powerPercent}%` }}
                />
              </div>
              <span className="text-[9px] text-muted-foreground">100+ requests</span>
            </div>

            {/* Regular Users */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-destructive" />
                  <span className="text-[11px] font-medium text-foreground">Regular Users</span>
                </div>
                <span className="text-[11px] text-muted-foreground">
                  {formatNumber(data?.regular_users ?? 0)} ({regularPercent.toFixed(0)}%)
                </span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-destructive rounded-full transition-all duration-300"
                  style={{ width: `${regularPercent}%` }}
                />
              </div>
              <span className="text-[9px] text-muted-foreground">10-100 requests</span>
            </div>

            {/* Light Users */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-info" />
                  <span className="text-[11px] font-medium text-foreground">Light Users</span>
                </div>
                <span className="text-[11px] text-muted-foreground">
                  {formatNumber(data?.light_users ?? 0)} ({lightPercent.toFixed(0)}%)
                </span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-info rounded-full transition-all duration-300"
                  style={{ width: `${lightPercent}%` }}
                />
              </div>
              <span className="text-[9px] text-muted-foreground">1-9 requests</span>
            </div>
          </>
        )}
      </div>
    </div>
  )
}