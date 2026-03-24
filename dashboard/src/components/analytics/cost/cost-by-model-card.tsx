"use client"

import { useEffect, useState } from "react"
import { getSpendByModel, formatCurrency, type SpendByDimension } from "@/lib/api"

export function CostByModelCard() {
  const [models, setModels] = useState<SpendByDimension[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getSpendByModel(168) // 7 days
        setModels(data)
      } catch (error) {
        console.error("Failed to fetch spend by model:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const maxCost = Math.max(...models.map((m) => m.total_cost_usd), 1)
  const totalCost = models.reduce((sum, m) => sum + m.total_cost_usd, 0)

  return (
    <div className="flex-1 bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          COST BY MODEL
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2.5">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-[12px] text-muted-foreground">Loading...</div>
          </div>
        ) : models.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {models.slice(0, 5).map((model, index) => {
              const barWidth = (model.total_cost_usd / maxCost) * 100
              const isActive = index === 0
              const percent = totalCost > 0
                ? ((model.total_cost_usd / totalCost) * 100).toFixed(0)
                : "0"

              return (
                <div key={model.dimension} className="flex flex-col gap-1">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span
                        className="text-[11px] text-muted-foreground truncate max-w-[100px]"
                        title={model.dimension}
                      >
                        {model.dimension}
                      </span>
                      <span className="text-[10px] text-muted-foreground">{percent}%</span>
                    </div>
                    <span className="text-[11px] font-medium text-foreground">
                      {formatCurrency(model.total_cost_usd)}
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
                {formatCurrency(totalCost)}
              </span>
            </div>
          </>
        )}
      </div>
    </div>
  )
}