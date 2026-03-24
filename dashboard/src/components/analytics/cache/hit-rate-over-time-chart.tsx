"use client"

import { useEffect, useState } from "react"
import {
  AreaChart,
  Area,
  ResponsiveContainer,
  XAxis,
  Tooltip,
} from "recharts"
import { getCacheHitRateTimeseries, type CacheHitRatePoint } from "@/lib/api"

export function HitRateOverTimeChart() {
  const [data, setData] = useState<CacheHitRatePoint[]>([])
  const [loading, setLoading] = useState(true)
  const [avgHitRate, setAvgHitRate] = useState(0)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getCacheHitRateTimeseries(168) // 7 days
        setData(timeseries)
        // Calculate average hit rate
        if (timeseries.length > 0) {
          const total = timeseries.reduce((sum, p) => sum + p.hit_rate, 0)
          setAvgHitRate(total / timeseries.length)
        }
      } catch (error) {
        console.error("Failed to fetch cache hit rate timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleDateString("en-US", { weekday: "short" }),
    hitRate: point.hit_rate,
    hitCount: point.hit_count,
    totalCount: point.total_count,
  }))

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          HIT RATE OVER TIME
        </span>
        <span className="text-[24px] font-semibold text-success">
          {avgHitRate.toFixed(1)}%
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No cache data available</span>
          </div>
        ) : (
          <div className="flex-1 relative">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 10, bottom: 10 }}>
                <XAxis dataKey="time" hide />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--card))",
                    border: "1px solid #E2E8F0",
                    borderRadius: "8px",
                    boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
                  }}
                  formatter={(value) => `${(value as number).toFixed(1)}%`}
                />
                <Area
                  type="monotone"
                  dataKey="hitRate"
                  stroke="#10B981"
                  strokeWidth={3}
                  fill="#10B981"
                  fillOpacity={0.13}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        )}
      </div>
    </div>
  )
}