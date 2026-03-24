import type { SpendCap } from "@/lib/types/analytics"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

interface SpendCapsWidgetProps {
  caps: SpendCap[]
  loading?: boolean
}

function SkeletonRow() {
  return (
    <div className="flex items-center gap-3 h-8">
      <Skeleton className="w-[140px] h-4" />
      <Skeleton className="w-[40px] h-4" />
    </div>
  )
}

export function SpendCapsWidget({ caps, loading }: SpendCapsWidgetProps) {
  if (loading) {
    return (
      <div className="flex-1 flex flex-col gap-1.5">
        {[1, 2, 3].map((i) => (
          <SkeletonRow key={i} />
        ))}
      </div>
    )
  }

  if (caps.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground text-xs">
        No spend caps configured
      </div>
    )
  }

  const displayCaps = caps.slice(0, 3).map((c) => {
    const percent = Math.round(((c.spend_used_usd || 0) / c.spend_cap_usd) * 100)
    return {
      token: c.token_name,
      percent,
      variant: percent >= 95 ? "error" : "warning" as const,
    }
  })

  return (
    <div className="flex-1 flex flex-col gap-1.5">
      {displayCaps.map((cap) => (
        <div key={cap.token} className="flex items-center gap-3 h-8">
          <span className="w-[140px] text-xs font-mono text-foreground truncate">{cap.token}</span>
          <span
            className={cn(
              "w-[40px] text-xs font-semibold",
              cap.variant === "error" ? "text-destructive" : "text-warning"
            )}
          >
            {cap.percent}%
          </span>
        </div>
      ))}
    </div>
  )
}