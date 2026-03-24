"use client"

import { useEffect, useState } from "react"
import {
  LineChart,
  Line,
  ResponsiveContainer,
  XAxis,
  Tooltip,
} from "recharts"
import { getRequestsPerUser, type RequestsPerUserPoint, formatNumber } from "@/lib/api"

export function RequestsPerUserChart() {
  const [data, setData] = useState<RequestsPerUserPoint[]>([])
  const [loading, setLoading] = useState(true)
  const [avgPerUser, setAvgPerUser] = useState(0)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getRequestsPerUser(168) // 7 days
        setData(timeseries)
        // Calculate average
        if (timeseries.length > 0) {
          const total = timeseries.reduce((sum, p) => sum + p.avg_per_user, 0)
          setAvgPerUser(total / timeseries.length)
        }
      } catch (error) {
        console.error("Failed to fetch requests per user:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleDateString("en-US", { month: "short", day: "numeric" }),
    avgPerUser: point.avg_per_user,
    requests: point.request_count,
    users: point.user_count,
  }))

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <div className="flex flex-col gap-0.5">
          <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
            REQUESTS PER USER
          </span>
          <span className="text-[12px] font-bold text-foreground">
            {avgPerUser.toFixed(1)} avg per user
          </span>
        </div>
        {data.length > 0 && (
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] text-muted-foreground">Last 7 days</span>
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
            <span className="text-[14px] text-muted-foreground">No data available</span>
          </div>
        ) : (
          <>
            {/* Legend */}
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-1.5">
                <div className="w-1.5 h-1.5 rounded-full bg-primary" />
                <span className="text-[11px] text-muted-foreground">Avg Requests/User</span>
              </div>
            </div>

            {/* Chart */}
            <div className="flex-1 relative">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={chartData} margin={{ top: 10, right: 10, left: 10, bottom: 10 }}>
                  <XAxis dataKey="time" hide />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: "hsl(var(--card))",
                      border: "1px solid #E2E8F0",
                      borderRadius: "8px",
                      boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
                    }}
                    formatter={(value) => (value as number).toFixed(1)}
                  />
                  <Line
                    type="monotone"
                    dataKey="avgPerUser"
                    stroke="#0A0A0A"
                    strokeWidth={2.5}
                    dot={false}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </>
        )}
      </div>
    </div>
  )
}