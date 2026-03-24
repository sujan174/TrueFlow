"use client"

import { useEffect, useState } from "react"
import {
  LineChart,
  Line,
  ResponsiveContainer,
  XAxis,
  YAxis,
  Tooltip,
  Legend,
} from "recharts"
import { getHitlVolume, type HitlVolumePoint } from "@/lib/api"

export function HitlVolumeChart() {
  const [data, setData] = useState<HitlVolumePoint[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchData() {
      try {
        const timeseries = await getHitlVolume(168) // 7 days
        setData(timeseries)
      } catch (error) {
        console.error("Failed to fetch HITL volume timeseries:", error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  // Format time for x-axis
  const chartData = data.map((point) => ({
    time: new Date(point.bucket).toLocaleDateString("en-US", { weekday: "short" }),
    approved: point.approved_count,
    rejected: point.rejected_count,
    expired: point.expired_count,
    pending: point.pending_count,
  }))

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          HITL Volume
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 p-4 flex flex-col">
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground">Loading...</div>
          </div>
        ) : data.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No HITL data available</span>
          </div>
        ) : (
          <div className="flex-1 relative">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData} margin={{ top: 10, right: 10, left: 10, bottom: 10 }}>
                <XAxis dataKey="time" tick={{ fontSize: 10, fill: "#64748B" }} />
                <YAxis tick={{ fontSize: 10, fill: "#64748B" }} width={30} />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--card))",
                    border: "1px solid #E2E8F0",
                    borderRadius: "8px",
                    boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
                  }}
                />
                <Legend
                  wrapperStyle={{ fontSize: "10px" }}
                  iconType="circle"
                  iconSize={6}
                />
                <Line
                  type="monotone"
                  dataKey="approved"
                  name="Approved"
                  stroke="#10B981"
                  strokeWidth={3}
                  dot={false}
                />
                <Line
                  type="monotone"
                  dataKey="rejected"
                  name="Rejected"
                  stroke="#F43F5E"
                  strokeWidth={3}
                  dot={false}
                />
                <Line
                  type="monotone"
                  dataKey="pending"
                  name="Pending"
                  stroke="#0A0A0A"
                  strokeWidth={3}
                  dot={false}
                />
                <Line
                  type="monotone"
                  dataKey="expired"
                  name="Expired"
                  stroke="#F59E0B"
                  strokeWidth={3}
                  dot={false}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
        )}
      </div>
    </div>
  )
}