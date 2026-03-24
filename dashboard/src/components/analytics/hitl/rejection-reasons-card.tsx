"use client"

import { useEffect, useState } from "react"
import { getHitlRejectionReasons, type RejectionReason } from "@/lib/api"

export function RejectionReasonsCard() {
  const [data, setData] = useState<RejectionReason[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const reasons = await getHitlRejectionReasons(168) // 7 days
        setData(reasons)
      } catch (error) {
        console.error("Failed to fetch rejection reasons:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Rejection Reasons
        </span>
        <span className="text-[9px] text-muted-foreground italic">
          Mock data
        </span>
      </div>

      {/* Body - Text-only list */}
      <div className="flex-1 p-4 flex flex-col">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No rejection data</span>
          </div>
        ) : (
          <div className="flex-1 flex flex-col justify-center gap-3">
            {data.map((item, index) => (
              <div key={index} className="flex items-center justify-between">
                <span className="text-[13px] text-muted-foreground">
                  {item.reason}
                </span>
                <span className="text-[13px] font-medium text-muted-foreground">
                  {item.percentage}%
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}