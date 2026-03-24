"use client"

import { useEffect, useState } from "react"
import { getErrorBreakdown, type ErrorTypeBreakdown } from "@/lib/api"

export function ErrorsByTypeChart() {
  const [data, setData] = useState<ErrorTypeBreakdown[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const breakdown = await getErrorBreakdown(168)
        setData(breakdown)
      } catch (error) {
        console.error("Failed to fetch error breakdown:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading error breakdown...</div>
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <span className="text-[14px] text-muted-foreground">No error type data available</span>
      </div>
    )
  }

  const maxCount = Math.max(...data.map((d) => d.count))
  const barHeight = 28
  const barGap = 12
  const labelHeight = 16

  return (
    <div className="h-full bg-card border border rounded-[14px] p-4 flex flex-col shadow-sm">
      {/* Title */}
      <div className="h-6 flex items-center justify-between mb-2">
        <span className="text-[12px] font-semibold text-foreground">Errors by Type</span>
      </div>

      {/* Chart */}
      <div className="flex-1 flex flex-col justify-center gap-2 px-4">
        {data.slice(0, 5).map((item, index) => {
          const barWidth = (item.count / (maxCount || 1)) * 100
          const isFirst = index === 0
          const barColor = isFirst ? "#FF3B30" : "#E2E8F0"
          const textColor = isFirst ? "#FF3B30" : "#64748B"
          const countColor = isFirst ? "#FF3B30" : "#64748B"

          return (
            <div key={item.error_type} className="flex flex-col gap-0.5">
              {/* Label above bar */}
              <span className="text-[11px] font-medium text-muted-foreground pl-1">
                {item.error_type.toLowerCase()}
              </span>

              {/* Bar container */}
              <div className="flex items-center gap-3 h-[28px]">
                {/* Bar */}
                <div className="flex-1 h-[28px] bg-muted rounded overflow-hidden">
                  <div
                    className="h-full rounded transition-all duration-300"
                    style={{
                      width: `${barWidth}%`,
                      backgroundColor: barColor,
                    }}
                  />
                </div>

                {/* Count */}
                <span className={`text-[12px] font-medium min-w-[50px] text-right`} style={{ color: countColor }}>
                  {item.count.toLocaleString()}
                </span>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}