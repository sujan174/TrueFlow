"use client"

import { useEffect, useState } from "react"
import { getBudgetBurnRate, formatCurrency, type BudgetBurnRate } from "@/lib/api"

export function BudgetBurnRateCard() {
  const [rate, setRate] = useState<BudgetBurnRate | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const data = await getBudgetBurnRate()
        setRate(data)
      } catch (error) {
        console.error("Failed to fetch budget burn rate:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="flex-1 bg-card border border rounded-[14px] flex flex-col shadow-sm">
        <div className="h-[44px] px-4 flex items-center border-b border">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            BUDGET BURN RATE
          </span>
        </div>
        <div className="flex-1 flex items-center justify-center">
          <div className="animate-pulse text-[12px] text-muted-foreground">Loading...</div>
        </div>
      </div>
    )
  }

  if (!rate) return null

  const percentUsed = Math.min(rate.percent_used, 100)
  const circumference = 2 * Math.PI * 40
  const strokeDashoffset = circumference - (percentUsed / 100) * circumference

  return (
    <div className="flex-1 bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          BUDGET BURN RATE
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex items-center gap-4">
        {/* Donut Chart */}
        <div className="relative w-[100px] h-[100px]">
          <svg className="w-full h-full -rotate-90" viewBox="0 0 100 100">
            {/* Background circle */}
            <circle
              cx="50"
              cy="50"
              r="40"
              fill="none"
              stroke="hsl(var(--muted))"
              strokeWidth="8"
            />
            {/* Progress circle */}
            <circle
              cx="50"
              cy="50"
              r="40"
              fill="none"
              stroke={rate.on_track ? "hsl(var(--color-success))" : "hsl(var(--destructive))"}
              strokeWidth="8"
              strokeLinecap="round"
              strokeDasharray={circumference}
              strokeDashoffset={strokeDashoffset}
              className="transition-all duration-500"
            />
          </svg>
          {/* Center text */}
          <div className="absolute inset-0 flex flex-col items-center justify-center">
            <span className="text-[20px] font-bold text-foreground">
              {percentUsed.toFixed(0)}%
            </span>
            <span className="text-[10px] text-muted-foreground">used</span>
          </div>
        </div>

        {/* Stats */}
        <div className="flex-1 flex flex-col gap-2">
          <div className="flex justify-between">
            <span className="text-[11px] text-muted-foreground">Days Elapsed</span>
            <span className="text-[11px] font-medium text-foreground">
              {rate.days_elapsed}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-[11px] text-muted-foreground">Days Remaining</span>
            <span className="text-[11px] font-medium text-foreground">
              {rate.days_remaining}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-[11px] text-muted-foreground">Daily Rate</span>
            <span className="text-[11px] font-medium text-foreground">
              {formatCurrency(rate.actual_per_day)}/day
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-[11px] text-muted-foreground">Target Rate</span>
            <span className="text-[11px] font-medium text-foreground">
              {formatCurrency(rate.needed_per_day)}/day
            </span>
          </div>

          {/* Status */}
          <div className="mt-1 flex items-center gap-1.5">
            <div
              className={`w-2 h-2 rounded-full ${
                rate.on_track ? "bg-success" : "bg-destructive"
              }`}
            />
            <span
              className={`text-[10px] font-medium ${
                rate.on_track ? "text-success" : "text-destructive"
              }`}
            >
              {rate.on_track ? "On Track" : "Over Budget Pace"}
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}