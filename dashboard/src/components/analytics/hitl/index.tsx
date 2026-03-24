"use client"

import { HitlKpiRibbon } from "./hitl-kpi-ribbon"
import { PendingApprovalsCard } from "./pending-approvals-card"
import { HitlVolumeChart } from "./hitl-volume-chart"
import { RejectionReasonsCard } from "./rejection-reasons-card"
import { SlaCard } from "./sla-card"

export function HitlTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Title - 28px to match Cache/Security pages */}
      <div className="flex items-center justify-between h-10">
        <h2 className="text-[28px] font-bold text-foreground tracking-[-0.5px]">
          HITL
        </h2>
      </div>

      {/* HITL KPI Ribbon - 3 stats with dividers */}
      <HitlKpiRibbon />

      {/* Top Row - Pending Approvals Table */}
      <div className="min-h-[320px]">
        <PendingApprovalsCard />
      </div>

      {/* Middle Row - HITL Volume Chart */}
      <div className="h-[280px]">
        <HitlVolumeChart />
      </div>

      {/* Bottom Row - Rejection Reasons + SLA */}
      <div className="h-[200px] flex gap-4">
        <div className="flex-1">
          <RejectionReasonsCard />
        </div>
        <div className="w-[280px]">
          <SlaCard />
        </div>
      </div>
    </div>
  )
}

export {
  HitlKpiRibbon,
  PendingApprovalsCard,
  HitlVolumeChart,
  RejectionReasonsCard,
  SlaCard,
}