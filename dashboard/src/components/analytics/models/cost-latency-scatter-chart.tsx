"use client"

import { useEffect, useState } from "react"
import {
  getCostLatencyScatter,
  type CostLatencyScatterPoint,
} from "@/lib/api"

// Color mapping for performance (based on latency)
function getPerformanceColor(avgLatency: number): string {
  if (avgLatency <= 500) return "#10B981" // Green - fast
  if (avgLatency <= 1000) return "#0A0A0A" // Black - moderate
  return "#F59E0B" // Amber - slow
}

export function CostLatencyScatterChart() {
  const [data, setData] = useState<CostLatencyScatterPoint[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const scatter = await getCostLatencyScatter(168)
        setData(scatter)
      } catch (error) {
        console.error("Failed to fetch cost-latency scatter:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center">
        <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center">
        <span className="text-[14px] text-muted-foreground">No data available</span>
      </div>
    )
  }

  // Calculate scales
  const maxLatency = Math.max(...data.map((d) => d.avg_latency_ms), 1)
  const maxCost = Math.max(...data.map((d) => d.avg_cost_per_request), 0.01)
  const maxSpend = Math.max(...data.map((d) => d.total_spend_usd), 1)

  // Dot sizing: sqrt scale for proportional sizing
  const minRadius = 6
  const maxRadius = 24

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Cost vs Latency by Model
        </span>
      </div>

      {/* Chart */}
      <div className="flex-1 p-4 relative">
        {/* Y-axis label */}
        <div className="absolute left-2 top-1/2 -translate-y-1/2 -rotate-90 text-[9px] text-muted-foreground uppercase tracking-[1px]">
          Latency (ms)
        </div>

        {/* Chart area */}
        <div className="ml-8 h-full relative">
          {/* Grid lines */}
          <div className="absolute inset-0 grid grid-cols-4 grid-rows-4 opacity-20">
            {Array.from({ length: 16 }).map((_, i) => (
              <div key={i} className="border border" />
            ))}
          </div>

          {/* Scatter points */}
          {data.map((point, idx) => {
            const x = (point.avg_cost_per_request / maxCost) * 100
            const y = (1 - point.avg_latency_ms / maxLatency) * 100 // Invert Y
            const radius = Math.sqrt(point.total_spend_usd / maxSpend) * (maxRadius - minRadius) + minRadius

            return (
              <div
                key={idx}
                className="absolute transform -translate-x-1/2 -translate-y-1/2 group cursor-pointer"
                style={{
                  left: `${Math.min(Math.max(x, 5), 95)}%`,
                  top: `${Math.min(Math.max(y, 5), 95)}%`,
                }}
              >
                {/* Dot */}
                <div
                  className="rounded-full transition-all duration-200 group-hover:scale-125"
                  style={{
                    width: radius,
                    height: radius,
                    backgroundColor: getPerformanceColor(point.avg_latency_ms),
                  }}
                />

                {/* Tooltip */}
                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block z-10">
                  <div className="bg-popover border text-foreground text-[10px] px-2 py-1 rounded shadow-lg whitespace-nowrap">
                    <div className="font-semibold">{point.model}</div>
                    <div className="opacity-80">
                      {point.avg_latency_ms.toFixed(0)}ms · ${point.avg_cost_per_request.toFixed(4)}/req
                    </div>
                  </div>
                </div>
              </div>
            )
          })}
        </div>

        {/* X-axis label */}
        <div className="text-center text-[9px] text-muted-foreground uppercase tracking-[1px] mt-2 ml-8">
          Cost per Request ($)
        </div>
      </div>
    </div>
  )
}