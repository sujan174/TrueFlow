"use client"

import { useEffect, useState } from "react"
import { getModelErrorRates, type ModelErrorRate } from "@/lib/api"

// Color mapping for error rates
function getErrorRateColor(errorRate: number): string {
  if (errorRate > 1) return "#F43F5E" // Red - high error
  if (errorRate > 0.5) return "#94A3B8" // Grey - medium
  return "#10B981" // Green - low
}

export function ErrorRateCard() {
  const [data, setData] = useState<ModelErrorRate[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const rates = await getModelErrorRates(168)
        setData(rates)
      } catch (error) {
        console.error("Failed to fetch model error rates:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const maxErrorRate = Math.max(...data.map((d) => d.error_rate), 1)

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Error Rate by Model
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col justify-center gap-3 overflow-auto">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground text-[11px]">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No error data</span>
          </div>
        ) : (
          data.slice(0, 5).map((model, idx) => (
            <div key={idx} className="flex items-center gap-3">
              {/* Model name */}
              <div className="w-[140px] text-[11px] font-mono text-muted-foreground truncate">
                {model.model}
              </div>

              {/* Bar */}
              <div className="flex-1 h-3 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-300"
                  style={{
                    width: `${(model.error_rate / maxErrorRate) * 100}%`,
                    backgroundColor: getErrorRateColor(model.error_rate),
                  }}
                />
              </div>

              {/* Percentage */}
              <div className="w-[50px] text-right text-[11px] font-semibold">
                <span style={{ color: getErrorRateColor(model.error_rate) }}>
                  {model.error_rate.toFixed(2)}%
                </span>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}