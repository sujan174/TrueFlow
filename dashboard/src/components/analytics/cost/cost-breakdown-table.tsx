"use client"

import { useEffect, useState } from "react"
import {
  getTokenSpendWithCaps,
  formatCurrency,
  formatNumber,
  type TokenSpendWithCap,
} from "@/lib/api"

export function CostBreakdownTable() {
  const [tokens, setTokens] = useState<TokenSpendWithCap[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getTokenSpendWithCaps(168) // 7 days
        setTokens(Array.isArray(data) ? data : [])
      } catch (error) {
        console.error("Failed to fetch token spend:", error)
        setTokens([])
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const getCapUsedColor = (percent: number | null) => {
    if (percent === null) return "text-muted-foreground"
    if (percent >= 80) return "text-destructive"
    if (percent >= 60) return "text-warning"
    return "text-success"
  }

  const getCapUsedBg = (percent: number | null) => {
    if (percent === null) return "bg-muted"
    if (percent >= 80) return "bg-destructive/10"
    if (percent >= 60) return "bg-warning/20"
    return "bg-success/10"
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Cost Breakdown by Token
        </span>
        <span className="text-[11px] text-muted-foreground">
          Last 7 days
        </span>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="h-full flex items-center justify-center">
            <div className="animate-pulse text-[12px] text-muted-foreground">Loading...</div>
          </div>
        ) : tokens.length === 0 ? (
          <div className="h-full flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No tokens with spend data</span>
          </div>
        ) : (
          <table className="w-full">
            <thead className="sticky top-0 bg-muted/50 border-b border">
              <tr>
                <th className="text-left px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  Token
                </th>
                <th className="text-left px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  Provider
                </th>
                <th className="text-right px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  Spend
                </th>
                <th className="text-right px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  Requests
                </th>
                <th className="text-right px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  Cost/1K
                </th>
                <th className="text-right px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  Cap
                </th>
                <th className="text-right px-4 py-2.5 text-[10px] font-semibold tracking-[0.5px] text-muted-foreground uppercase">
                  % Used
                </th>
              </tr>
            </thead>
            <tbody>
              {tokens.map((token) => (
                <tr
                  key={token.token_id}
                  className="border-b border-border hover:bg-muted/50 transition-colors"
                >
                  <td className="px-4 py-3">
                    <div className="flex flex-col">
                      <span className="text-[12px] font-medium text-foreground">
                        {token.token_name}
                      </span>
                      <span className="text-[10px] text-muted-foreground">
                        {token.token_id.slice(0, 12)}...
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-[12px] text-muted-foreground capitalize">
                      {token.provider}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-[12px] font-medium text-foreground">
                      {formatCurrency(token.total_spend_usd)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-[12px] text-muted-foreground">
                      {formatNumber(token.request_count)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-[12px] text-muted-foreground">
                      ${token.cost_per_1k.toFixed(2)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-[12px] text-muted-foreground">
                      {token.spend_cap_usd ? formatCurrency(token.spend_cap_usd) : "—"}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    {token.percent_cap_used !== null ? (
                      <span
                        className={`inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium ${getCapUsedColor(
                          token.percent_cap_used
                        )} ${getCapUsedBg(token.percent_cap_used)}`}
                      >
                        {token.percent_cap_used.toFixed(0)}%
                      </span>
                    ) : (
                      <span className="text-[12px] text-muted-foreground">—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}