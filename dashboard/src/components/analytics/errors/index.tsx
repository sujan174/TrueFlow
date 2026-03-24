"use client"

import { ErrorsKpiRibbon } from "./errors-kpi-ribbon"
import { ErrorsOverTimeChart } from "./errors-over-time-chart"
import { ErrorsByTypeChart } from "./errors-by-type-chart"
import { ErrorLogsTable } from "./error-logs-table"

export function ErrorsTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Title */}
      <div className="flex items-center justify-between h-10">
        <h2 className="text-[28px] font-bold text-foreground tracking-[-0.5px]">
          Errors
        </h2>
      </div>

      {/* Errors KPI Ribbon - 5 cards */}
      <ErrorsKpiRibbon />

      {/* Errors Over Time Chart - Rose tinted background */}
      <div className="h-[280px]">
        <ErrorsOverTimeChart />
      </div>

      {/* Middle Row - Errors by Type chart */}
      <div className="h-[220px]">
        <ErrorsByTypeChart />
      </div>

      {/* Bottom Row - Error Logs Table */}
      <div className="flex-1 min-h-[300px]">
        <ErrorLogsTable />
      </div>
    </div>
  )
}

export {
  ErrorsKpiRibbon,
  ErrorsOverTimeChart,
  ErrorsByTypeChart,
  ErrorLogsTable,
}