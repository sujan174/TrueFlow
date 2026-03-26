"use client"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import type { SessionStatus } from "@/lib/types/session"
import { getSessionStatusDisplay, getSessionStatusVariant } from "@/lib/types/session"

interface SessionStatusBadgeProps {
  status: SessionStatus
  className?: string
}

export function SessionStatusBadge({ status, className }: SessionStatusBadgeProps) {
  const variant = getSessionStatusVariant(status)

  // Custom styling for paused status (yellow/amber)
  const pausedStyles = status === "paused"
    ? "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400"
    : ""

  return (
    <Badge
      variant={variant}
      className={cn("text-[10px]", pausedStyles, className)}
    >
      {getSessionStatusDisplay(status)}
    </Badge>
  )
}