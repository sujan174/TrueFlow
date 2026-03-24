"use client"

import { useEffect, useState } from "react"
import { getSpendByProvider, formatCurrency, type ProviderSpendStat } from "@/lib/api"

export function CostByProviderCard() {
  const [providers, setProviders] = useState<ProviderSpendStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getSpendByProvider(168) // 7 days
        setProviders(data)
      } catch (error) {
        console.error("Failed to fetch spend by provider:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const maxSpend = Math.max(...providers.map((p) => p.spend_usd), 1)
  const totalSpend = providers.reduce((sum, p) => sum + p.spend_usd, 0)

  return (
    <div className="flex-1 bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          COST BY PROVIDER
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2.5">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-[12px] text-muted-foreground">Loading...</div>
          </div>
        ) : providers.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {providers.slice(0, 5).map((provider, index) => {
              const barWidth = (provider.spend_usd / maxSpend) * 100
              const isActive = index === 0
              const percent = totalSpend > 0
                ? ((provider.spend_usd / totalSpend) * 100).toFixed(0)
                : "0"

              return (
                <div key={provider.provider} className="flex flex-col gap-1">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="text-[11px] text-muted-foreground capitalize">
                        {provider.provider}
                      </span>
                      <span className="text-[10px] text-muted-foreground">{percent}%</span>
                    </div>
                    <span className="text-[11px] font-medium text-foreground">
                      {formatCurrency(provider.spend_usd)}
                    </span>
                  </div>
                  <div className="h-2 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-2 rounded-full"
                      style={{
                        width: `${barWidth}%`,
                        backgroundColor: isActive ? "hsl(var(--primary))" : "hsl(var(--muted))",
                      }}
                    />
                  </div>
                </div>
              )
            })}

            {/* Footer */}
            <div className="mt-2 pt-2 border-t border-border flex items-center justify-between">
              <span className="text-[10px] text-muted-foreground">Total</span>
              <span className="text-[12px] font-semibold text-foreground">
                {formatCurrency(totalSpend)}
              </span>
            </div>
          </>
        )}
      </div>
    </div>
  )
}