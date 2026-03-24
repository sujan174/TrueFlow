"use client"

import { CostLatencyScatterChart } from "./cost-latency-scatter-chart"
import { ABTestLiftCard } from "./ab-test-lift-card"
import { ModelUsageChart } from "./model-usage-chart"
import { ErrorRateCard } from "./error-rate-card"
import { CacheEfficiencyCard } from "./cache-efficiency-card"
import { ModelStatsTable } from "./model-stats-table"

export function ModelsTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Top Row: Cost vs Latency Scatter + A/B Test Lift */}
      <div className="h-[320px] flex gap-4">
        <div className="flex-1">
          <CostLatencyScatterChart />
        </div>
        <div className="w-[300px]">
          <ABTestLiftCard />
        </div>
      </div>

      {/* Middle Row: Model Usage Over Time */}
      <div className="h-[280px]">
        <ModelUsageChart />
      </div>

      {/* Bottom Row: Error Rate + Cache Efficiency */}
      <div className="h-[200px] flex gap-4">
        <ErrorRateCard />
        <CacheEfficiencyCard />
      </div>

      {/* Table Row: Model Stats */}
      <div className="h-[360px]">
        <ModelStatsTable />
      </div>
    </div>
  )
}

export {
  CostLatencyScatterChart,
  ABTestLiftCard,
  ModelUsageChart,
  ErrorRateCard,
  CacheEfficiencyCard,
  ModelStatsTable,
}