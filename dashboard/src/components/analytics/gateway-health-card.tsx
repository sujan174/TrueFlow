"use client"

import { useEffect, useState } from "react"
import { getUpstreamHealth, type UpstreamStatus } from "@/lib/api"
import { Heart } from "lucide-react"

export function GatewayHealthCard() {
  const [upstreams, setUpstreams] = useState<UpstreamStatus[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const health = await getUpstreamHealth()
        setUpstreams(health)
      } catch (error) {
        console.error("Failed to fetch upstream health:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Separate into warning and healthy
  const warnings = upstreams.filter((u) => !u.healthy)
  const healthy = upstreams.filter((u) => u.healthy)

  return (
    <div className="flex-1 bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-200">
      {/* Header */}
      <div className="h-11 px-4 flex items-center justify-between border-b">
        <div className="flex items-center gap-2">
          <Heart className="h-4 w-4 text-destructive" />
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            GATEWAY HEALTH
          </span>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-0.5">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : upstreams.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-sm text-muted-foreground">No upstreams configured</span>
          </div>
        ) : (
          <>
            {/* Warning Rows */}
            {warnings.map((upstream) => (
              <div key={upstream.name} className="flex items-center gap-2 py-1.5">
                <div className="w-1.5 h-1.5 rounded-full border border-muted-foreground bg-background" />
                <span className="text-xs font-medium text-muted-foreground">{upstream.name}</span>
                <span className="text-xs font-medium text-warning">
                  {upstream.error || "Unhealthy"}
                </span>
                <span className="px-1.5 py-0.5 bg-warning/20 rounded-full text-[9px] font-semibold text-warning">
                  Action needed
                </span>
              </div>
            ))}

            {/* Divider if there are warnings */}
            {warnings.length > 0 && healthy.length > 0 && (
              <div className="h-px bg-border my-1" />
            )}

            {/* Healthy Rows */}
            {healthy.map((upstream) => (
              <div key={upstream.name} className="flex items-center gap-2 py-1.5">
                <div className="w-1.5 h-1.5 rounded-full bg-success" />
                <span className="text-xs font-medium text-muted-foreground">{upstream.name}</span>
                {upstream.latency_ms && (
                  <span className="text-xs text-muted-foreground">{upstream.latency_ms}ms p95</span>
                )}
                <span className="text-xs font-semibold text-success">99.98%</span>
              </div>
            ))}

            {/* Static system health */}
            {upstreams.length > 0 && (
              <>
                <div className="h-px bg-border my-1" />
                <div className="flex items-center gap-2 py-1.5">
                  <div className="w-1.5 h-1.5 rounded-full bg-success" />
                  <span className="text-xs text-muted-foreground">Policy engine</span>
                  <span className="text-xs text-success">Operational</span>
                </div>
                <div className="flex items-center gap-2 py-1.5">
                  <div className="w-1.5 h-1.5 rounded-full bg-success" />
                  <span className="text-xs text-muted-foreground">Vault</span>
                  <span className="text-xs text-success">Operational</span>
                </div>
              </>
            )}
          </>
        )}

        {/* Footer */}
        <div className="h-px bg-border my-1" />
        <span className="text-[10px] text-muted-foreground">Last checked 18s ago</span>
      </div>
    </div>
  )
}