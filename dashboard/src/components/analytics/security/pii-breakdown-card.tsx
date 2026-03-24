"use client"

import { useEffect, useState } from "react"
import { getPiiBreakdown, type PiiBreakdownStat } from "@/lib/api"

const PATTERN_COLORS: Record<string, string> = {
  "Email": "#0B1020",
  "API key": "#0B1020",
  "Credit card": "#475569",
  "SSN": "#94A3B8",
  "Phone": "#CBD5E1",
  "NLP-detected": "#E2E8F0",
}

export function PiiBreakdownCard() {
  const [data, setData] = useState<PiiBreakdownStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const breakdown = await getPiiBreakdown(168)
        setData(breakdown)
      } catch (error) {
        console.error("Failed to fetch PII breakdown:", error)
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

  const maxCount = Math.max(...data.map((d) => d.count), 1)

  return (
    <div className="h-full bg-card border border rounded-[14px] p-5 flex flex-col shadow-sm">
      <h3 className="text-[14px] font-semibold text-foreground mb-4">
        PII Breakdown by Pattern
      </h3>
      <div className="flex-1 flex items-end gap-4 px-2">
        {data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-[12px] text-muted-foreground">
            No PII redactions in the selected period
          </div>
        ) : (
          data.map((item) => {
            const barHeight = (item.count / maxCount) * 100
            const color = PATTERN_COLORS[item.pattern] || "#94A3B8"
            return (
              <div
                key={item.pattern}
                className="flex flex-col items-center gap-2 flex-1"
              >
                <div className="flex-1 w-full flex items-end justify-center">
                  <div
                    className="w-[40px] rounded-t-[4px]"
                    style={{
                      height: `${barHeight}%`,
                      minHeight: "20px",
                      backgroundColor: color,
                    }}
                  />
                </div>
                <span className="text-[10px] text-muted-foreground text-center truncate w-full">
                  {item.pattern}
                </span>
                <span className="text-[12px] font-medium text-foreground">
                  {item.count}
                </span>
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}