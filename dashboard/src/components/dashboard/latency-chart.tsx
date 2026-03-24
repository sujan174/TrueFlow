"use client"

import type { ProviderLatencyStat } from "@/lib/types/analytics"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

interface LatencyChartProps {
  data: ProviderLatencyStat[]
  loading?: boolean
}

const COLORS = [
  "hsl(var(--primary))",
  "hsl(var(--chart-3))",
  "hsl(var(--chart-4))",
]

function SkeletonBar() {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <Skeleton className="w-20 h-3" />
        <Skeleton className="w-10 h-3" />
      </div>
      <Skeleton className="h-2 w-full" />
    </div>
  )
}

export function LatencyChart({ data, loading }: LatencyChartProps) {
  if (loading) {
    return (
      <div className="flex-1 flex flex-col gap-2.5">
        {[1, 2, 3].map((i) => (
          <SkeletonBar key={i} />
        ))}
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground text-xs">
        No latency data available
      </div>
    )
  }

  const maxLatency = Math.max(...data.map((d) => d.latency_ms), 1)
  const items = data.slice(0, 3).map((item, i) => ({
    provider: item.provider,
    latency: item.latency_ms,
    color: COLORS[i % COLORS.length],
    barWidth: Math.round((item.latency_ms / maxLatency) * 160),
  }))

  return (
    <div className="flex-1 flex flex-col gap-2.5">
      {items.map((item) => (
        <div key={item.provider} className="flex flex-col gap-1">
          <div className="flex items-center justify-between">
            <span className="text-[11px] text-foreground">{item.provider}</span>
            <span className="text-[10px] text-muted-foreground">{item.latency}ms</span>
          </div>
          <div className="h-2 bg-muted rounded-full overflow-hidden">
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{
                width: `${item.barWidth}px`,
                backgroundColor: item.color,
              }}
            />
          </div>
        </div>
      ))}
    </div>
  )
}