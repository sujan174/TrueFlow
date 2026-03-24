"use client"

import { useEffect, useState } from "react"
import { getLatencyByProvider, type ProviderLatencyStat } from "@/lib/api"
import { Timer } from "lucide-react"

export function LatencyByProviderCard() {
  const [providers, setProviders] = useState<ProviderLatencyStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getLatencyByProvider(24)
        setProviders(data)
      } catch (error) {
        console.error("Failed to fetch latency by provider:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Calculate max latency for bar scaling
  const maxLatency = Math.max(...providers.map((p) => p.latency_ms), 1)

  return (
    <div className="flex-1 bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-200">
      {/* Header */}
      <div className="h-11 px-4 flex items-center justify-between border-b">
        <div className="flex items-center gap-2">
          <Timer className="h-4 w-4 text-alerts" />
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            LATENCY BY PROVIDER
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
          providers.slice(0, 5).map((provider, index) => {
            const barWidth = Math.max(40, (provider.latency_ms / maxLatency) * 180)
            const isHighest = index === 0 && providers.length > 1

            return (
              <div key={provider.provider} className="flex flex-col gap-1">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted-foreground capitalize">{provider.provider}</span>
                  <span className="text-[10px] text-muted-foreground">{Math.round(provider.latency_ms)}ms</span>
                </div>
                <div className="h-2 bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-2 rounded-full transition-all duration-300"
                    style={{
                      width: `${barWidth}px`,
                      backgroundColor: isHighest ? "hsl(var(--destructive))" : "hsl(var(--muted-foreground) / 0.3)",
                    }}
                  />
                </div>
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}