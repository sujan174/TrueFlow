"use client"

import type { AuditLogRowType } from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import {
  formatRelativeTime,
  formatLatency,
  formatTokens,
  formatCost,
  getPolicyResultColor,
  getPolicyResultDisplay,
} from "@/lib/types/audit"
import { Eye } from "lucide-react"
import { Button } from "@/components/ui/button"

interface TraceTableProps {
  logs: AuditLogRowType[]
  onSelect: (log: AuditLogRowType) => void
}

export function TraceTable({ logs, onSelect }: TraceTableProps) {
  return (
    <div className="overflow-auto">
      <table className="w-full">
        <thead className="bg-muted/50 border-b sticky top-0 z-10">
          <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
            <th className="px-4 py-3 text-left">Time</th>
            <th className="px-4 py-3 text-left">Status</th>
            <th className="px-4 py-3 text-left">Method</th>
            <th className="px-4 py-3 text-left">Path</th>
            <th className="px-4 py-3 text-left">Model</th>
            <th className="px-4 py-3 text-left">Tokens</th>
            <th className="px-4 py-3 text-left">Cost</th>
            <th className="px-4 py-3 text-left">Latency</th>
            <th className="px-4 py-3 text-left">Policy</th>
            <th className="px-4 py-3 text-right">Actions</th>
          </tr>
        </thead>
        <tbody>
          {logs.map((log) => (
            <tr
              key={log.id}
              className="border-b last:border-0 hover:bg-muted/30 transition-colors"
            >
              <td className="px-4 py-3">
                <span className="text-sm text-foreground">
                  {formatRelativeTime(log.created_at)}
                </span>
              </td>
              <td className="px-4 py-3">
                <StatusBadge status={log.upstream_status} errorType={log.error_type} />
              </td>
              <td className="px-4 py-3">
                <Badge variant="outline" className="text-[10px] px-1.5">
                  {log.method}
                </Badge>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground truncate max-w-[200px] block">
                  {log.path}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-foreground">
                  {log.model || "—"}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground">
                  {formatTokens(log.prompt_tokens, log.completion_tokens)}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-foreground">
                  {formatCost(log.estimated_cost_usd)}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground">
                  {formatLatency(log.response_latency_ms)}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className={`px-2 py-0.5 rounded text-[10px] font-medium ${getPolicyResultColor(log.policy_result)}`}>
                  {getPolicyResultDisplay(log.policy_result)}
                </span>
              </td>
              <td className="px-4 py-3 text-right">
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => onSelect(log)}
                  title="View details"
                >
                  <Eye className="h-4 w-4" />
                </Button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function StatusBadge({ status, errorType }: { status: number | null; errorType: string | null }) {
  if (errorType) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-destructive/10 text-destructive">
        Error
      </span>
    )
  }
  if (status === null) {
    return (
      <span className="text-sm text-muted-foreground">—</span>
    )
  }
  const colorClass = status < 300
    ? "bg-success/10 text-success"
    : status < 400
    ? "bg-warning/10 text-warning"
    : "bg-destructive/10 text-destructive"

  return (
    <span className={`px-2 py-0.5 rounded text-[10px] font-medium ${colorClass}`}>
      {status}
    </span>
  )
}