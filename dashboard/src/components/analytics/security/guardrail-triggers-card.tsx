"use client"

import { useEffect, useState } from "react"
import { getGuardrailTriggers, type GuardrailTriggerStat } from "@/lib/api"

const CATEGORY_COLORS: Record<string, string> = {
  "Jailbreak": "#DC2626",
  "Harmful content": "#DC2626",
  "Code injection": "#F59E0B",
  "Profanity": "#64748B",
  "Bias": "#64748B",
  "Sensitive topics": "#64748B",
  "Competitor mentions": "#64748B",
}

function getCountColor(count: number): string {
  if (count >= 200) return "#DC2626"
  if (count >= 100) return "#F59E0B"
  return "#64748B"
}

export function GuardrailTriggersCard() {
  const [data, setData] = useState<GuardrailTriggerStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const triggers = await getGuardrailTriggers(168)
        setData(triggers)
      } catch (error) {
        console.error("Failed to fetch guardrail triggers:", error)
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
        Guardrail Triggers by Category
      </h3>
      <div className="flex-1 flex flex-col gap-3 overflow-auto">
        {data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-[12px] text-muted-foreground">
            No guardrail triggers in the selected period
          </div>
        ) : (
          data.map((item) => {
            const barWidth = (item.count / maxCount) * 100
            const countColor = getCountColor(item.count)
            return (
              <div key={item.category} className="flex items-center gap-3">
                <span className="w-[100px] text-[12px] text-muted-foreground truncate">
                  {item.category}
                </span>
                <div className="flex-1 h-[20px] bg-muted rounded-[4px] overflow-hidden">
                  <div
                    className="h-full rounded-[4px]"
                    style={{
                      width: `${barWidth}%`,
                      backgroundColor: countColor,
                    }}
                  />
                </div>
                <span
                  className="w-[40px] text-[12px] font-medium text-right"
                  style={{ color: countColor }}
                >
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