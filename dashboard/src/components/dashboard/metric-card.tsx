import { cn } from "@/lib/utils"
import { type LucideIcon } from "lucide-react"
import Link from "next/link"

interface MetricCardProps {
  label: string
  value: string
  valueColor?: string
  icon?: LucideIcon
  variant?: "default" | "requests" | "latency" | "spend" | "alerts" | "tokens"
  href?: string
}

const variantStyles = {
  default: {
    bg: "bg-transparent",
    icon: "text-muted-foreground",
    dot: "bg-muted-foreground",
  },
  requests: {
    bg: "metric-bg-requests",
    icon: "text-requests",
    dot: "bg-requests",
  },
  latency: {
    bg: "bg-transparent",
    icon: "text-muted-foreground",
    dot: "bg-muted-foreground",
  },
  spend: {
    bg: "metric-bg-spend",
    icon: "text-spend",
    dot: "bg-spend",
  },
  alerts: {
    bg: "metric-bg-alerts",
    icon: "text-alerts",
    dot: "bg-alerts",
  },
  tokens: {
    bg: "metric-bg-tokens",
    icon: "text-tokens",
    dot: "bg-tokens",
  },
}

export function MetricCard({ label, value, valueColor, icon: Icon, variant = "default", href }: MetricCardProps) {
  const styles = variantStyles[variant]

  const content = (
    <div className={cn("flex flex-col gap-1 transition-all duration-200", href && "cursor-pointer hover:opacity-80")}>
      <div className="flex items-center gap-1.5">
        {Icon && <Icon className={cn("h-3.5 w-3.5", styles.icon)} />}
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          {label}
        </span>
      </div>
      <span
        className={cn(
          "text-xl font-bold transition-all duration-300",
          valueColor || "text-foreground"
        )}
      >
        {value}
      </span>
    </div>
  )

  if (href) {
    return (
      <Link href={href} className="block">
        {content}
      </Link>
    )
  }

  return content
}