"use client"

import { useMemo } from "react"
import { Check, AlertTriangle, TrendingDown, TrendingUp, Minus } from "lucide-react"
import { cn } from "@/lib/utils"
import type { ExperimentResult } from "@/lib/api"

interface StatisticalSummaryProps {
  results: ExperimentResult[]
  primaryMetric?: "latency" | "cost" | "tokens" | "error_rate"
}

// Approximate t-test p-value using normal approximation for large samples
function approximatePValue(t: number, df: number): number {
  // For large df, t-distribution approaches normal
  // Use a simple approximation
  const absT = Math.abs(t)
  if (df >= 30) {
    // Normal approximation
    const p = 2 * (1 - normalCDF(absT))
    return Math.max(0, Math.min(1, p))
  }
  // For smaller samples, use a conservative estimate
  // This is simplified - in production, use a proper statistics library
  if (absT > 2.576) return 0.01
  if (absT > 1.96) return 0.05
  if (absT > 1.645) return 0.10
  return 0.20
}

// Standard normal CDF approximation
function normalCDF(x: number): number {
  const a1 = 0.254829592
  const a2 = -0.284496736
  const a3 = 1.421413741
  const a4 = -1.453152027
  const a5 = 1.061405429
  const p = 0.3275911

  const sign = x < 0 ? -1 : 1
  x = Math.abs(x) / Math.sqrt(2)

  const t = 1.0 / (1.0 + p * x)
  const y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * Math.exp(-x * x)

  return 0.5 * (1.0 + sign * y)
}

function calculateSignificance(
  control: ExperimentResult,
  treatment: ExperimentResult,
  metricKey: "avg_latency_ms" | "total_cost_usd" | "error_rate"
): {
  isSignificant: boolean
  pValue: number
  delta: number
  deltaPercent: number
  confidenceInterval: [number, number]
  winner: "control" | "treatment" | null
} {
  const n1 = control.total_requests
  const n2 = treatment.total_requests

  // Need sufficient sample size
  if (n1 < 30 || n2 < 30) {
    return {
      isSignificant: false,
      pValue: 1,
      delta: 0,
      deltaPercent: 0,
      confidenceInterval: [0, 0],
      winner: null,
    }
  }

  const mean1 = control[metricKey]
  const mean2 = treatment[metricKey]

  // Estimate variance from error rate for that metric
  // This is a simplification - in reality we'd use individual request data
  const estVariance1 = mean1 * 0.3 // 30% coefficient of variation estimate
  const estVariance2 = mean2 * 0.3

  // Standard error of difference
  const se = Math.sqrt(estVariance1 / n1 + estVariance2 / n2)

  if (se === 0) {
    return {
      isSignificant: false,
      pValue: 1,
      delta: 0,
      deltaPercent: 0,
      confidenceInterval: [0, 0],
      winner: null,
    }
  }

  // t-statistic
  const t = (mean2 - mean1) / se

  // Degrees of freedom (Welch-Satterthwaite approximation)
  const df = Math.min(n1 + n2 - 2, 1000)

  // P-value
  const pValue = approximatePValue(t, df)

  // Delta
  const delta = mean2 - mean1
  const deltaPercent = mean1 !== 0 ? ((mean2 - mean1) / mean1) * 100 : 0

  // 95% confidence interval
  const tCrit = 1.96 // Approximate for large df
  const ci: [number, number] = [
    delta - tCrit * se,
    delta + tCrit * se,
  ]

  // Determine winner (lower is better for latency, cost, error rate)
  const lowerIsBetter = true
  const isSignificant = pValue < 0.05

  let winner: "control" | "treatment" | null = null
  if (isSignificant) {
    if (lowerIsBetter) {
      winner = mean2 < mean1 ? "treatment" : "control"
    } else {
      winner = mean2 > mean1 ? "treatment" : "control"
    }
  }

  return { isSignificant, pValue, delta, deltaPercent, confidenceInterval: ci, winner }
}

interface MetricCardProps {
  title: string
  control: ExperimentResult
  treatment: ExperimentResult
  metricKey: "avg_latency_ms" | "total_cost_usd" | "error_rate"
  formatValue: (v: number) => string
  lowerIsBetter?: boolean
}

