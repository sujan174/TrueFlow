"use client"

import { useMemo } from "react"
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  Legend,
  CartesianGrid,
} from "recharts"
import { DollarSign } from "lucide-react"
import { Skeleton } from "@/components/ui/skeleton"
import type { ExperimentTimeseriesPoint } from "@/lib/api"

interface CostTrendChartProps {
  timeseries: ExperimentTimeseriesPoint[]
  loading?: boolean
}

const VARIANT_COLORS = [
  "hsl(var(--chart-1))",
  "hsl(var(--chart-2))",
  "hsl(var(--chart-3))",
  "hsl(var(--chart-4))",
  "hsl(var(--chart-5))",
]

function CustomTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ name: string; value: number; color: string }>; label?: string }) {
  if (!active || !payload || payload.length === 0) return null

  return (
    <div className="bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-sm">
      <p className="text-muted-foreground text-xs mb-1">{label}</p>
      {payload.map((entry, index) => (
        <div key={index} className="flex items-center gap-2">
          <div
            className="w-2 h-2 rounded-full"
            style={{ backgroundColor: entry.color }}
          />
          <span className="text-muted-foreground">{entry.name}:</span>
          <span className="font-medium">${entry.value.toFixed(4)}</span>
        </div>
      ))}
    </div>
  )
}

export function CostTrendChart({ timeseries, loading }: CostTrendChartProps) {
  // Group data by bucket and accumulate cost - useMemo ensures re-computation when timeseries changes
  const chartData = useMemo(() => {
    const buckets = new Map<string, Record<string, number | string>>()

    timeseries.forEach((point) => {
      const time = new Date(point.bucket).toLocaleTimeString("en-US", {
        hour: "numeric",
        minute: "2-digit",
      })

      if (!buckets.has(time)) {
        buckets.set(time, { time })
      }
      const bucket = buckets.get(time)!
      bucket[point.variant_name] = point.total_cost_usd
    })

    return Array.from(buckets.values())
  }, [timeseries])

  // Get unique variant names
  const variants = useMemo(() => {
    const names = new Set<string>()
    timeseries.forEach((point) => names.add(point.variant_name))
    return Array.from(names)
  }, [timeseries])

  if (loading) {
    return (
      <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
        <div className="px-4 py-3 border-b flex items-center gap-2">
          <DollarSign className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-semibold">Cost Over Time</span>
        </div>
        <div className="p-4 h-[250px] flex items-center justify-center">
          <Skeleton className="h-full w-full" />
        </div>
      </div>
    )
  }

  if (timeseries.length === 0) {
    return (
      <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
        <div className="px-4 py-3 border-b flex items-center gap-2">
          <DollarSign className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-semibold">Cost Over Time</span>
        </div>
        <div className="p-4 h-[250px] flex items-center justify-center text-muted-foreground">
          No timeseries data available
        </div>
      </div>
    )
  }

  // Calculate total cost
  const totalCost = timeseries.reduce((sum, p) => sum + p.total_cost_usd, 0)

  return (
    <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
      <div className="px-4 py-3 border-b flex items-center justify-between">
        <div className="flex items-center gap-2">
          <DollarSign className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-semibold">Cost Over Time</span>
        </div>
        <div className="text-sm font-medium text-muted-foreground">
          Total: ${totalCost.toFixed(4)}
        </div>
      </div>
      <div className="p-4 h-[250px]">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={chartData} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
            <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
            <XAxis
              dataKey="time"
              tick={{ fontSize: 10 }}
              className="text-muted-foreground"
              tickLine={false}
            />
            <YAxis
              tick={{ fontSize: 10 }}
              className="text-muted-foreground"
              tickLine={false}
              tickFormatter={(v) => `$${v.toFixed(3)}`}
            />
            <Tooltip content={<CustomTooltip />} />
            <Legend
              wrapperStyle={{ fontSize: "11px" }}
            />
            {variants.map((variant, index) => (
              <Line
                key={variant}
                type="monotone"
                dataKey={variant}
                stroke={VARIANT_COLORS[index % VARIANT_COLORS.length]}
                strokeWidth={2}
                dot={false}
                activeDot={{ r: 4 }}
              />
            ))}
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}