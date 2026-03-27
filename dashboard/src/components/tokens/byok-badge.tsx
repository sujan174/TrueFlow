"use client"

import { KeyRound } from "lucide-react"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"

interface ByokBadgeProps {
  showTooltip?: boolean
}

export function ByokBadge({ showTooltip = true }: ByokBadgeProps) {
  const badge = (
    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium rounded border border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-950/50 dark:text-amber-300">
      <KeyRound className="h-3 w-3" />
      BYOK
    </span>
  )

  if (!showTooltip) {
    return badge
  }

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger>
          {badge}
        </TooltipTrigger>
        <TooltipContent>
          <p className="text-xs">Bring Your Own Key - API key provided at runtime</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}