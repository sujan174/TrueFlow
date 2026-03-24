"use client"

import { useEffect, useState } from "react"
import { getBudgetHealth, type BudgetHealthStatus } from "@/lib/api"

export function BudgetHealthStrip() {
  const [health, setHealth] = useState<BudgetHealthStatus | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getBudgetHealth()
        setHealth(data)
      } catch (error) {
        console.error("Failed to fetch budget health:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-[44px] bg-muted border border rounded-[10px] flex items-center justify-center">
        <div className="animate-pulse text-[12px] text-muted-foreground">Loading budget health...</div>
      </div>
    )
  }

  if (!health) return null

  const hasAlerts = health.tokens_above_80_percent > 0 || health.tokens_without_cap > 0

  if (!hasAlerts) {
    return (
      <div className="h-[44px] bg-success/10 border border-success/30 rounded-[10px] flex items-center px-4 gap-3">
        <span className="text-[16px]">✓</span>
        <span className="text-[12px] text-success font-medium">
          All {health.total_tokens} tokens are within budget limits
        </span>
      </div>
    )
  }

  return (
    <div className="h-[44px] bg-warning/20 border border-warning/30 rounded-[10px] flex items-center px-4 gap-4">
      <span className="text-[16px]">⚠</span>
      {health.tokens_above_80_percent > 0 && (
        <span className="text-[12px] text-warning font-medium">
          {health.tokens_above_80_percent} token{health.tokens_above_80_percent !== 1 ? "s" : ""} near cap (80%+)
        </span>
      )}
      {health.tokens_without_cap > 0 && (
        <span className="text-[12px] text-warning font-medium">
          {health.tokens_without_cap} token{health.tokens_without_cap !== 1 ? "s" : ""} without cap
        </span>
      )}
      <button className="ml-auto text-[11px] text-warning font-medium underline hover:no-underline">
        View Details
      </button>
    </div>
  )
}