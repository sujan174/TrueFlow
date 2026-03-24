"use client"

import { useEffect, useState } from "react"
import { getErrorTimeseries, type ErrorTimeseriesPoint } from "@/lib/api"

export function ErrorsOverTimeChart() {
  const [data, setData] = useState<ErrorTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getErrorTimeseries(168)
        setData(timeseries)
      } catch (error) {
        console.error("Failed to fetch error timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-destructive/10 border border-destructive/30 rounded-[14px] flex items-center justify-center">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading error chart...</div>
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="h-full bg-destructive/10 border border-destructive/30 rounded-[14px] flex items-center justify-center">
        <span className="text-[14px] text-muted-foreground">No error data available</span>
      </div>
    )
  }

  // Find max value for scaling
  const maxCount = Math.max(
    ...data.flatMap((d) => [
      d.timeout_count,
      d.rate_limit_count,
      d.upstream_5xx_count,
      d.circuit_breaker_count,
    ])
  )

  const chartWidth = 900
  const chartHeight = 180
  const paddingX = 40
  const paddingY = 20
  const innerWidth = chartWidth - paddingX * 2
  const innerHeight = chartHeight - paddingY * 2

  const xScale = (i: number) => paddingX + (i / (data.length - 1 || 1)) * innerWidth
  const yScale = (v: number) => paddingY + innerHeight - (v / (maxCount || 1)) * innerHeight

  const lineColors = {
    timeout: "#F43F5E", // rose
    rate_limit: "#F59E0B", // amber
    upstream_5xx: "#0A0A0A", // black
    circuit_breaker: "#E2E8F0", // grey
  }

  // Legend dot colors (intentionally different from line colors per spec)
  const legendDotColors = {
    timeout: "#14B8A6", // teal
    rate_limit: "#FFFFFF",
    upstream_5xx: "#FFFFFF",
    circuit_breaker: "#FFFFFF",
  }

  const createPath = (key: keyof typeof lineColors) => {
    const points = data.map((d, i) => {
      const value = d[`${key}_count` as keyof ErrorTimeseriesPoint] as number
      return `${xScale(i)},${yScale(value)}`
    })
    return `M ${points.join(" L ")}`
  }

  return (
    <div className="h-full bg-destructive/10 border border-destructive/30 rounded-[14px] p-4 flex flex-col">
      {/* Title */}
      <div className="h-6 flex items-center justify-between mb-2">
        <span className="text-[12px] font-semibold text-foreground">Errors Over Time</span>
      </div>

      {/* Chart */}
      <div className="flex-1 flex items-center justify-center">
        <svg width={chartWidth} height={chartHeight} className="overflow-visible">
          {/* Y-axis labels */}
          <text x={paddingX - 8} y={paddingY + 4} className="text-[10px] fill-[#64748B] text-right" textAnchor="end">
            {maxCount}
          </text>
          <text x={paddingX - 8} y={chartHeight - paddingY + 4} className="text-[10px] fill-[#64748B] text-right" textAnchor="end">
            0
          </text>

          {/* Grid lines */}
          <line x1={paddingX} y1={paddingY} x2={chartWidth - paddingX} y2={paddingY} stroke="#E2E8F0" strokeWidth="1" strokeDasharray="4,4" />
          <line x1={paddingX} y1={chartHeight - paddingY} x2={chartWidth - paddingX} y2={chartHeight - paddingY} stroke="#E2E8F0" strokeWidth="1" />

          {/* Lines */}
          <path d={createPath("timeout")} fill="none" stroke={lineColors.timeout} strokeWidth="2" />
          <path d={createPath("rate_limit")} fill="none" stroke={lineColors.rate_limit} strokeWidth="2" />
          <path d={createPath("upstream_5xx")} fill="none" stroke={lineColors.upstream_5xx} strokeWidth="2" />
          <path d={createPath("circuit_breaker")} fill="none" stroke={lineColors.circuit_breaker} strokeWidth="2" />
        </svg>
      </div>

      {/* Legend */}
      <div className="h-8 flex items-center justify-center gap-6">
        <div className="flex items-center gap-1.5">
          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: legendDotColors.timeout }} />
          <span className="text-[11px] text-muted-foreground">timeout</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-2 h-2 rounded-full border border" style={{ backgroundColor: legendDotColors.rate_limit }} />
          <span className="text-[11px] text-muted-foreground">rate_limit</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-2 h-2 rounded-full border border" style={{ backgroundColor: legendDotColors.upstream_5xx }} />
          <span className="text-[11px] text-muted-foreground">upstream_5xx</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-2 h-2 rounded-full border border" style={{ backgroundColor: legendDotColors.circuit_breaker }} />
          <span className="text-[11px] text-muted-foreground">circuit_breaker</span>
        </div>
      </div>
    </div>
  )
}