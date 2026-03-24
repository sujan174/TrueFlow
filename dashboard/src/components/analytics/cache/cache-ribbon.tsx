"use client"

import { useEffect, useState } from "react"
import { getCacheSummary, type CacheSummaryStats, formatNumber } from "@/lib/api"

function formatBytes(bytes: number): string {
  if (bytes >= 1073741824) {
    return (bytes / 1073741824).toFixed(1) + " GB"
  }
  if (bytes >= 1048576) {
    return (bytes / 1048576).toFixed(1) + " MB"
  }
  if (bytes >= 1024) {
    return (bytes / 1024).toFixed(1) + " KB"
  }
  return bytes + " B"
}

export function CacheRibbon() {
  const [data, setData] = useState<CacheSummaryStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const summary = await getCacheSummary(24)
        setData(summary)
      } catch (error) {
        console.error("Failed to fetch cache summary:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-[72px] bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading cache stats...</div>
      </div>
    )
  }

  const hitRate = data?.hit_rate ?? 0
  const costAvoided = data?.cost_avoided_usd ?? 0
  const cacheSize = data?.cache_size_bytes ?? 0
  const topModel = data?.top_cached_model ?? "N/A"

  return (
    <div className="h-[72px] bg-card border border rounded-[14px] flex items-center px-6 gap-8 shadow-sm">
      {/* Overall Hit Rate */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Overall Hit Rate
        </span>
        <span className="text-[20px] font-semibold text-success">
          {hitRate.toFixed(1)}%
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Cost Avoided */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Cost Avoided
        </span>
        <span className="text-[20px] font-semibold text-success">
          ${formatNumber(costAvoided)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Cache Size */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Cache Size
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {formatBytes(cacheSize)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Top Model Cached */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Top Model Cached
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {topModel}
        </span>
      </div>
    </div>
  )
}