"use client"

import { SecurityKpiRibbon } from "./security-kpi-ribbon"
import { GuardrailTriggersCard } from "./guardrail-triggers-card"
import { PiiBreakdownCard } from "./pii-breakdown-card"
import { DataResidencyCard } from "./data-residency-card"
import { PolicyActionsCard } from "./policy-actions-card"
import { ShadowPoliciesTable } from "./shadow-policies-table"

export function SecurityTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Title - 28px to match Cache page */}
      <div className="flex items-center justify-between h-10">
        <h2 className="text-[28px] font-bold text-foreground tracking-[-0.5px]">
          Security
        </h2>
      </div>

      {/* Security KPI Ribbon - 4 stats with dividers */}
      <SecurityKpiRibbon />

      {/* Top Row - 2 cards side by side */}
      <div className="h-[260px] flex gap-4">
        <div className="flex-1">
          <GuardrailTriggersCard />
        </div>
        <div className="flex-1">
          <PiiBreakdownCard />
        </div>
      </div>

      {/* Middle Row - 3 cards */}
      <div className="h-[200px] flex gap-4">
        <div className="w-[280px]">
          <DataResidencyCard />
        </div>
        <div className="flex-1">
          <PolicyActionsCard />
        </div>
      </div>

      {/* Bottom Row - Shadow Policies Table */}
      <div className="flex-1 min-h-[200px]">
        <ShadowPoliciesTable />
      </div>
    </div>
  )
}

export {
  SecurityKpiRibbon,
  GuardrailTriggersCard,
  PiiBreakdownCard,
  DataResidencyCard,
  PolicyActionsCard,
  ShadowPoliciesTable,
}