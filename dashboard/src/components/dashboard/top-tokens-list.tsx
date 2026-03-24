import type { TokenSummary } from "@/lib/types/analytics"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

interface TopTokensListProps {
  tokens: TokenSummary[]
  loading?: boolean
}

function SkeletonRow() {
  return (
    <div className="flex items-center gap-3 h-8 px-2">
      <Skeleton className="w-[260px] h-4" />
      <Skeleton className="w-[120px] h-4" />
      <Skeleton className="w-[120px] h-4" />
      <Skeleton className="w-[120px] h-4" />
    </div>
  )
}

export function TopTokensList({ tokens, loading }: TopTokensListProps) {
  if (loading) {
    return (
      <div className="flex-1 flex flex-col gap-1.5">
        {[1, 2, 3].map((i) => (
          <SkeletonRow key={i} />
        ))}
      </div>
    )
  }

  if (tokens.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground text-xs">
        No token activity yet
      </div>
    )
  }

  const displayTokens = tokens.slice(0, 3).map((t) => ({
    token: t.token_id.replace(/^(tf_v1_)/, ""),
    requests: t.total_requests.toLocaleString(),
    violations: t.errors.toString(),
    spend: "$" + Math.round(t.total_requests * 0.033),
  }))

  return (
    <div className="flex-1 flex flex-col gap-1.5">
      {/* Column headers */}
      <div className="flex items-center gap-3">
        <span className="w-[260px] text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">TOKEN</span>
        <span className="w-[120px] text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">REQUESTS</span>
        <span className="w-[120px] text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">VIOLATIONS</span>
        <span className="w-[120px] text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">SPEND</span>
      </div>

      {/* Data rows */}
      {displayTokens.map((row, index) => (
        <div
          key={row.token}
          className={cn(
            "flex items-center gap-3 h-8 px-2 rounded-md",
            index % 2 === 0 && "bg-muted/50"
          )}
        >
          <span className="w-[260px] text-xs font-mono text-foreground">{row.token}</span>
          <span className="w-[120px] text-xs text-foreground">{row.requests}</span>
          <span className={cn(
            "w-[120px] text-xs",
            parseInt(row.violations) > 0 ? "text-destructive" : "text-foreground"
          )}>
            {row.violations}
          </span>
          <span className="w-[120px] text-xs text-foreground">{row.spend}</span>
        </div>
      ))}
    </div>
  )
}