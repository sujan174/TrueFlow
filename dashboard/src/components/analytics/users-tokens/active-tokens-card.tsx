"use client"

import { useEffect, useState } from "react"
import { getTokenAlerts, type TokenAlertsResponse } from "@/lib/api"

export function ActiveTokensCard() {
  const [data, setData] = useState<TokenAlertsResponse | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const alerts = await getTokenAlerts(24)
        setData(alerts)
      } catch (error) {
        console.error("Failed to fetch token alerts:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const percentage = data?.token_limit && data.token_limit > 0
    ? Math.min((data.active_tokens / data.token_limit) * 100, 100)
    : 0

  return (
    <div className="h-[124px] bg-card border border rounded-[14px] flex flex-col shadow-sm p-4">
      {loading ? (
        <div className="flex-1 flex items-center justify-center">
          <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
        </div>
      ) : (
        <>
          {/* Header */}
          <div className="flex items-center justify-between mb-3">
            <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
              Active Tokens
            </span>
            <span className="text-[12px] font-bold text-foreground">
              {data?.active_tokens ?? 0}
            </span>
          </div>

          {/* Progress bar */}
          <div className="flex-1 flex flex-col justify-center gap-2">
            <div className="h-2 bg-muted rounded-full overflow-hidden">
              <div
                className="h-full bg-success rounded-full transition-all duration-300"
                style={{ width: `${percentage}%` }}
              />
            </div>
            {data?.token_limit && data.token_limit > 0 && (
              <span className="text-[10px] text-muted-foreground">
                of {data.token_limit} limit
              </span>
            )}
            {(!data?.token_limit || data.token_limit === 0) && (
              <span className="text-[10px] text-muted-foreground">
                No limit configured
              </span>
            )}
          </div>
        </>
      )}
    </div>
  )
}