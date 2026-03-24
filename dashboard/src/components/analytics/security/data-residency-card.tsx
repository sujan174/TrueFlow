"use client"

import { useEffect, useState } from "react"
import { getDataResidency, type DataResidencyStats } from "@/lib/api"

export function DataResidencyCard() {
  const [data, setData] = useState<DataResidencyStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const stats = await getDataResidency(168)
        setData(stats)
      } catch (error) {
        console.error("Failed to fetch data residency:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading...</div>
      </div>
    )
  }

  const euPercent = data?.eu_percent ?? 0
  const usPercent = data?.us_percent ?? 0

  return (
    <div className="h-full bg-card border border rounded-[14px] p-5 flex flex-col shadow-sm">
      <h3 className="text-[14px] font-semibold text-foreground mb-4">
        Data Residency
      </h3>
      <div className="flex-1 flex flex-col gap-4">
        {/* EU Route */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className="text-[12px] text-muted-foreground">EU Route</span>
            <span className="text-[12px] font-medium text-success">
              {euPercent.toFixed(0)}%
            </span>
          </div>
          <div className="h-[8px] bg-muted rounded-full overflow-hidden">
            <div
              className="h-full rounded-full"
              style={{
                width: `${euPercent}%`,
                backgroundColor: "#16A34A",
              }}
            />
          </div>
        </div>

        {/* US Route */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className="text-[12px] text-muted-foreground">US Route</span>
            <span className="text-[12px] font-medium text-muted-foreground">
              {usPercent.toFixed(0)}%
            </span>
          </div>
          <div className="h-[8px] bg-muted rounded-full overflow-hidden">
            <div
              className="h-full rounded-full"
              style={{
                width: `${usPercent}%`,
                backgroundColor: "#64748B",
              }}
            />
          </div>
        </div>
      </div>
    </div>
  )
}