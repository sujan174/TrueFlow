"use client"

import { useEffect, useState } from "react"
import { getSpendByProvider, type ProviderSpendStat, formatCurrency } from "@/lib/api"
import { DollarSign } from "lucide-react"

export function SpendByProviderCard() {
  const [providers, setProviders] = useState<ProviderSpendStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getSpendByProvider(24)
        setProviders(data)
      } catch (error) {
        console.error("Failed to fetch spend by provider:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Calculate max spend for bar scaling
  const maxSpend = Math.max(...providers.map((p) => p.spend_usd), 1)

  // Calculate totals for footer
  const totalSpend = providers.reduce((sum, p) => sum + p.spend_usd, 0)
  const projectedMonthly = totalSpend * 30

  return (
    <div className="flex-1 bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-200">
      {/* Header */}
      <div className="h-11 px-4 flex items-center justify-between border-b">
        <div className="flex items-center gap-2">
          <DollarSign className="h-4 w-4 text-spend" />
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            SPEND BY PROVIDER
          </span>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2.5">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : providers.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-sm text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {providers.slice(0, 5).map((provider, index) => {
              const barWidth = Math.max(40, (provider.spend_usd / maxSpend) * 170)
              const isActive = index === 0

              return (
                <div key={provider.provider} className="flex flex-col gap-1">
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-muted-foreground capitalize">{provider.provider}</span>
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-muted-foreground">{formatCurrency(provider.spend_usd)}</span>
                      <span className="text-[10px] text-muted-foreground">${provider.rate_per_1k.toFixed(2)}/1k</span>
                    </div>
                  </div>
                  <div className="h-2 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-2 rounded-full transition-all duration-300"
                      style={{
                        width: `${barWidth}px`,
                        backgroundColor: isActive ? "hsl(var(--primary))" : "hsl(var(--muted-foreground) / 0.3)",
                      }}
                    />
                  </div>
                </div>
              )
            })}

            {/* Footer */}
            <div className="flex items-center justify-between mt-2">
              <span className="text-[10px] text-muted-foreground">
                Projected ${projectedMonthly >= 1000 ? `${(projectedMonthly / 1000).toFixed(1)}k` : projectedMonthly.toFixed(0)}/mo
              </span>
              <span className="text-[10px] text-muted-foreground">Cap $5k</span>
            </div>
          </>
        )}
      </div>
    </div>
  )
}