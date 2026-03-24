"use client"

import { useEffect, useState } from "react"
import { getSecuritySummary, type SecuritySummaryStats, formatNumber } from "@/lib/api"

export function SecurityKpiRibbon() {
  const [data, setData] = useState<SecuritySummaryStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const summary = await getSecuritySummary(168)
        setData(summary)
      } catch (error) {
        console.error("Failed to fetch security summary:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-[72px] bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading security stats...</div>
      </div>
    )
  }

  const piiRedactions = data?.pii_redactions ?? 0
  const guardrailBlocks = data?.guardrail_blocks ?? 0
  const shadowViolations = data?.shadow_violations ?? 0
  const externalBlocks = data?.external_blocks ?? 0

  return (
    <div className="h-[72px] bg-card border border rounded-[14px] flex items-center px-6 gap-8 shadow-sm">
      {/* PII Redactions */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          PII Redactions
        </span>
        <span className="text-[20px] font-semibold text-foreground">
          {formatNumber(piiRedactions)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Guardrail Blocks - Red value */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Guardrail Blocks
        </span>
        <span className="text-[20px] font-semibold text-destructive">
          {formatNumber(guardrailBlocks)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* Shadow Violations */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Shadow Violations
        </span>
        <span className="text-[20px] font-semibold text-foreground">
          {formatNumber(shadowViolations)}
        </span>
      </div>

      <div className="w-[1px] h-[40px] bg-border" />

      {/* External Blocks */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          External Blocks
        </span>
        <span className="text-[20px] font-semibold text-foreground">
          {formatNumber(externalBlocks)}
        </span>
      </div>
    </div>
  )
}