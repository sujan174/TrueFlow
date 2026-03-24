"use client"

import { useEffect, useState } from "react"
import { getSpendTimeseries, formatCurrency, type SpendTimeseriesPoint } from "@/lib/api"

export function SpendOverTimeChart() {
  const [data, setData] = useState<SpendTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [groupBy, setGroupBy] = useState<"provider" | "model" | "token">("provider")

  useEffect(() => {
    async function fetchData() {
      try {
        const result = await getSpendTimeseries(groupBy, 168) // 7 days
        setData(result)
      } catch (error) {
        console.error("Failed to fetch spend timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [groupBy])

  // Aggregate data by dimension for stacked bars
  const dimensions = [...new Set(data.map((d) => d.dimension))]
  const buckets = [...new Set(data.map((d) => d.bucket))].sort()

  // Group by bucket and calculate totals
  const bucketTotals: Record<string, number> = {}
  buckets.forEach((bucket) => {
    bucketTotals[bucket] = data
      .filter((d) => d.bucket === bucket)
      .reduce((sum, d) => sum + d.spend_usd, 0)
  })

  const maxSpend = Math.max(...Object.values(bucketTotals), 1)

  // Colors for different dimensions
  const colors = [
    "#0A0A0A",
    "#3B82F6",
    "#10B981",
    "#F59E0B",
    "#EF4444",
    "#8B5CF6",
    "#EC4899",
  ]

  const dimensionColors: Record<string, string> = {}
  dimensions.forEach((dim, index) => {
    dimensionColors[dim] = colors[index % colors.length]
  })

  // Format bucket for display
  const formatBucket = (bucket: string) => {
    const date = new Date(bucket)
    return date.toLocaleDateString("en-US", { month: "short", day: "numeric" })
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Spend Over Time
        </span>
        <div className="flex gap-2">
          {(["provider", "model", "token"] as const).map((g) => (
            <button
              key={g}
              onClick={() => setGroupBy(g)}
              className={`px-2 py-1 text-[10px] rounded-md transition-colors ${
                groupBy === g
                  ? "bg-primary text-white"
                  : "bg-muted text-muted-foreground hover:bg-border"
              }`}
            >
              {g.charAt(0).toUpperCase() + g.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-[12px] text-muted-foreground">Loading chart...</div>
          </div>
        ) : buckets.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {/* Chart Area */}
            <div className="flex-1 flex items-end gap-2">
              {buckets.slice(-14).map((bucket) => {
                const bucketData = data.filter((d) => d.bucket === bucket)
                const total = bucketTotals[bucket]
                const height = (total / maxSpend) * 200

                return (
                  <div
                    key={bucket}
                    className="flex-1 flex flex-col items-center gap-1"
                    title={`${formatBucket(bucket)}: ${formatCurrency(total)}`}
                  >
                    <div
                      className="w-full rounded-t-md overflow-hidden"
                      style={{ height: `${Math.max(height, 4)}px` }}
                    >
                      {/* Stacked bar segments */}
                      {bucketData.map((segment, index) => {
                        const segmentHeight =
                          (segment.spend_usd / total) * Math.max(height, 4)
                        return (
                          <div
                            key={`${segment.dimension}-${index}`}
                            className="w-full"
                            style={{
                              height: `${segmentHeight}px`,
                              backgroundColor: dimensionColors[segment.dimension],
                            }}
                          />
                        )
                      })}
                    </div>
                  </div>
                )
              })}
            </div>

            {/* Legend */}
            <div className="mt-4 flex flex-wrap gap-3">
              {dimensions.slice(0, 5).map((dim) => (
                <div key={dim} className="flex items-center gap-1.5">
                  <div
                    className="w-2.5 h-2.5 rounded-sm"
                    style={{ backgroundColor: dimensionColors[dim] }}
                  />
                  <span className="text-[10px] text-muted-foreground capitalize">{dim}</span>
                </div>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  )
}