"use client"

import { useEffect, useState } from "react"
import { getAuditLogs, type AuditLogRow, formatLatency, formatCurrency } from "@/lib/api"

function StatusBadge({ status }: { status: number | null }) {
  if (!status) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-muted text-muted-foreground">
        N/A
      </span>
    )
  }

  if (status >= 200 && status < 300) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-success/10 text-success">
        {status} OK
      </span>
    )
  }

  if (status === 429) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-warning/20 text-warning">
        429 RL
      </span>
    )
  }

  if (status >= 500) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-destructive/10 text-destructive">
        {status}
      </span>
    )
  }

  return (
    <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-muted text-muted-foreground">
      {status}
    </span>
  )
}

function PolicyResultBadge({ result }: { result: string }) {
  if (result.startsWith("Allow")) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-success/10 text-success">
        Passed
      </span>
    )
  }

  if (result.startsWith("ShadowDeny")) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-warning/20 text-warning">
        Throttled
      </span>
    )
  }

  if (result.startsWith("Deny")) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-destructive/10 text-destructive">
        Blocked
      </span>
    )
  }

  if (result.startsWith("Hitl")) {
    return (
      <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-chart-5/20 text-chart-5">
        HITL
      </span>
    )
  }

  return (
    <span className="px-2 py-0.5 rounded text-[10px] font-medium bg-muted text-muted-foreground">
      {result}
    </span>
  )
}

function LatencyCell({ ms }: { ms: number }) {
  if (ms > 1000) {
    return (
      <span className="text-[12px] font-semibold text-warning">
        {formatLatency(ms)}
      </span>
    )
  }
  return (
    <span className="text-[12px] text-foreground">
      {formatLatency(ms)}
    </span>
  )
}

export function RequestLogTable() {
  const [data, setData] = useState<AuditLogRow[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const logs = await getAuditLogs(50)
        setData(logs)
      } catch (error) {
        console.error("Failed to fetch audit logs:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <div className="flex flex-col gap-0.5">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            REQUEST LOG
          </span>
          <span className="text-[12px] font-bold text-foreground">
            Recent {data.length} requests
          </span>
        </div>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="h-full flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="h-full flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No requests yet</span>
          </div>
        ) : (
          <table className="w-full">
            <thead className="sticky top-0 bg-muted/50">
              <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                <th className="px-4 py-2 text-left">Time</th>
                <th className="px-4 py-2 text-left">Request ID</th>
                <th className="px-4 py-2 text-left">Model</th>
                <th className="px-4 py-2 text-left">Status</th>
                <th className="px-4 py-2 text-left">Policy</th>
                <th className="px-4 py-2 text-right">Latency</th>
                <th className="px-4 py-2 text-right">Cost</th>
              </tr>
            </thead>
            <tbody>
              {data.map((row, idx) => (
                <tr key={row.request_id || idx} className="border-t border hover:bg-muted/50">
                  <td className="px-4 py-2 text-[11px] text-muted-foreground whitespace-nowrap">
                    {new Date(row.created_at).toLocaleTimeString("en-US", {
                      hour: "numeric",
                      minute: "2-digit",
                    })}
                  </td>
                  <td className="px-4 py-2 text-[11px] text-muted-foreground font-mono max-w-[120px] truncate">
                    {row.request_id.slice(0, 8)}...
                  </td>
                  <td className="px-4 py-2 text-[11px] text-foreground max-w-[150px] truncate">
                    {row.model || "N/A"}
                  </td>
                  <td className="px-4 py-2">
                    <StatusBadge status={row.upstream_status} />
                  </td>
                  <td className="px-4 py-2">
                    <PolicyResultBadge result={row.policy_result} />
                  </td>
                  <td className="px-4 py-2 text-right">
                    <LatencyCell ms={row.response_latency_ms} />
                  </td>
                  <td className="px-4 py-2 text-right text-[11px] text-foreground">
                    {row.estimated_cost_usd != null ? formatCurrency(row.estimated_cost_usd) : "-"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}