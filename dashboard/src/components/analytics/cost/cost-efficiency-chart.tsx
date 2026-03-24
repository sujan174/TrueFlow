"use client"

import { useEffect, useState } from "react"
import { getCostEfficiency, type CostEfficiencyPoint } from "@/lib/api"

export function CostEfficiencyChart() {
  const [data, setData] = useState<CostEfficiencyPoint[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const result = await getCostEfficiency(168) // 7 days
        setData(result)
      } catch (error) {
        console.error("Failed to fetch cost efficiency:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Aggregate by model and calculate average cost per 1k tokens
  const modelStats: Record<string, { total: number; count: number }> = {}
  data.forEach((point) => {
    if (!modelStats[point.model]) {
      modelStats[point.model] = { total: 0, count: 0 }
    }
    modelStats[point.model].total += point.cost_per_1k_tokens
    modelStats[point.model].count += 1
  })

  const modelAverages = Object.entries(modelStats)
    .map(([model, stats]) => ({
      model,
      avgCost: stats.total / stats.count,
    }))
    .sort((a, b) => b.avgCost - a.avgCost)
    .slice(0, 8)

  const maxCost = Math.max(...modelAverages.map((m) => m.avgCost), 1)

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Cost Efficiency by Model
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-[12px] text-muted-foreground">Loading...</div>
          </div>
        ) : modelAverages.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {modelAverages.map((item, index) => {
              const barWidth = (item.avgCost / maxCost) * 100
              const isActive = index === 0

              return (
                <div key={item.model} className="flex flex-col gap-1">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] text-muted-foreground truncate max-w-[140px]" title={item.model}>
                      {item.model}
                    </span>
                    <span className="text-[11px] font-medium text-foreground">
                      ${item.avgCost.toFixed(2)}/1k
                    </span>
                  </div>
                  <div className="h-2 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-2 rounded-full transition-all duration-300"
                      style={{
                        width: `${barWidth}%`,
                        backgroundColor: isActive ? "hsl(var(--primary))" : "hsl(var(--muted-foreground))",
                      }}
                    />
                  </div>
                </div>
              )
            })}

            {/* Footer note */}
            <div className="mt-2 text-[10px] text-muted-foreground">
              Lower cost/1k tokens = more efficient
            </div>
          </>
        )}
      </div>
    </div>
  )
}