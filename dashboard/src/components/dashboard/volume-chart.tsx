"use client"

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  ResponsiveContainer,
  Tooltip,
} from "recharts"
import type { AnalyticsTimeseriesPoint } from "@/lib/types/analytics"
import { Skeleton } from "@/components/ui/skeleton"
import { formatNumber } from "@/lib/api"

interface VolumeChartProps {
  data: AnalyticsTimeseriesPoint[]
  loading?: boolean
}

// Custom tooltip
function CustomTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number }>; label?: number }) {
  if (!active || !payload || payload.length === 0) return null

  return (
    <div className="bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-sm">
      <p className="text-muted-foreground text-xs mb-0.5">Day {label}</p>
      <p className="font-semibold text-foreground">
        {formatNumber(payload[0].value)} requests
      </p>
    </div>
  )
}

export function VolumeChart({ data, loading }: VolumeChartProps) {
  if (loading) {
    return (
      <div className="w-full h-[180px] rounded-lg overflow-hidden flex items-center justify-center">
        <Skeleton className="w-full h-[160px]" />
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="w-full h-[180px] rounded-lg overflow-hidden flex flex-col items-center justify-center gap-2">
        <div className="w-8 h-8 rounded-full bg-muted flex items-center justify-center">
          <span className="text-muted-foreground text-xs">📊</span>
        </div>
        <span className="text-muted-foreground text-xs">No request data</span>
      </div>
    )
  }

  const chartData = data.map((point) => ({
    day: new Date(point.bucket).getDate(),
    value: point.request_count,
  }))

  return (
    <div className="w-full h-[180px] rounded-lg overflow-hidden">
      <ResponsiveContainer width="100%" height={180}>
        <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
          <defs>
            <linearGradient id="volumeGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--color-requests)" stopOpacity={0.4} />
              <stop offset="95%" stopColor="var(--color-requests)" stopOpacity={0} />
            </linearGradient>
          </defs>
          <XAxis
            dataKey="day"
            stroke="var(--muted-foreground)"
            fontSize={10}
            tickLine={false}
            axisLine={false}
          />
          <YAxis
            stroke="var(--muted-foreground)"
            fontSize={10}
            tickLine={false}
            axisLine={false}
            tickFormatter={(value) => formatNumber(value)}
            width={40}
          />
          <Tooltip content={<CustomTooltip />} />
          <Area
            type="monotone"
            dataKey="value"
            stroke="var(--color-requests)"
            strokeWidth={2}
            fill="url(#volumeGradient)"
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
}