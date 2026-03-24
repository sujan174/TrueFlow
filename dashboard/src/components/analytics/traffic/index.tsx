"use client"

import { TrafficVolumeChart } from "./traffic-volume-chart"
import { LatencyTrendChart } from "./latency-trend-chart"
import { RequestLogTable } from "./request-log-table"

export function TrafficTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Charts Row */}
      <div className="h-[280px] flex gap-4">
        <div className="flex-1">
          <TrafficVolumeChart />
        </div>
        <div className="flex-1">
          <LatencyTrendChart />
        </div>
      </div>

      {/* Request Log Table */}
      <div className="h-[400px]">
        <RequestLogTable />
      </div>
    </div>
  )
}

export { TrafficVolumeChart, LatencyTrendChart, RequestLogTable }