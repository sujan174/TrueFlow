"use client"

import { useEffect, useState } from "react"
import { getTopCachedQueries, type CachedQueryRow, formatNumber } from "@/lib/api"

function formatTimeAgo(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)

  if (diffMins < 60) return `${diffMins}m ago`
  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h ago`
  const diffDays = Math.floor(diffHours / 24)
  return `${diffDays}d ago`
}

function formatDuration(seconds: number): string {
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`
  return `${Math.floor(seconds / 86400)}d`
}

export function TopCachedQueriesTable() {
  const [data, setData] = useState<CachedQueryRow[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const queries = await getTopCachedQueries(25)
        setData(queries)
      } catch (error) {
        console.error("Failed to fetch top cached queries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          TOP CACHED QUERIES
        </span>
        <span className="text-[10px] text-muted-foreground">
          Showing 1–{data.length} of {data.length}
        </span>
      </div>

      {/* Column Headers */}
      <div className="h-[40px] px-4 flex items-center gap-3 border-b border">
        <span className="text-[10px] font-medium tracking-[1.5px] text-muted-foreground uppercase w-[200px]">
          QUERY
        </span>
        <span className="text-[10px] font-medium tracking-[1.5px] text-muted-foreground uppercase w-[80px]">
          MODEL
        </span>
        <span className="text-[10px] font-medium tracking-[1.5px] text-muted-foreground uppercase w-[60px]">
          HITS
        </span>
        <span className="text-[10px] font-medium tracking-[1.5px] text-muted-foreground uppercase w-[70px]">
          LAST HIT
        </span>
        <span className="text-[10px] font-medium tracking-[1.5px] text-muted-foreground uppercase w-[60px]">
          CACHE AGE
        </span>
        <span className="text-[10px] font-medium tracking-[1.5px] text-muted-foreground uppercase w-[60px]">
          EXPIRES
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="h-full flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground text-[12px]">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="h-full flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No cached queries</span>
          </div>
        ) : (
          <div className="flex flex-col">
            {data.slice(0, 5).map((row, idx) => (
              <div
                key={idx}
                className="h-[40px] px-4 flex items-center gap-3 border-b border last:border-b-0"
              >
                <span className="text-[10px] font-mono text-muted-foreground w-[200px] truncate">
                  {row.query_hash}
                </span>
                <span className="text-[10px] text-muted-foreground w-[80px] truncate">
                  {row.model || "unknown"}
                </span>
                <span className="text-[10px] font-semibold text-muted-foreground w-[60px]">
                  {formatNumber(row.hits)}
                </span>
                <span className="text-[10px] text-muted-foreground w-[70px]">
                  {formatTimeAgo(row.last_hit_at)}
                </span>
                <span className="text-[10px] text-muted-foreground w-[60px]">
                  {formatDuration(row.cache_age_seconds)}
                </span>
                <span className="text-[10px] text-muted-foreground w-[60px]">
                  {row.expires_in_seconds ? formatDuration(row.expires_in_seconds) : "—"}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}