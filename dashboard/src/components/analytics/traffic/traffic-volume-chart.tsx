"use client"

import { useEffect, useState } from "react"
import {
  AreaChart,
  Area,
  ResponsiveContainer,
  XAxis,
  YAxis,
  Tooltip,
} from "recharts"
import {
  getTrafficTimeseries,
  type TrafficTimeseriesPoint,
  formatNumber,
} from "@/lib/api"
import { Activity } from "lucide-react"

// Custom tooltip component
function CustomTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string }>; label?: string }) {
  if (!active || !payload || payload.length === 0) return null

  return (
    <div className="bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-sm max-w-[200px]">
      <p className="text-muted-foreground text-xs mb-1.5">{label}</p>
      <div className="space-y-1">
        {payload.map((entry, index) => (
          <div key={index} className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-full" style={{ backgroundColor: entry.color }} />
              <span className="text-xs text-muted-foreground">{entry.name}</span>
            </div>
            <span className="text-xs font-medium text-foreground">{formatNumber(entry.value)}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

export function TrafficVolumeChart() {
  const [data, setData] = useState<TrafficTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [total, setTotal] = useState(0)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getTrafficTimeseries(24)
        setData(timeseries)
        setTotal(timeseries.reduce((sum, p) => sum + p.total_count, 0))
      } catch (error) {
        console.error("Failed to fetch traffic timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleTimeString("en-US", { hour: "numeric" }),
    Passed: point.passed_count,
    Throttled: point.throttled_count,
    Blocked: point.blocked_count,
    "HITL-paused": point.hitl_paused_count,
  }))

  // Color palette using CSS variables
  const colors = {
    passed: "var(--color-success)",
    throttled: "var(--color-warning)",
    blocked: "var(--color-error)",
    hitl: "var(--color-chart-5)",
  }

  return (
    <div className="h-full bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-200">
      {/* Header */}
      <div className="h-12 px-4 flex items-center justify-between border-b">
        <div className="flex items-center gap-2">
          <Activity className="h-4 w-4 text-requests" />
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            TRAFFIC VOLUME
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
      <div className="flex-1 p-4 flex flex-col gap-3">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center gap-2">
            <Activity className="h-8 w-8 text-muted-foreground/50" />
            <span className="text-sm text-muted-foreground">No traffic data</span>
            <span className="text-xs text-muted-foreground">Data will appear here</span>
          </div>
        ) : (
          <>
            {/* Legend */}
            <div className="flex items-center gap-4 text-xs flex-wrap">
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: colors.passed }} />
                <span className="text-muted-foreground">Passed</span>
              </div>
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: colors.throttled }} />
                <span className="text-muted-foreground">Throttled</span>
              </div>
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: colors.blocked }} />
                <span className="text-muted-foreground">Blocked</span>
              </div>
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: colors.hitl }} />
                <span className="text-muted-foreground">HITL</span>
              </div>
            </div>

            {/* Chart */}
            <div className="flex-1 relative">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                  <XAxis dataKey="time" hide />
                  <YAxis hide />
                  <Tooltip content={<CustomTooltip />} />
                  <Area
                    type="monotone"
                    dataKey="Passed"
                    stackId="1"
                    stroke={colors.passed}
                    fill={colors.passed}
                    fillOpacity={0.6}
                  />
                  <Area
                    type="monotone"
                    dataKey="Throttled"
                    stackId="1"
                    stroke={colors.throttled}
                    fill={colors.throttled}
                    fillOpacity={0.6}
                  />
                  <Area
                    type="monotone"
                    dataKey="Blocked"
                    stackId="1"
                    stroke={colors.blocked}
                    fill={colors.blocked}
                    fillOpacity={0.6}
                  />
                  <Area
                    type="monotone"
                    dataKey="HITL-paused"
                    stackId="1"
                    stroke={colors.hitl}
                    fill={colors.hitl}
                    fillOpacity={0.6}
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