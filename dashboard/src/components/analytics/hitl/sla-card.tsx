"use client"

import { useEffect, useState } from "react"
import { getHitlLatency, type HitlLatencyStats } from "@/lib/api"

export function SlaCard() {
  const [data, setData] = useState<HitlLatencyStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const stats = await getHitlLatency(168) // 7 days
        setData(stats)
      } catch (error) {
        console.error("Failed to fetch HITL latency:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time
  const formatTime = (seconds: number): string => {
    if (seconds < 60) {
      return `${Math.round(seconds)}s`
    } else if (seconds < 3600) {
      const mins = Math.round(seconds / 60)
      return `${mins}m`
    } else {
      const hours = (seconds / 3600).toFixed(1)
      return `${hours}h`
    }
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          SLA / P99 Approval Time
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center gap-2">
            <span className="text-[24px] font-semibold text-foreground">
              {formatTime(data?.p99_seconds ?? 0)}
            </span>
            <span className="text-[11px] text-muted-foreground">
              P99 approval time
            </span>
            <div className="mt-4 grid grid-cols-3 gap-4 w-full">
              <div className="flex flex-col items-center">
                <span className="text-[12px] font-medium text-foreground">
                  {formatTime(data?.p50_seconds ?? 0)}
                </span>
                <span className="text-[9px] text-muted-foreground">P50</span>
              </div>
              <div className="flex flex-col items-center">
                <span className="text-[12px] font-medium text-foreground">
                  {formatTime(data?.p90_seconds ?? 0)}
                </span>
                <span className="text-[9px] text-muted-foreground">P90</span>
              </div>
              <div className="flex flex-col items-center">
                <span className="text-[12px] font-medium text-foreground">
                  {formatTime(data?.avg_seconds ?? 0)}
                </span>
                <span className="text-[9px] text-muted-foreground">Avg</span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}