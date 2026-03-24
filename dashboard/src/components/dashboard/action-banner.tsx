"use client"

import { useState } from "react"

interface ActionBannerProps {
  hitlApprovals?: number
  spendCapsWarning?: number
  circuitBreakersOpen?: number
}

export function ActionBanner({
  hitlApprovals = 3,
  spendCapsWarning = 2,
  circuitBreakersOpen = 1,
}: ActionBannerProps) {
  const [dismissed, setDismissed] = useState(false)

  const hasAction = hitlApprovals > 0 || spendCapsWarning > 0 || circuitBreakersOpen > 0
  if (!hasAction || dismissed) return null

  return (
    <div className="h-[46px] flex items-center justify-between bg-warning/10 border border-warning/30 rounded-xl py-3 px-4 shadow-sm">
      <div className="flex items-center gap-3">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-warning uppercase">
          ACTION REQUIRED
        </span>
        <div className="flex items-center gap-3">
          {hitlApprovals > 0 && (
            <div className="flex items-center gap-[6px]">
              <div className="w-[6px] h-[6px] rounded-full bg-warning" />
              <span className="text-[12px] font-normal text-warning">HITL approvals: {hitlApprovals}</span>
            </div>
          )}
          {spendCapsWarning > 0 && (
            <div className="flex items-center gap-[6px]">
              <div className="w-[6px] h-[6px] rounded-full bg-warning" />
              <span className="text-[12px] font-normal text-warning">Spend caps &gt;80%: {spendCapsWarning}</span>
            </div>
          )}
          {circuitBreakersOpen > 0 && (
            <div className="flex items-center gap-[6px]">
              <div className="w-[6px] h-[6px] rounded-full bg-destructive" />
              <span className="text-[12px] font-normal text-warning">Circuit breakers open: {circuitBreakersOpen}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}