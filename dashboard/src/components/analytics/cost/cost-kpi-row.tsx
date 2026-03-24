"use client"

import { useEffect, useState } from "react"
import {
  getAnalyticsSummary,
  getSpendByProvider,
  formatCurrency,
  formatNumber,
  type AnalyticsSummary,
  type ProviderSpendStat,
} from "@/lib/api"

export function CostKpiRow() {
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null)
  const [topProvider, setTopProvider] = useState<ProviderSpendStat | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const [summaryData, providerData] = await Promise.all([
          getAnalyticsSummary(168), // 7 days
          getSpendByProvider(168),
        ])
        setSummary(summaryData)
        setTopProvider(providerData[0] || null)
      } catch (error) {
        console.error("Failed to fetch KPI data:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const costPer1kRequests = summary && summary.total_requests > 0
    ? (summary.total_cost / (summary.total_requests / 1000)).toFixed(2)
    : "0.00"

  const kpis = [
    {
      label: "Total Spend",
      value: summary ? formatCurrency(summary.total_cost) : "$0.00",
      sublabel: "Last 7 days",
    },
    {
      label: "Spend vs Cap",
      value: "78%",
      sublabel: "$3,900 / $5,000",
    },
    {
      label: "Cost / 1K Requests",
      value: `$${costPer1kRequests}`,
      sublabel: "Average",
    },
    {
      label: "Top Provider",
      value: topProvider?.provider || "N/A",
      sublabel: topProvider ? formatCurrency(topProvider.spend_usd) : "-",
    },
    {
      label: "Total Tokens",
      value: summary ? formatNumber(summary.total_tokens) : "0",
      sublabel: "Processed",
    },
  ]

  return (
    <div className="h-[80px] flex gap-4">
      {kpis.map((kpi) => (
        <div
          key={kpi.label}
          className="flex-1 bg-card border border rounded-[14px] p-4 flex flex-col justify-center shadow-sm"
        >
          {loading ? (
            <div className="animate-pulse text-muted-foreground text-[12px]">Loading...</div>
          ) : (
            <>
              <span className="text-[10px] font-medium tracking-[1px] text-muted-foreground uppercase">
                {kpi.label}
              </span>
              <span className="text-[24px] font-bold text-foreground mt-1">
                {kpi.value}
              </span>
              <span className="text-[11px] text-muted-foreground mt-0.5">
                {kpi.sublabel}
              </span>
            </>
          )}
        </div>
      ))}
    </div>
  )
}