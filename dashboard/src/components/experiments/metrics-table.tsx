"use client"

import { Check, Minus } from "lucide-react"
import { cn } from "@/lib/utils"
import type { ExperimentResult } from "@/lib/api"

interface MetricsTableProps {
  results: ExperimentResult[]
  primaryMetric?: "latency" | "cost" | "tokens" | "error_rate"
}

function formatNumber(num: number, decimals = 2): string {
  if (num >= 1000000) {
    return `${(num / 1000000).toFixed(1)}M`
  }
  if (num >= 1000) {
    return `${(num / 1000).toFixed(1)}K`
  }
  return num.toFixed(decimals)
}

function formatCurrency(num: number): string {
  if (num < 0.01) {
    return `$${num.toFixed(4)}`
  }
  return `$${num.toFixed(2)}`
}

function formatLatency(ms: number): string {
  if (ms >= 1000) {
    return `${(ms / 1000).toFixed(2)}s`
  }
  return `${ms.toFixed(0)}ms`
}

function findBestVariant(
  results: ExperimentResult[],
  metric: keyof ExperimentResult,
  lowerIsBetter: boolean
): string | null {
  if (results.length === 0) return null

  const validResults = results.filter((r) => r.total_requests > 0)
  if (validResults.length === 0) return null

  const sorted = [...validResults].sort((a, b) => {
    const aVal = a[metric]
    const bVal = b[metric]
    if (typeof aVal === "number" && typeof bVal === "number") {
      return lowerIsBetter ? aVal - bVal : bVal - aVal
    }
    return 0
  })

  return sorted[0]?.variant || null
}

function WinnerIndicator({ isWinner }: { isWinner: boolean }) {
  if (!isWinner) return null
  return (
    <span className="inline-flex items-center gap-0.5 text-green-600 text-xs font-medium">
      <Check className="h-3 w-3" />
    </span>
  )
}

export function MetricsTable({ results, primaryMetric = "latency" }: MetricsTableProps) {
  if (results.length === 0) {
    return (
      <div className="bg-card border rounded-xl p-6 text-center text-muted-foreground">
        No results yet. Data will appear here once requests are routed through this experiment.
      </div>
    )
  }

  const totalRequests = results.reduce((sum, r) => sum + r.total_requests, 0)
  const bestLatency = findBestVariant(results, "avg_latency_ms", true)
  const bestCost = findBestVariant(results, "total_cost_usd", true)
  const bestErrorRate = findBestVariant(results, "error_rate", true)

  return (
    <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
      <div className="px-4 py-3 border-b">
        <h3 className="text-sm font-semibold">Metrics Comparison</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          {totalRequests.toLocaleString()} total requests across all variants
        </p>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full">
          <thead className="bg-muted/30 border-b">
            <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
              <th className="px-4 py-2 text-left">Variant</th>
              <th className="px-4 py-2 text-right">Requests</th>
              <th className="px-4 py-2 text-right">Avg Latency</th>
              <th className="px-4 py-2 text-right">Total Cost</th>
              <th className="px-4 py-2 text-right">Avg Tokens</th>
              <th className="px-4 py-2 text-right">Error Rate</th>
            </tr>
          </thead>
          <tbody>
            {results.map((result) => {
              const isBestLatency = result.variant === bestLatency
              const isBestCost = result.variant === bestCost
              const isBestErrorRate = result.variant === bestErrorRate

              return (
                <tr key={result.variant} className="border-b last:border-0 hover:bg-muted/20">
                  <td className="px-4 py-3">
                    <span className="text-sm font-medium">{result.variant}</span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-sm tabular-nums">
                      {result.total_requests.toLocaleString()}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-1">
                      <span className={cn(
                        "text-sm tabular-nums",
                        isBestLatency && "text-green-600 font-medium"
                      )}>
                        {formatLatency(result.avg_latency_ms)}
                      </span>
                      <WinnerIndicator isWinner={isBestLatency} />
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-1">
                      <span className={cn(
                        "text-sm tabular-nums",
                        isBestCost && "text-green-600 font-medium"
                      )}>
                        {formatCurrency(result.total_cost_usd)}
                      </span>
                      <WinnerIndicator isWinner={isBestCost} />
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-sm tabular-nums">
                      {formatNumber(result.avg_tokens, 0)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-1">
                      <span className={cn(
                        "text-sm tabular-nums",
                        result.error_rate > 0.05 ? "text-destructive" :
                        isBestErrorRate && "text-green-600 font-medium"
                      )}>
                        {(result.error_rate * 100).toFixed(1)}%
                      </span>
                      <WinnerIndicator isWinner={isBestErrorRate && result.error_rate < 0.05} />
                    </div>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      <div className="px-4 py-2 bg-muted/20 border-t text-xs text-muted-foreground">
        <span className="inline-flex items-center gap-1">
          <Check className="h-3 w-3 text-green-600" />
          Best performer for metric
        </span>
      </div>
    </div>
  )
}