"use client"

import { useEffect, useState } from "react"
import { getModelUsage, type ModelUsageStat, formatNumber, formatCurrency } from "@/lib/api"
import { Cpu } from "lucide-react"

export function ModelUsageCard() {
  const [models, setModels] = useState<ModelUsageStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getModelUsage(24)
        setModels(data)
      } catch (error) {
        console.error("Failed to fetch model usage:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Calculate max requests for bar scaling
  const maxRequests = Math.max(...models.map((m) => m.request_count), 1)

  return (
    <div className="flex-1 bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-200">
      {/* Header */}
      <div className="h-11 px-4 flex items-center justify-between border-b">
        <div className="flex items-center gap-2">
          <Cpu className="h-4 w-4 text-tokens" />
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            MODEL USAGE
          </span>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2.5">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : models.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-sm text-muted-foreground">No data available</span>
          </div>
        ) : (
          models.slice(0, 5).map((model, index) => {
            const barWidth = Math.max(40, (model.request_count / maxRequests) * 160)
            const isActive = index === 0

            return (
              <div key={model.model} className="flex flex-col gap-1">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted-foreground">{model.model}</span>
                  <span className="text-[10px] text-muted-foreground">
                    {formatNumber(model.request_count)} · {formatCurrency(model.cost_usd)}
                  </span>
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
          })
        )}
      </div>
    </div>
  )
}