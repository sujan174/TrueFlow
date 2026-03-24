"use client"

import { useEffect, useState } from "react"
import {
  AreaChart,
  Area,
  ResponsiveContainer,
  XAxis,
  Tooltip,
} from "recharts"
import { getUserGrowth, type UserGrowthPoint, formatNumber } from "@/lib/api"

export function UserGrowthChart() {
  const [data, setData] = useState<UserGrowthPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [totalUsers, setTotalUsers] = useState(0)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getUserGrowth(720) // 30 days
        setData(timeseries)
        // Get the last cumulative count
        if (timeseries.length > 0) {
          setTotalUsers(timeseries[timeseries.length - 1].cumulative_users)
        }
      } catch (error) {
        console.error("Failed to fetch user growth:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleDateString("en-US", { month: "short", day: "numeric" }),
    cumulative: point.cumulative_users,
    newUsers: point.new_users,
  }))

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <div className="flex flex-col gap-0.5">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            USER GROWTH
          </span>
          <span className="text-[12px] font-bold text-foreground">
            {formatNumber(totalUsers)} total users
          </span>
        </div>
        {data.length > 0 && (
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] text-muted-foreground">Last 30 days</span>
          </div>
        )}
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col gap-2">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No users in this time period</span>
          </div>
        ) : (
          <>
            {/* Legend */}
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-1.5">
                <div className="w-1.5 h-1.5 rounded-full bg-success" />
                <span className="text-[11px] text-muted-foreground">Cumulative Users</span>
              </div>
            </div>

            {/* Chart */}
            <div className="flex-1 relative">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 10, bottom: 10 }}>
                  <XAxis dataKey="time" hide />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: "hsl(var(--card))",
                      border: "1px solid #E2E8F0",
                      borderRadius: "8px",
                      boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
                    }}
                    formatter={(value) => formatNumber(value as number)}
                  />
                  <Area
                    type="monotone"
                    dataKey="cumulative"
                    stroke="#10B981"
                    strokeWidth={2}
                    fill="#10B981"
                    fillOpacity={0.15}
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </>
        )}
      </div>
    </div>
  )
}