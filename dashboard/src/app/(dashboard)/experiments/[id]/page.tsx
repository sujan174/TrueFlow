"use client"

import { useEffect, useState, useCallback } from "react"
import { useParams } from "next/navigation"
import { ArrowLeft, Square, RefreshCw } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { toast } from "sonner"
import {
  getExperiment,
  stopExperiment,
  getExperimentTimeseries,
  type ExperimentWithResults,
  type ExperimentTimeseriesPoint,
} from "@/lib/api"
import { MetricsTable } from "@/components/experiments/metrics-table"
import { TrafficSplitChart } from "@/components/experiments/traffic-split-chart"
import { StatisticalSummary } from "@/components/experiments/statistical-summary"
import { LatencyTrendChart } from "@/components/experiments/latency-trend-chart"
import { CostTrendChart } from "@/components/experiments/cost-trend-chart"
import { formatRelativeTime } from "@/lib/utils"

export default function ExperimentDetailPage() {
  const params = useParams()
  const experimentId = params.id as string

  const [experiment, setExperiment] = useState<ExperimentWithResults | null>(null)
  const [timeseries, setTimeseries] = useState<ExperimentTimeseriesPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const fetchData = useCallback(async (isRefresh = false) => {
    if (isRefresh) {
      setRefreshing(true)
    } else {
      setLoading(true)
    }

    try {
      const [expData, tsData] = await Promise.all([
        getExperiment(experimentId),
        getExperimentTimeseries(experimentId, 24).catch(() => []),
      ])
      setExperiment(expData)
      setTimeseries(tsData)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load experiment")
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [experimentId])

  useEffect(() => {
    fetchData()
  }, [fetchData])

  const handleStop = async () => {
    if (!experiment) return

    try {
      await stopExperiment(experiment.id)
      setExperiment({ ...experiment, status: "stopped" })
      toast.success("Experiment stopped")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to stop experiment")
    }
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-muted-foreground">Loading experiment...</div>
      </div>
    )
  }

  if (error || !experiment) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-4">
        <div className="text-destructive">{error || "Experiment not found"}</div>
        <Link href="/experiments">
          <Button variant="outline">Back to Experiments</Button>
        </Link>
      </div>
    )
  }

  const variants = experiment.results?.map((r) => ({
    name: r.variant,
    weight: 0, // We don't have weight info from results
  })) || []

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div className="flex items-center gap-4">
            <Link href="/experiments">
              <Button variant="ghost" size="icon-sm">
                <ArrowLeft className="h-4 w-4" />
              </Button>
            </Link>
            <div className="flex flex-col gap-1">
              <div className="flex items-center gap-2">
                <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
                  {experiment.name}
                </h1>
                <Badge
                  variant={experiment.status === "running" ? "success" : "secondary"}
                  className="text-[10px]"
                >
                  {experiment.status === "running" ? "Running" : "Stopped"}
                </Badge>
              </div>
              <p className="text-sm text-muted-foreground">
                Created {formatRelativeTime(experiment.created_at)}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => fetchData(true)}
              disabled={refreshing}
            >
              <RefreshCw className={`h-4 w-4 mr-1 ${refreshing ? "animate-spin" : ""}`} />
              Refresh
            </Button>
            {experiment.status === "running" && (
              <Button variant="destructive" size="sm" onClick={handleStop}>
                <Square className="h-4 w-4 mr-1" />
                Stop Experiment
              </Button>
            )}
          </div>
        </div>

        {/* Metrics Comparison */}
        <MetricsTable results={experiment.results || []} />

        {/* Statistical Analysis */}
        <StatisticalSummary results={experiment.results || []} />

        {/* Charts Row */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <TrafficSplitChart
            variants={variants}
            results={experiment.results}
          />
          <LatencyTrendChart timeseries={timeseries} />
        </div>

        {/* Cost Chart */}
        <CostTrendChart timeseries={timeseries} />
      </div>
    </div>
  )
}