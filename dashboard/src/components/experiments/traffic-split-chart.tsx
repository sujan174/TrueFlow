"use client"

import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from "recharts"
import type { ExperimentVariant } from "@/lib/api"

interface TrafficSplitChartProps {
  variants: ExperimentVariant[]
  results?: { variant: string; total_requests: number }[]
}

const VARIANT_COLORS = [
  "hsl(var(--chart-1))",
  "hsl(var(--chart-2))",
  "hsl(var(--chart-3))",
  "hsl(var(--chart-4))",
  "hsl(var(--chart-5))",
]

function CustomTooltip({ active, payload }: { active?: boolean; payload?: Array<{ name: string; value: number; payload: { percentage: number } }> }) {
  if (!active || !payload || payload.length === 0) return null

  const data = payload[0]
  return (
    <div className="bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-sm">
      <p className="font-medium">{data.name}</p>
      <p className="text-muted-foreground">
        {data.value} ({data.payload.percentage.toFixed(1)}%)
      </p>
    </div>
  )
}

export function TrafficSplitChart({ variants, results }: TrafficSplitChartProps) {
  // Calculate total weight
  const totalWeight = variants.reduce((sum, v) => sum + v.weight, 0)

  // Build chart data
  const chartData = variants.map((v, index) => {
    const percentage = totalWeight > 0 ? (v.weight / totalWeight) * 100 : 0
    const resultData = results?.find((r) => r.variant === v.name)

    return {
      name: v.name,
      value: v.weight,
      percentage,
      actualRequests: resultData?.total_requests || 0,
      color: VARIANT_COLORS[index % VARIANT_COLORS.length],
    }
  })

  return (
    <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
      <div className="px-4 py-3 border-b">
        <h3 className="text-sm font-semibold">Traffic Split</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          Variant weights configuration
        </p>
      </div>

      <div className="p-4">
        <div className="h-[200px]">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie
                data={chartData}
                cx="50%"
                cy="50%"
                innerRadius={50}
                outerRadius={80}
                paddingAngle={2}
                dataKey="value"
              >
                {chartData.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill={entry.color} />
                ))}
              </Pie>
              <Tooltip content={<CustomTooltip />} />
              <Legend
                verticalAlign="bottom"
                height={36}
                formatter={(value: string, entry: { payload?: { percentage?: number } }) => (
                  <span className="text-xs text-muted-foreground">
                    {value} ({entry.payload?.percentage?.toFixed(0) || 0}%)
                  </span>
                )}
              />
            </PieChart>
          </ResponsiveContainer>
        </div>

        {/* Variant details */}
        <div className="mt-4 space-y-2">
          {chartData.map((item) => (
            <div key={item.name} className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                <div
                  className="w-3 h-3 rounded-sm"
                  style={{ backgroundColor: item.color }}
                />
                <span className="font-medium">{item.name}</span>
              </div>
              <div className="text-right">
                <span className="text-muted-foreground">
                  {item.percentage.toFixed(1)}%
                </span>
                {item.actualRequests > 0 && (
                  <span className="ml-2 text-xs text-muted-foreground">
                    ({item.actualRequests.toLocaleString()} req)
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}