"use client"

import { useEffect, useState } from "react"
import {
  LineChart,
  Line,
  ResponsiveContainer,
  XAxis,
  YAxis,
  Tooltip,
  Legend,
} from "recharts"
import { getLatencyTimeseries, type LatencyTimeseriesPoint, formatLatency } from "@/lib/api"

export function LatencyTrendChart() {
  const [data, setData] = useState<LatencyTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getLatencyTimeseries(24)
        setData(timeseries)
      } catch (error) {
        console.error("Failed to fetch latency timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleTimeString("en-US", { hour: "numeric" }),
    p50: Math.round(point.p50),
    p90: Math.round(point.p90),
    p99: Math.round(point.p99),
  }))

  // Get max latency for display
  const maxLatency = data.reduce((max, p) => Math.max(max, p.p99), 0)

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <div className="flex flex-col gap-0.5">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            LATENCY TREND
          </span>
          <span className="text-[12px] font-bold text-foreground">
            Peak p99: {formatLatency(maxLatency)}
          </span>
        </div>
        {data.length > 0 && (
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] text-muted-foreground">Last 24 hours</span>
          </div>
        )}
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {/* Legend */}
            <div className="flex items-center gap-4 text-[10px]">
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-info" />
                <span className="text-muted-foreground">p50</span>
              </div>
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-warning" />
                <span className="text-muted-foreground">p90</span>
              </div>
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-destructive" />
                <span className="text-muted-foreground">p99</span>
              </div>
            </div>

            {/* Chart */}
            <div className="flex-1 relative">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                  <XAxis dataKey="time" hide />
                  <YAxis hide />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: "hsl(var(--card))",
                      border: "1px solid #E2E8F0",
                      borderRadius: "8px",
                      boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
                    }}
                    formatter={(value) => formatLatency(value as number)}
                  />
                  <Line
                    type="monotone"
                    dataKey="p50"
                    stroke="#3B82F6"
                    strokeWidth={2}
                    dot={false}
                  />
                  <Line
                    type="monotone"
                    dataKey="p90"
                    stroke="#F59E0B"
                    strokeWidth={2}
                    dot={false}
                  />
                  <Line
                    type="monotone"
                    dataKey="p99"
                    stroke="#EF4444"
                    strokeWidth={2}
                    dot={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </>
        )}
      </div>
    </div>
  )
}