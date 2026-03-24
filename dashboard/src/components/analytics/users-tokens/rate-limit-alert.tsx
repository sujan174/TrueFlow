"use client"

import { useEffect, useState } from "react"
import { getTokenAlerts, type TokenAlertsResponse } from "@/lib/api"

export function RateLimitAlert() {
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

  // Don't render if loading, no data, or no rate-limited tokens
  if (loading) {
    return (
      <div className="h-[124px] bg-card border border rounded-[14px] flex flex-col shadow-sm p-4">
        <div className="flex-1 flex items-center justify-center">
          <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
        </div>
      </div>
    )
  }

  if (!data || data.tokens_at_rate_limit === 0) {
    return (
      <div className="h-[124px] bg-card border border rounded-[14px] flex flex-col shadow-sm p-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            Rate Limit Status
          </span>
        </div>
        <div className="flex-1 flex items-center justify-center">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-success" />
            <span className="text-[12px] text-muted-foreground">All tokens healthy</span>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="h-[124px] bg-destructive/10 border border-destructive/30 rounded-[14px] flex flex-col shadow-sm p-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-destructive uppercase">
          Rate Limit Alert
        </span>
        <span className="text-[12px] font-bold text-destructive">
          {data.tokens_at_rate_limit} tokens
        </span>
      </div>

      {/* Token list */}
      <div className="flex-1 flex flex-col justify-center gap-1 overflow-hidden">
        {data.rate_limited_tokens.slice(0, 3).map((token, idx) => (
          <div key={idx} className="flex items-center justify-between">
            <span className="text-[11px] text-destructive truncate max-w-[140px]">
              {token.token_name || "Unknown"}
            </span>
            <span className="text-[11px] font-medium text-destructive">
              {token.percent.toFixed(0)}%
            </span>
          </div>
        ))}
        {data.rate_limited_tokens.length > 3 && (
          <span className="text-[10px] text-destructive">
            +{data.rate_limited_tokens.length - 3} more
          </span>
        )}
      </div>
    </div>
  )
}