"use client"

import { useEffect, useState } from "react"
import {
  LineChart,
  Line,
  ResponsiveContainer,
  XAxis,
  Tooltip,
  YAxis,
  Area,
  AreaChart,
} from "recharts"
import { getAnalyticsTimeseries, type AnalyticsTimeseriesPoint, formatNumber } from "@/lib/api"
import { Activity } from "lucide-react"
import { Skeleton } from "@/components/ui/skeleton"

// Custom tooltip component with Tailwind styles
function CustomTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string }>; label?: string }) {
  if (!active || !payload || payload.length === 0) return null

  return (
    <div className="bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-sm">
      <p className="text-muted-foreground text-xs mb-1">{label}</p>
      <p className="font-semibold text-foreground">
        {formatNumber(payload[0].value)} requests
      </p>
    </div>
  )
}

export function RequestVolumeChart() {
  const [data, setData] = useState<AnalyticsTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [total, setTotal] = useState(0)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getAnalyticsTimeseries(24)
        setData(timeseries)
        setTotal(timeseries.reduce((sum, p) => sum + p.request_count, 0))
      } catch (error) {
        console.error("Failed to fetch timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleTimeString("en-US", { hour: "numeric" }),
    requests: point.request_count,
    errors: point.error_count,
  }))

  return (
    <div className="h-full bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-200">
      {/* Header */}
      <div className="h-12 px-4 flex items-center justify-between border-b">
        <div className="flex items-center gap-2">
          <Activity className="h-4 w-4 text-requests" />
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            REQUEST VOLUME
          </span>
        </div>
        {data.length > 0 && (
          <div className="flex items-center gap-2">
            <span className="text-sm font-bold text-foreground">
              {formatNumber(total)}
            </span>
            <span className="text-xs text-muted-foreground">24h</span>
          </div>
        )}
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2">
        {loading ? (
          <div className="flex-1 flex flex-col gap-3">
            <div className="flex items-center gap-4">
              <Skeleton className="h-4 w-20" />
            </div>
            <div className="flex-1 flex items-end gap-1">
              {Array.from({ length: 24 }).map((_, i) => (
                <Skeleton
                  key={i}
                  className="flex-1 rounded-sm"
                  style={{ height: `${20 + Math.random() * 60}%`, animationDelay: `${i * 20}ms` }}
                />
              ))}
            </div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center gap-2">
            <Activity className="h-8 w-8 text-muted-foreground/50" />
            <span className="text-sm text-muted-foreground">No requests yet</span>
            <span className="text-xs text-muted-foreground">Data will appear here</span>
          </div>
        ) : (
          <>
            {/* Legend */}
            <div className="flex items-center gap-4">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-requests" />
                <span className="text-xs text-muted-foreground">Requests</span>
              </div>
            </div>

            {/* Chart with gradient fill */}
            <div className="flex-1 relative">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                  <defs>
                    <linearGradient id="requestGradient" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="var(--color-requests)" stopOpacity={0.3} />
                      <stop offset="95%" stopColor="var(--color-requests)" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <XAxis
                    dataKey="time"
                    hide
                  />
                  <YAxis hide />
                  <Tooltip content={<CustomTooltip />} />
                  <Area
                    type="monotone"
                    dataKey="requests"
                    stroke="var(--color-requests)"
                    strokeWidth={2}
                    fill="url(#requestGradient)"
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </>
        )}
      </div>
    </div>
  )
}