"use client"

import { cn } from "@/lib/utils"

interface StatusBadgeProps {
  children: React.ReactNode
  variant?: "success" | "warning" | "error" | "info" | "neutral"
  pulse?: boolean
  className?: string
}

const variantStyles = {
  success: {
    badge: "bg-success/10 text-success border-success/20",
    dot: "bg-success",
  },
  warning: {
    badge: "bg-warning/10 text-warning border-warning/20",
    dot: "bg-warning",
  },
  error: {
    badge: "bg-error/10 text-error border-error/20",
    dot: "bg-error",
  },
  info: {
    badge: "bg-info/10 text-info border-info/20",
    dot: "bg-info",
  },
  neutral: {
    badge: "bg-muted text-muted-foreground border-muted",
    dot: "bg-muted-foreground",
  },
}

export function StatusBadge({
  children,
  variant = "neutral",
  pulse = false,
  className,
}: StatusBadgeProps) {
  const styles = variantStyles[variant]

  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border text-[11px] font-semibold tracking-wide",
        styles.badge,
        className
      )}
    >
      <span className="relative flex h-2 w-2">
        {pulse && (
          <span
            className={cn(
              "absolute inline-flex h-full w-full rounded-full opacity-75 animate-ping",
              styles.dot
            )}
          />
        )}
        <span
          className={cn(
            "relative inline-flex rounded-full h-2 w-2",
            styles.dot
          )}
        />
      </span>
      {children}
    </div>
  )
}