"use client"

import { useEffect, useState } from "react"
import { getHitlSummary, type HitlSummaryStats, formatNumber } from "@/lib/api"

export function HitlKpiRibbon() {
  const [data, setData] = useState<HitlSummaryStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const summary = await getHitlSummary(168) // 7 days
        setData(summary)
      } catch (error) {
        console.error("Failed to fetch HITL summary:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-[72px] bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading HITL stats...</div>
      </div>
    )
  }

  const pendingCount = data?.pending_count ?? 0
  const avgWaitSeconds = data?.avg_wait_seconds ?? 0
  const approvalRate = data?.approval_rate ?? 0

  // Format wait time
  const formatWaitTime = (seconds: number): string => {
    if (seconds < 60) {
      return `${Math.round(seconds)}s`
    } else if (seconds < 3600) {
      return `${Math.round(seconds / 60)}m`
    } else {
      return `${(seconds / 3600).toFixed(1)}h`
    }
  }

  return (
    <div className="h-[72px] bg-card border border rounded-[14px] flex items-center px-6 gap-8 shadow-sm">
      {/* Pending Approvals - Red value */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Pending Approvals
        </span>
        <span className="text-[20px] font-semibold text-destructive">
          {formatNumber(pendingCount)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Avg Wait Time */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Avg Wait Time
        </span>
        <span className="text-[20px] font-semibold text-foreground">
          {formatWaitTime(avgWaitSeconds)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Approval Rate - Green value */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Approval Rate
        </span>
        <span className="text-[20px] font-semibold text-success">
          {approvalRate.toFixed(1)}%
        </span>
      </div>
    </div>
  )
}