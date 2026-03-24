"use client"

import { useEffect, useState } from "react"
import { getErrorSummary, type ErrorSummaryStats, formatNumber } from "@/lib/api"

function calculateDelta(current: number, prior: number): { value: number; direction: "up" | "down" | "neutral" } {
  if (prior === 0) {
    return { value: 0, direction: "neutral" }
  }
  const delta = ((current - prior) / prior) * 100
  if (delta > 0) return { value: Math.abs(delta), direction: "up" }
  if (delta < 0) return { value: Math.abs(delta), direction: "down" }
  return { value: 0, direction: "neutral" }
}

function formatDelta(delta: number): string {
  if (delta < 1) return `${delta.toFixed(1)}%`
  return `${Math.round(delta)}%`
}

export function ErrorsKpiRibbon() {
  const [data, setData] = useState<ErrorSummaryStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const summary = await getErrorSummary(168)
        setData(summary)
      } catch (error) {
        console.error("Failed to fetch error summary:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-[100px] bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading error stats...</div>
      </div>
    )
  }

  const totalErrors = data?.total_errors ?? 0
  const errorRate = data?.error_rate ?? 0
  const circuitBreakerTrips = data?.circuit_breaker_trips ?? 0
  const rateLimitHits = data?.rate_limit_hits ?? 0
  const topErrorType = data?.top_error_type ?? null
  const priorTotalErrors = data?.prior_total_errors ?? 0
  const priorTotalRequests = data?.prior_total_requests ?? 0

  // Calculate deltas
  const totalErrorsDelta = calculateDelta(totalErrors, priorTotalErrors)
  const errorRateDelta = calculateDelta(errorRate, priorTotalRequests > 0 ? (priorTotalErrors / priorTotalRequests) * 100 : 0)

  return (
    <div className="h-[100px] bg-card border border rounded-[14px] flex items-center px-6 shadow-sm">
      {/* Total Errors - Red delta */}
      <div className="flex-1 flex flex-col gap-1.5">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Total Errors
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {formatNumber(totalErrors)}
        </span>
        {totalErrorsDelta.direction !== "neutral" && (
          <span className={`text-[11px] font-medium ${totalErrorsDelta.direction === "up" ? "text-destructive" : "text-muted-foreground"}`}>
            {totalErrorsDelta.direction === "up" ? "↑" : "↓"} {formatDelta(totalErrorsDelta.value)}
          </span>
        )}
      </div>

      <div className="w-[1px] h-[60px] bg-border" />

      {/* Error Rate */}
      <div className="flex-1 flex flex-col gap-1.5">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Error Rate
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {errorRate.toFixed(1)}%
        </span>
        {errorRateDelta.direction !== "neutral" && (
          <span className="text-[11px] font-medium text-muted-foreground">
            {errorRateDelta.direction === "up" ? "↑" : "↓"} {formatDelta(errorRateDelta.value)}
          </span>
        )}
      </div>

      <div className="w-[1px] h-[60px] bg-border" />

      {/* Circuit Breaker Trips */}
      <div className="flex-1 flex flex-col gap-1.5">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Circuit Breaker Trips
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {formatNumber(circuitBreakerTrips)}
        </span>
      </div>

      <div className="w-[1px] h-[60px] bg-border" />

      {/* Rate Limit Hits */}
      <div className="flex-1 flex flex-col gap-1.5">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Rate Limit Hits
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {formatNumber(rateLimitHits)}
        </span>
      </div>

      <div className="w-[1px] h-[60px] bg-border" />

      {/* Top Error Type - Red delta, lowercase string value */}
      <div className="flex-1 flex flex-col gap-1.5">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Top Error Type
        </span>
        <span className="text-[20px] font-semibold text-muted-foreground">
          {topErrorType ? topErrorType.toLowerCase() : "—"}
        </span>
      </div>
    </div>
  )
}