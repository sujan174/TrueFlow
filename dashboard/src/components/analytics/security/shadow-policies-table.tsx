"use client"

import { useEffect, useState } from "react"
import { getShadowPolicies, type ShadowPolicyStat } from "@/lib/api"

function StatusBadge({ status }: { status: string }) {
  const isMonitoring = status === "Monitoring"

  return (
    <span
      className={`px-2 py-0.5 rounded-[4px] text-[10px] font-medium ${
        isMonitoring
          ? "bg-warning/20 text-warning"
          : "bg-success/10 text-success"
      }`}
    >
      {status}
    </span>
  )
}

export function ShadowPoliciesTable() {
  const [data, setData] = useState<ShadowPolicyStat[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const policies = await getShadowPolicies(168)
        setData(policies)
      } catch (error) {
        console.error("Failed to fetch shadow policies:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  if (loading) {
    return (
      <div className="h-full bg-card border border rounded-[14px] flex items-center justify-center shadow-sm">
        <div className="animate-pulse text-muted-foreground text-[12px]">Loading...</div>
      </div>
    )
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] p-5 flex flex-col shadow-sm">
      <h3 className="text-[14px] font-semibold text-foreground mb-4">
        Shadow Mode Policies
      </h3>
      <div className="flex-1 overflow-auto">
        {data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-[12px] text-muted-foreground py-8">
            No shadow mode policies with violations in the selected period
          </div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border">
                <th className="text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase py-2 px-3">
                  Policy Name
                </th>
                <th className="text-right text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase py-2 px-3">
                  Violations
                </th>
                <th className="text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase py-2 px-3">
                  Top Token
                </th>
                <th className="text-right text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase py-2 px-3">
                  Status
                </th>
              </tr>
            </thead>
            <tbody>
              {data.map((item, index) => (
                <tr
                  key={item.policy_name}
                  className={`border-b border-border ${
                    index === data.length - 1 ? "border-b-0" : ""
                  }`}
                >
                  <td className="py-3 px-3">
                    <span className="text-[12px] font-mono text-foreground">
                      {item.policy_name}
                    </span>
                  </td>
                  <td className="py-3 px-3 text-right">
                    <span className="text-[12px] font-medium text-foreground">
                      {item.violations}
                    </span>
                  </td>
                  <td className="py-3 px-3">
                    <span className="text-[12px] text-muted-foreground">
                      {item.top_token}
                    </span>
                  </td>
                  <td className="py-3 px-3 text-right">
                    <StatusBadge status={item.status} />
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