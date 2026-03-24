"use client"

import { useEffect, useState } from "react"
import { getModelStats, type ModelStatsRow, formatCurrency, formatNumber } from "@/lib/api"

export function ModelStatsTable() {
  const [data, setData] = useState<ModelStatsRow[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const stats = await getModelStats(168)
        setData(stats)
      } catch (error) {
        console.error("Failed to fetch model stats:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
      </div>
    )
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Model Capabilities & Stats
        </span>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        <table className="w-full">
          <thead className="sticky top-0 bg-muted/50 z-10">
            <tr className="border-b border">
              <th className="text-left text-[9px] uppercase tracking-[1px] text-muted-foreground font-semibold px-4 py-3">
                Model
              </th>
              <th className="text-right text-[9px] uppercase tracking-[1px] text-muted-foreground font-semibold px-4 py-3">
                Tokens
              </th>
              <th className="text-right text-[9px] uppercase tracking-[1px] text-muted-foreground font-semibold px-4 py-3">
                Cost
              </th>
              <th className="text-right text-[9px] uppercase tracking-[1px] text-muted-foreground font-semibold px-4 py-3">
                Error Rate
              </th>
              <th className="text-right text-[9px] uppercase tracking-[1px] text-muted-foreground font-semibold px-4 py-3">
                Avg Latency
              </th>
              <th className="text-right text-[9px] uppercase tracking-[1px] text-muted-foreground font-semibold px-4 py-3">
                Requests
              </th>
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr>
                <td colSpan={6} className="text-center py-8 text-[14px] text-muted-foreground">
                  No model data available
                </td>
              </tr>
            ) : (
              data.map((row, idx) => (
                <tr
                  key={idx}
                  className="border-b border-border hover:bg-muted/50 transition-colors"
                >
                  <td className="px-4 py-3">
                    <span className="text-[12px] font-mono text-foreground">
                      {row.model}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right text-[12px] text-muted-foreground">
                    {formatNumber(row.total_tokens)}
                  </td>
                  <td className="px-4 py-3 text-right text-[12px] text-muted-foreground">
                    {formatCurrency(row.total_cost_usd)}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span
                      className={`text-[12px] font-semibold ${
                        row.error_rate > 1
                          ? "text-destructive"
                          : row.error_rate > 0.5
                            ? "text-muted-foreground"
                            : "text-success"
                      }`}
                    >
                      {row.error_rate.toFixed(2)}%
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right text-[12px] text-muted-foreground">
                    {Math.round(row.avg_latency_ms)}ms
                  </td>
                  <td className="px-4 py-3 text-right text-[12px] text-muted-foreground">
                    {formatNumber(row.request_count)}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}