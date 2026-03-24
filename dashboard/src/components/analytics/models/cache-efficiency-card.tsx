"use client"

import { useEffect, useState } from "react"
import { getModelCacheEfficiency, type ModelCacheEfficiency } from "@/lib/api"

// 3-tier color system: teal (≥50%), grey (25-49%), amber (<25%)
function getEfficiencyColor(hitRate: number): string {
  if (hitRate >= 50) return "#14B8A6" // Teal
  if (hitRate >= 25) return "#64748B" // Grey
  return "#F59E0B" // Amber
}

export function CacheEfficiencyCard() {
  const [data, setData] = useState<ModelCacheEfficiency[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const efficiency = await getModelCacheEfficiency(168)
        setData(efficiency)
      } catch (error) {
        console.error("Failed to fetch model cache efficiency:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Cache Efficiency by Model
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col justify-center gap-3 overflow-auto">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No cache data</span>
          </div>
        ) : (
          data.slice(0, 5).map((model, idx) => (
            <div key={idx} className="flex items-center gap-3">
              {/* Model name */}
              <div className="w-[140px] text-[11px] font-mono text-muted-foreground truncate">
                {model.model}
              </div>

              {/* Progress bar */}
              <div className="flex-1 h-3 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-300"
                  style={{
                    width: `${model.hit_rate}%`,
                    backgroundColor: getEfficiencyColor(model.hit_rate),
                  }}
                />
              </div>

              {/* Percentage */}
              <div className="w-[50px] text-right text-[11px] font-semibold text-muted-foreground">
                {model.hit_rate.toFixed(0)}%
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}