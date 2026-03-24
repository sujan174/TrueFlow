"use client"

import { useEffect, useState } from "react"
import { getModelCacheEfficiency, type ModelCacheEfficiency } from "@/lib/api"

// Color mapping for efficiency levels
function getEfficiencyColor(hitRate: number): string {
  if (hitRate >= 60) return "#0A0A0A" // Black - highest
  if (hitRate >= 40) return "#555555" // Dark grey - medium
  return "#E2E8F0" // Light grey - lowest
}

export function ModelEfficiencyCard() {
  const [data, setData] = useState<ModelCacheEfficiency[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const efficiency = await getModelCacheEfficiency(168)
        setData(efficiency.slice(0, 3)) // Top 3 models
      } catch (error) {
        console.error("Failed to fetch model cache efficiency:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const maxHitRate = Math.max(...data.map((d) => d.hit_rate), 1)

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          MODEL EFFICIENCY
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col justify-center gap-4">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No model data</span>
          </div>
        ) : (
          data.map((model, idx) => (
            <div key={idx} className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-muted-foreground">{model.model}</span>
                <span className="text-[10px] font-semibold text-muted-foreground">
                  {model.hit_rate.toFixed(0)}%
                </span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-300"
                  style={{
                    width: `${(model.hit_rate / maxHitRate) * 100}%`,
                    backgroundColor: getEfficiencyColor(model.hit_rate),
                  }}
                />
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}