function MetricCard({ title, control, treatment, metricKey, formatValue, lowerIsBetter = true }: MetricCardProps) {
  const stats = useMemo(() => {
    return calculateSignificance(control, treatment, metricKey)
  }, [control, treatment, metricKey])

  const improvement = stats.deltaPercent < 0 ? Math.abs(stats.deltaPercent) : -stats.deltaPercent
  const treatmentIsBetter = lowerIsBetter ? treatment[metricKey] < control[metricKey] : treatment[metricKey] > control[metricKey]

  return (
    <div className="bg-muted/30 rounded-lg p-3 space-y-2">
      <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
        {title}
      </div>

      <div className="flex items-center justify-between">
        <div className="text-sm">
          <span className="text-muted-foreground">Control: </span>
          <span className="font-medium">{formatValue(control[metricKey])}</span>
        </div>
        <div className="text-sm">
          <span className="text-muted-foreground">Treatment: </span>
          <span className="font-medium">{formatValue(treatment[metricKey])}</span>
        </div>
      </div>

      {stats.isSignificant ? (
        <div className="flex items-center gap-2">
          {treatmentIsBetter ? (
            <TrendingDown className="h-4 w-4 text-green-600" />
          ) : (
            <TrendingUp className="h-4 w-4 text-red-600" />
          )}
          <span className={cn(
            "text-sm font-medium",
            treatmentIsBetter ? "text-green-600" : "text-red-600"
          )}>
            {treatmentIsBetter ? "" : "+"}{improvement.toFixed(1)}%
          </span>
          <span className="text-xs text-muted-foreground">
            (p={stats.pValue.toFixed(3)})
          </span>
          <Check className="h-4 w-4 text-green-600" />
        </div>
      ) : (
        <div className="flex items-center gap-2 text-muted-foreground">
          <Minus className="h-4 w-4" />
          <span className="text-sm">Not significant</span>
          <span className="text-xs">({stats.pValue.toFixed(3)})</span>
        </div>
      )}
    </div>
  )
}

export function StatisticalSummary({ results, primaryMetric = "latency" }: StatisticalSummaryProps) {
  // Need at least 2 variants (control and treatment)
  if (results.length < 2) {
    return (
      <div className="bg-card border rounded-xl p-6 text-center text-muted-foreground">
        <AlertTriangle className="h-8 w-8 mx-auto mb-2 text-muted-foreground/50" />
        <p>Statistical analysis requires at least 2 variants</p>
      </div>
    )
  }

  // Sort by request count to find control (usually the first/highest traffic)
  const sorted = [...results].sort((a, b) => b.total_requests - a.total_requests)
  const control = sorted[0]
  const treatment = sorted[1]

  // Check for sufficient data
  const hasEnoughData = control.total_requests >= 30 && treatment.total_requests >= 30

  if (!hasEnoughData) {
    return (
      <div className="bg-card border rounded-xl p-6">
        <div className="flex items-center gap-3 mb-4">
          <AlertTriangle className="h-5 w-5 text-yellow-600" />
          <h3 className="text-sm font-semibold">Statistical Analysis</h3>
        </div>
        <p className="text-sm text-muted-foreground">
          More data needed for statistical significance. Current:
        </p>
        <div className="mt-2 flex gap-4 text-sm">
          <span>Control: <strong>{control.total_requests}</strong> requests</span>
          <span>Treatment: <strong>{treatment.total_requests}</strong> requests</span>
        </div>
        <p className="text-xs text-muted-foreground mt-2">
          Need at least 30 requests per variant for reliable statistics.
        </p>
      </div>
    )
  }

  return (
    <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
      <div className="px-4 py-3 border-b flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Statistical Analysis</h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Comparing {control.variant} vs {treatment.variant}
          </p>
        </div>
        <div className="text-xs text-muted-foreground">
          95% confidence level
        </div>
      </div>

      <div className="p-4 space-y-3">
        <MetricCard
          title="Latency"
          control={control}
          treatment={treatment}
          metricKey="avg_latency_ms"
          formatValue={(v) => `${v.toFixed(0)}ms`}
        />
        <MetricCard
          title="Cost"
          control={control}
          treatment={treatment}
          metricKey="total_cost_usd"
          formatValue={(v) => `$${v.toFixed(4)}`}
        />
        <MetricCard
          title="Error Rate"
          control={control}
          treatment={treatment}
          metricKey="error_rate"
          formatValue={(v) => `${(v * 100).toFixed(1)}%`}
        />
      </div>

      <div className="px-4 py-2 bg-muted/20 border-t text-xs text-muted-foreground">
        Note: Statistics are approximate based on aggregated metrics. For precise analysis,
        use individual request-level data.
      </div>
    </div>
  )
}