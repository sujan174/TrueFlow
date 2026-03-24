"use client"

import { useEffect, useState } from "react"
import { getAnalyticsExperiments, type ExperimentSummary } from "@/lib/api"

export function ABTestLiftCard() {
  const [experiments, setExperiments] = useState<ExperimentSummary[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const exp = await getAnalyticsExperiments()
        setExperiments(exp)
      } catch (error) {
        console.error("Failed to fetch experiments:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center">
        <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
      </div>
    )
  }

  // Find the most recent active experiment (baseline vs variant)
  const activeExperiment = experiments.length >= 2 ? experiments.slice(0, 2) : null

  if (!activeExperiment) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
        <div className="h-[44px] px-4 flex items-center border-b border">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            A/B Test Lift
          </span>
        </div>
        <div className="flex-1 flex items-center justify-center">
          <span className="text-[14px] text-muted-foreground">No active experiments</span>
        </div>
      </div>
    )
  }

  const champion = activeExperiment[0]
  const challenger = activeExperiment[1]

  // Calculate cost delta
  const costDelta = champion.total_cost_usd > 0
    ? ((challenger.total_cost_usd - champion.total_cost_usd) / champion.total_cost_usd) * 100
    : 0

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          A/B Test Lift
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col justify-center gap-4">
        {/* Experiment name */}
        <div className="text-[11px] font-medium text-foreground">
          {champion.experiment_name}
        </div>

        {/* Variants */}
        <div className="flex gap-4">
          {/* Champion */}
          <div className="flex-1 p-3 bg-muted/50 rounded-lg">
            <div className="text-[9px] uppercase tracking-[1px] text-muted-foreground mb-1">
              Champion
            </div>
            <div className="text-[12px] font-mono text-foreground truncate">
              {champion.variant_name}
            </div>
            <div className="text-[11px] text-muted-foreground mt-1">
              ${champion.total_cost_usd.toFixed(2)}
            </div>
          </div>

          {/* Challenger */}
          <div className="flex-1 p-3 bg-muted/50 rounded-lg">
            <div className="text-[9px] uppercase tracking-[1px] text-muted-foreground mb-1">
              Challenger
            </div>
            <div className="text-[12px] font-mono text-foreground truncate">
              {challenger.variant_name}
            </div>
            <div className="text-[11px] text-muted-foreground mt-1">
              ${challenger.total_cost_usd.toFixed(2)}
            </div>
          </div>
        </div>

        {/* Cost Delta */}
        <div className="flex items-center justify-center gap-2">
          <span className="text-[11px] text-muted-foreground">Cost Delta:</span>
          <span
            className={`text-[14px] font-semibold ${
              costDelta < 0 ? "text-success" : costDelta > 0 ? "text-destructive" : "text-muted-foreground"
            }`}
          >
            {costDelta > 0 ? "+" : ""}{costDelta.toFixed(1)}%
          </span>
        </div>

        {/* Latency comparison */}
        <div className="flex justify-between text-[10px] text-muted-foreground">
          <span>Avg Latency: {champion.avg_latency_ms.toFixed(0)}ms vs {challenger.avg_latency_ms.toFixed(0)}ms</span>
        </div>
      </div>
    </div>
  )
}