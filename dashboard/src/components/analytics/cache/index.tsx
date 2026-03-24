"use client"

import { CacheRibbon } from "./cache-ribbon"
import { HitRateOverTimeChart } from "./hit-rate-over-time-chart"
import { TopCachedQueriesTable } from "./top-cached-queries-table"
import { ModelEfficiencyCard } from "./model-efficiency-card"
import { LatencyComparisonCard } from "./latency-comparison-card"

export function CacheTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Cache Ribbon - 4 KPI stats */}
      <CacheRibbon />

      {/* Hit Rate Over Time Chart - Full width */}
      <div className="h-[260px]">
        <HitRateOverTimeChart />
      </div>

      {/* Bottom Row - 3 cards */}
      <div className="h-[260px] flex gap-4">
        <div className="flex-1">
          <TopCachedQueriesTable />
        </div>
        <div className="w-[280px]">
          <ModelEfficiencyCard />
        </div>
        <div className="w-[280px]">
          <LatencyComparisonCard />
        </div>
      </div>
    </div>
  )
}

export {
  CacheRibbon,
  HitRateOverTimeChart,
  TopCachedQueriesTable,
  ModelEfficiencyCard,
  LatencyComparisonCard,
}