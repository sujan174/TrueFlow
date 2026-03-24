"use client"

import { cn } from "@/lib/utils"
import { Skeleton } from "@/components/ui/skeleton"
import { ArrowUp, ArrowDown, Minus, type LucideIcon } from "lucide-react"

interface KPICardProps {
  label: string
  value: string | number
  trend?: {
    value: number
    label?: string
  }
  icon?: LucideIcon
  progress?: number // 0-100 for progress bar
  sparklineData?: number[] // Array of values for mini sparkline
  loading?: boolean
  className?: string
  variant?: "default" | "primary" | "success" | "warning" | "error" | "requests" | "spend" | "tokens" | "alerts"
  glow?: boolean // Enable glow effect
}

const variantStyles = {
  default: {
    card: "bg-card border-border",
    label: "text-muted-foreground",
    value: "text-foreground",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-muted-foreground",
    progressBg: "bg-muted",
    progressFill: "bg-primary",
  },
  primary: {
    card: "bg-primary/5 border-primary/20",
    label: "text-primary",
    value: "text-primary",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-primary",
    progressBg: "bg-primary/20",
    progressFill: "bg-primary",
  },
  success: {
    card: "bg-success/5 border-success/20",
    label: "text-success",
    value: "text-success",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-success",
    progressBg: "bg-success/20",
    progressFill: "bg-success",
  },
  warning: {
    card: "bg-warning/5 border-warning/20",
    label: "text-warning",
    value: "text-warning",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-warning",
    progressBg: "bg-warning/20",
    progressFill: "bg-warning",
  },
  error: {
    card: "bg-error/5 border-error/20",
    label: "text-error",
    value: "text-error",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-error",
    progressBg: "bg-error/20",
    progressFill: "bg-error",
  },
  requests: {
    card: "metric-bg-requests border-requests/20",
    label: "text-requests",
    value: "text-foreground",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-requests",
    progressBg: "bg-requests/20",
    progressFill: "bg-requests",
  },
  spend: {
    card: "metric-bg-spend border-spend/20",
    label: "text-spend",
    value: "text-foreground",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-spend",
    progressBg: "bg-spend/20",
    progressFill: "bg-spend",
  },
  tokens: {
    card: "metric-bg-tokens border-tokens/20",
    label: "text-tokens",
    value: "text-foreground",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-tokens",
    progressBg: "bg-tokens/20",
    progressFill: "bg-tokens",
  },
  alerts: {
    card: "metric-bg-alerts border-alerts/20",
    label: "text-alerts",
    value: "text-foreground",
    trend: {
      up: "text-success",
      down: "text-error",
      neutral: "text-muted-foreground",
    },
    icon: "text-alerts",
    progressBg: "bg-alerts/20",
    progressFill: "bg-alerts",
  },
}

// Mini sparkline component
function MiniSparkline({ data, color }: { data?: number[]; color: string }) {
  if (!data || data.length === 0) return null

  const max = Math.max(...data)
  const heights = data.map((v) => Math.max(4, (v / max) * 16))

  // Map color class to CSS variable
  const colorMap: Record<string, string> = {
    'text-requests': 'var(--color-requests)',
    'text-spend': 'var(--color-spend)',
    'text-alerts': 'var(--color-alerts)',
    'text-tokens': 'var(--color-tokens)',
    'text-primary': 'var(--primary)',
    'text-success': 'var(--color-success)',
    'text-muted-foreground': 'var(--muted-foreground)',
  }

  const bgColor = colorMap[color] || 'var(--muted-foreground)'

  return (
    <div className="flex items-end gap-0.5 h-5">
      {heights.map((height, index) => (
        <div
          key={index}
          className="w-1 rounded-[1px] transition-all duration-200"
          style={{
            height: `${height}px`,
            backgroundColor: bgColor,
            opacity: 0.6 + (index / heights.length) * 0.4,
          }}
        />
      ))}
    </div>
  )
}

export function KPICard({
  label,
  value,
  trend,
  icon: Icon,
  progress,
  sparklineData,
  loading = false,
  className,
  variant = "default",
  glow = false,
}: KPICardProps) {
  const styles = variantStyles[variant]

  if (loading) {
    return (
      <div className={cn("rounded-xl border p-4", styles.card, className)}>
        <div className="flex items-center justify-between mb-2">
          <Skeleton className="h-3 w-20" />
          {Icon && <Skeleton className="h-5 w-5 rounded-sm" />}
        </div>
        <Skeleton className="h-8 w-24 mb-2" />
        {progress !== undefined && <Skeleton className="h-1.5 w-full rounded-full" />}
        {trend !== undefined && <Skeleton className="h-3 w-16 mt-2" />}
      </div>
    )
  }

  return (
    <div
      className={cn(
        "rounded-xl border p-4 transition-all duration-200 hover:shadow-md hover:border-primary/20 hover:scale-[1.01]",
        styles.card,
        glow && (variant === "success" ? "glow-success" : variant === "requests" ? "glow-requests" : variant === "primary" ? "glow-primary" : ""),
        className
      )}
    >
      <div className="flex items-center justify-between mb-1.5">
        <span className={cn("text-[10px] font-semibold tracking-[1.5px] uppercase", styles.label)}>
          {label}
        </span>
        <div className="flex items-center gap-2">
          {sparklineData && <MiniSparkline data={sparklineData} color={styles.icon} />}
          {Icon && <Icon className={cn("h-4 w-4", styles.icon)} />}
        </div>
      </div>
      <div className={cn("text-2xl font-bold tracking-tight", styles.value)}>
        {value}
      </div>

      {/* Progress bar */}
      {progress !== undefined && (
        <div className={cn("h-1.5 rounded-full mt-2", styles.progressBg)}>
          <div
            className={cn("h-full rounded-full transition-all duration-500", styles.progressFill)}
            style={{ width: `${Math.min(100, Math.max(0, progress))}%` }}
          />
        </div>
      )}

      {trend !== undefined && (
        <div className="flex items-center gap-1 mt-1.5">
          {trend.value > 0 ? (
            <>
              <ArrowUp className="h-3 w-3" />
              <span className={cn("text-xs font-medium", styles.trend.up)}>
                +{trend.value.toFixed(1)}%
              </span>
            </>
          ) : trend.value < 0 ? (
            <>
              <ArrowDown className="h-3 w-3" />
              <span className={cn("text-xs font-medium", styles.trend.down)}>
                {trend.value.toFixed(1)}%
              </span>
            </>
          ) : (
            <>
              <Minus className="h-3 w-3" />
              <span className={cn("text-xs font-medium", styles.trend.neutral)}>
                0%
              </span>
            </>
          )}
          {trend.label && (
            <span className="text-[10px] text-muted-foreground ml-0.5">
              {trend.label}
            </span>
          )}
        </div>
      )}
    </div>
  )
}