"use client"

import { useEffect, useState } from "react"
import { getErrorLogs, type ErrorLogRow } from "@/lib/api"

function formatTime(dateString: string): string {
  const date = new Date(dateString)
  return date.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  })
}

function formatLatency(ms: number | null): string {
  if (ms === null) return "—"
  return ms >= 1000 ? `${ms.toLocaleString()} ms` : `${ms} ms`
}

function formatErrorType(type: string | null): string {
  if (!type) return "—"
  return type.toLowerCase()
}

function formatHttpCode(status: number | null): string {
  if (status === null) return "—"
  return status.toString()
}

export function ErrorLogsTable() {
  const [data, setData] = useState<ErrorLogRow[]>([])
  const [loading, setLoading] = useState(true)
  const [showRecentOnly, setShowRecentOnly] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const logs = await getErrorLogs(showRecentOnly ? 50 : 100)
        setData(logs)
      } catch (error) {
        console.error("Failed to fetch error logs:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [showRecentOnly])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading error logs...</div>
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex flex-col items-center justify-center shadow-sm">
        <span className="text-[14px] text-muted-foreground">No error logs available</span>
      </div>
    )
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-12 px-4 flex items-center justify-between border-b border">
        <span className="text-[12px] font-semibold text-foreground">Error Log</span>

        {/* Toggle */}
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={showRecentOnly}
            onChange={(e) => setShowRecentOnly(e.target.checked)}
            className="w-4 h-4 rounded border text-foreground focus:ring-0"
          />
          <span className="text-[11px] text-muted-foreground">Showing recent errors</span>
        </label>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        <table className="w-full">
          <thead className="bg-muted/50 sticky top-0">
            <tr>
              <th className="text-left px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Time
              </th>
              <th className="text-left px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Token
              </th>
              <th className="text-left px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Model
              </th>
              <th className="text-left px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Type
              </th>
              <th className="text-left px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                HTTP
              </th>
              <th className="text-left px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Latency
              </th>
              <th className="text-right px-4 py-2 text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Actions
              </th>
            </tr>
          </thead>
          <tbody>
            {data.map((row, index) => (
              <tr key={row.request_id || index} className="border-t border hover:bg-muted/50">
                <td className="px-4 py-3 text-[12px] text-foreground font-mono">
                  {formatTime(row.created_at)}
                </td>
                <td className="px-4 py-3 text-[12px] text-foreground max-w-[150px] truncate">
                  {row.token_name || row.token_id.slice(0, 8)}
                </td>
                <td className="px-4 py-3 text-[12px] text-foreground max-w-[150px] truncate">
                  {row.model || "—"}
                </td>
                <td className="px-4 py-3 text-[12px] text-muted-foreground">
                  {formatErrorType(row.error_type)}
                </td>
                <td className="px-4 py-3 text-[12px] text-muted-foreground">
                  {formatHttpCode(row.upstream_status)}
                </td>
                <td className="px-4 py-3 text-[12px] text-muted-foreground">
                  {formatLatency(row.response_latency_ms)}
                </td>
                <td className="px-4 py-3 text-right">
                  <div className="flex items-center justify-end gap-2">
                    <button className="px-2.5 py-1 bg-card border border rounded text-[10px] font-medium text-foreground hover:bg-muted/50">
                      Retry
                    </button>
                    <button className="px-2.5 py-1 bg-card border border rounded text-[10px] font-medium text-foreground hover:bg-muted/50">
                      Inspect
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}