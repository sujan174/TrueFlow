"use client"

import { useEffect, useState } from "react"
import { getPolicyActions, type PolicyActionStat } from "@/lib/api"

const ACTION_COLORS: Record<string, string> = {
  "redact": "#0B1020",
  "deny": "#DC2626",
  "require_approval": "#7C3AED",
  "rate_limit": "#64748B",
  "content_filter": "#64748B",
  "shadow": "#64748B",
}

const ACTION_LABELS: Record<string, string> = {
  "redact": "Redact",
  "deny": "Deny",
  "require_approval": "Require approval",
  "rate_limit": "Rate limit",
  "content_filter": "Content filter",
  "shadow": "Shadow",
}

export function PolicyActionsCard() {
  const [data, setData] = useState<PolicyActionStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const actions = await getPolicyActions(168)
        setData(actions)
      } catch (error) {
        console.error("Failed to fetch policy actions:", error)
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
        Policy Action Breakdown
      </h3>
      <div className="flex-1 flex flex-col gap-3 overflow-auto">
        {data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-[12px] text-muted-foreground">
            No policy actions in the selected period
          </div>
        ) : (
          data.map((item) => {
            const barWidth = (item.count / maxCount) * 100
            const color = ACTION_COLORS[item.action] || "#64748B"
            const label = ACTION_LABELS[item.action] || item.action
            return (
              <div key={item.action} className="flex items-center gap-3">
                <span className="w-[100px] text-[12px] text-muted-foreground truncate">
                  {label}
                </span>
                <div className="flex-1 h-[20px] bg-muted rounded-[4px] overflow-hidden">
                  <div
                    className="h-full rounded-[4px]"
                    style={{
                      width: `${barWidth}%`,
                      backgroundColor: color,
                    }}
                  />
                </div>
                <span className="w-[40px] text-[12px] font-medium text-right text-foreground">
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