"use client"

import { useEffect, useState } from "react"
import { getAnalyticsSummary, type AnalyticsSummary } from "@/lib/api"
import { KPICard } from "@/components/ui/kpi-card"
import { CheckCircle, DollarSign } from "lucide-react"

// Mini sparkline component with CSS variable colors
function MiniSparkline({ highlightIndex = -1, color = "var(--color-success)" }: { highlightIndex?: number; color?: string }) {
  const heights = [6, 10, 8, 12, 9, 14, 11]

  return (
    <div className="flex items-end gap-0.5">
      {heights.map((height, index) => (
        <div
          key={index}
          className="w-1 rounded-[2px] transition-all duration-200"
          style={{
            height: `${height}px`,
            backgroundColor: index === highlightIndex ? color : "var(--muted-foreground)",
            opacity: index === highlightIndex ? 1 : 0.4,
          }}
        />
      ))}
    </div>
  )
}

export function KPICards() {
  const [data, setData] = useState<AnalyticsSummary | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const summary = await getAnalyticsSummary(24)
        setData(summary)
      } catch (error) {
        console.error("Failed to fetch analytics summary:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const successRate = data && data.total_requests > 0
    ? ((data.success_count / data.total_requests) * 100)
    : 0

  const totalCost = data?.total_cost ?? 0
  const projectedMonthly = totalCost * 30

  return (
    <>
      {/* Success Rate - Enhanced Card */}
      <KPICard
        label="Success Rate"
        value={loading ? "..." : `${successRate.toFixed(1)}%`}
        icon={CheckCircle}
        progress={successRate}
        trend={{ value: 0.3, label: "vs prior" }}
        variant="success"
        glow={successRate > 95}
        loading={loading}
        className="h-[120px]"
      />

      {/* Total Cost - Enhanced Card */}
      <KPICard
        label="Total Cost"
        value={loading ? "..." : `$${totalCost.toFixed(2)}`}
        icon={DollarSign}
        trend={{
          value: 8.2,
          label: projectedMonthly >= 1000
            ? `~$${(projectedMonthly / 1000).toFixed(1)}k/mo`
            : `~$${projectedMonthly.toFixed(0)}/mo`
        }}
        variant="spend"
        loading={loading}
        className="h-[120px]"
      />
    </>
  )
}