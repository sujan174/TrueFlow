"use client"

import { CostKpiRow } from "./cost-kpi-row"
import { BudgetHealthStrip } from "./budget-health-strip"
import { SpendOverTimeChart } from "./spend-over-time-chart"
import { CostEfficiencyChart } from "./cost-efficiency-chart"
import { CostByProviderCard } from "./cost-by-provider-card"
import { CostByModelCard } from "./cost-by-model-card"
import { BudgetBurnRateCard } from "./budget-burn-rate-card"
import { CostBreakdownTable } from "./cost-breakdown-table"

export function CostTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* KPI Row */}
      <CostKpiRow />

      {/* Budget Health Strip */}
      <BudgetHealthStrip />

      {/* Charts Row */}
      <div className="h-[320px] flex gap-4">
        <div className="flex-1">
          <SpendOverTimeChart />
        </div>
        <div className="flex-1">
          <CostEfficiencyChart />
        </div>
      </div>

      {/* Bottom Row */}
      <div className="h-[240px] flex gap-4">
        <CostByProviderCard />
        <CostByModelCard />
        <BudgetBurnRateCard />
      </div>

      {/* Cost Breakdown Table */}
      <div className="h-[400px]">
        <CostBreakdownTable />
      </div>
    </div>
  )
}

export {
  CostKpiRow,
  BudgetHealthStrip,
  SpendOverTimeChart,
  CostEfficiencyChart,
  CostByProviderCard,
  CostByModelCard,
  BudgetBurnRateCard,
  CostBreakdownTable,
}