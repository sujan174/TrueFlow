"use client"

import { useEffect, useState } from "react"
import {
  getModelUsageTimeseries,
  type ModelUsageTimeseriesPoint,
} from "@/lib/api"

type GroupBy = "requests" | "cost" | "cache_hits"

export function ModelUsageChart() {
  const [data, setData] = useState<ModelUsageTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [groupBy, setGroupBy] = useState<GroupBy>("requests")

  useEffect(() => {
    async function fetchData() {
      setLoading(true)
      try {
        const timeseries = await getModelUsageTimeseries(groupBy, 168)
        setData(timeseries)
      } catch (error) {
        console.error("Failed to fetch model usage timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [groupBy])

  // Group by bucket for chart
  const buckets = [...new Set(data.map((d) => d.bucket))].sort()
  const models = [...new Set(data.map((d) => d.model))].slice(0, 5) // Top 5 models

  // Pivot data for stacking
  const chartData = buckets.map((bucket) => {
    const bucketData: Record<string, number | string> = { bucket }
    models.forEach((model) => {
      const point = data.find((d) => d.bucket === bucket && d.model === model)
      bucketData[model] = point?.value ?? 0
    })
    return bucketData
  })

  // Calculate max for scale
  const maxValue = Math.max(
    ...chartData.map((d) => models.reduce((sum, m) => sum + ((d[m] as number) ?? 0), 0)),
    1
  )

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Model Usage Over Time
        </span>

        {/* Toggle Pills */}
        <div className="flex gap-1 bg-muted rounded-lg p-0.5">
          {(["requests", "cost", "cache_hits"] as GroupBy[]).map((option) => (
            <button
              key={option}
              onClick={() => setGroupBy(option)}
              className={`px-2.5 py-1 text-[10px] font-medium rounded-md transition-all ${
                groupBy === option
                  ? "bg-card text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {option === "cache_hits" ? "Cache Hits" : option.charAt(0).toUpperCase() + option.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* Chart */}
      <div className="flex-1 p-4 relative">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
          </div>
        ) : chartData.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <div className="h-full flex flex-col">
            {/* Chart area with bars */}
            <div className="flex-1 flex items-end gap-1">
              {chartData.slice(-24).map((d, idx) => {
                const total = models.reduce((sum, m) => sum + ((d[m] as number) ?? 0), 0)
                const height = (total / maxValue) * 100

                return (
                  <div key={idx} className="flex-1 flex flex-col justify-end h-full">
                    <div
                      className="w-full rounded-t-sm transition-all duration-200 hover:opacity-80"
                      style={{
                        height: `${Math.max(height, 2)}%`,
                        backgroundColor: "hsl(var(--primary) / 0.07)",
                      }}
                      title={`${new Date(d.bucket as string).toLocaleDateString()}: ${total.toFixed(1)}`}
                    />
                  </div>
                )
              })}
            </div>

            {/* X-axis labels */}
            <div className="h-6 flex items-center justify-between mt-2 text-[9px] text-muted-foreground">
              <span>{chartData.length > 0 ? new Date(chartData[0].bucket as string).toLocaleDateString() : ""}</span>
              <span>{chartData.length > 0 ? new Date(chartData[chartData.length - 1].bucket as string).toLocaleDateString() : ""}</span>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}