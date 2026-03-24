"use client"

import {
  LineChart,
  Line,
  ResponsiveContainer,
} from "recharts"

const sparklineData = [
  { x: 0, y: 30 },
  { x: 35, y: 45 },
  { x: 70, y: 35 },
  { x: 105, y: 55 },
  { x: 140, y: 50 },
  { x: 175, y: 70 },
  { x: 210, y: 60 },
]

export function FallbackWidget() {
  return (
    <div className="flex-1 flex flex-col gap-4">
      <span className="text-xs text-muted-foreground">
        Requests saved by fallback
      </span>
      <span className="text-2xl font-semibold text-success">
        1,204
      </span>

      {/* Sparkline */}
      <div className="relative h-[60px] w-full">
        {/* Grid line at bottom */}
        <div className="absolute bottom-0 left-0 w-[244px] h-px bg-border" />

        {/* Chart */}
        <ResponsiveContainer width="100%" height={60}>
          <LineChart data={sparklineData}>
            <Line
              type="monotone"
              dataKey="y"
              stroke="hsl(var(--color-success))"
              strokeWidth={2}
              dot={false}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}