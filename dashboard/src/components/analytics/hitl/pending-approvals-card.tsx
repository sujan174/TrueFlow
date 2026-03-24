"use client"

import { useEffect, useState, useCallback } from "react"
import { listApprovals, decideApproval, type ApprovalRequest } from "@/lib/api"

export function PendingApprovalsCard() {
  const [data, setData] = useState<ApprovalRequest[]>([])
  const [loading, setLoading] = useState(true)
  const [showResolved, setShowResolved] = useState(false)
  const [processingId, setProcessingId] = useState<string | null>(null)

  const fetchData = useCallback(async () => {
    try {
      const approvals = await listApprovals()
      setData(approvals)
    } catch (error) {
      console.error("Failed to fetch approvals:", error)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchData()
  }, [fetchData])

  const handleDecision = async (id: string, decision: "approved" | "rejected") => {
    setProcessingId(id)
    try {
      await decideApproval(id, decision)
      // Refresh data after decision
      await fetchData()
    } catch (error) {
      console.error(`Failed to ${decision} approval:`, error)
    } finally {
      setProcessingId(null)
    }
  }

  // Filter based on showResolved toggle
  const filteredData = showResolved
    ? data
    : data.filter((a) => a.status === "pending")

  // Calculate time waiting
  const formatWaitingSince = (createdAt: string): string => {
    const created = new Date(createdAt)
    const now = new Date()
    const diffMs = now.getTime() - created.getTime()
    const diffMins = Math.floor(diffMs / 60000)
    if (diffMins < 60) return `${diffMins}m`
    const diffHours = Math.floor(diffMins / 60)
    if (diffHours < 24) return `${diffHours}h`
    const diffDays = Math.floor(diffHours / 24)
    return `${diffDays}d`
  }

  // Calculate timeout remaining
  const formatTimeout = (expiresAt: string): string => {
    const expires = new Date(expiresAt)
    const now = new Date()
    const diffMs = expires.getTime() - now.getTime()
    if (diffMs <= 0) return "Expired"
    const diffMins = Math.floor(diffMs / 60000)
    if (diffMins < 60) return `${diffMins}m left`
    const diffHours = Math.floor(diffMins / 60)
    return `${diffHours}h left`
  }

  // Get status badge color
  const getStatusColor = (status: string): string => {
    switch (status) {
      case "approved":
        return "bg-success/10 text-success"
      case "rejected":
        return "bg-destructive/10 text-destructive"
      case "expired":
        return "bg-warning/20 text-warning"
      default:
        return "bg-muted text-muted-foreground"
    }
  }

  return (
    <div className="h-full bg-card border border rounded-[14px] flex flex-col shadow-sm overflow-hidden">
      {/* Header */}
      <div className="h-[44px] px-4 flex items-center justify-between border-b border">
        <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
          Pending Approvals
        </span>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="checkbox"
              checked={showResolved}
              onChange={(e) => setShowResolved(e.target.checked)}
              className="w-3.5 h-3.5 rounded border-border text-foreground focus:ring-0"
            />
            <span className="text-[11px] text-muted-foreground">Show resolved</span>
          </label>
        </div>
      </div>

      {/* Body - Table */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="h-full flex items-center justify-center">
            <div className="animate-pulse text-muted-foreground text-[12px]">Loading approvals...</div>
          </div>
        ) : filteredData.length === 0 ? (
          <div className="h-full flex items-center justify-center">
            <span className="text-[14px] text-muted-foreground">No pending approvals</span>
          </div>
        ) : (
          <table className="w-full">
            <thead className="sticky top-0 bg-muted/50 border-b border">
              <tr>
                <th className="px-4 py-2 text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Request ID
                </th>
                <th className="px-4 py-2 text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Token
                </th>
                <th className="px-4 py-2 text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Policy
                </th>
                <th className="px-4 py-2 text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Status
                </th>
                <th className="px-4 py-2 text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Waiting
                </th>
                <th className="px-4 py-2 text-left text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Timeout
                </th>
                <th className="px-4 py-2 text-right text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {filteredData.slice(0, 10).map((approval) => (
                <tr
                  key={approval.id}
                  className="border-b border-border hover:bg-muted/50"
                >
                  <td className="px-4 py-3">
                    <span className="text-[12px] font-mono text-foreground">
                      {approval.id.slice(0, 8)}...
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-[12px] text-foreground">
                      {approval.token_id.slice(0, 12)}...
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-[12px] text-muted-foreground">
                      Require Approval
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span className={`px-2 py-0.5 rounded text-[10px] font-medium ${getStatusColor(approval.status)}`}>
                      {approval.status}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-[12px] text-foreground">
                      {formatWaitingSince(approval.created_at)}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-[12px] text-muted-foreground">
                      {formatTimeout(approval.expires_at)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    {approval.status === "pending" ? (
                      <div className="flex items-center justify-end gap-2">
                        <button
                          onClick={() => handleDecision(approval.id, "approved")}
                          disabled={processingId === approval.id}
                          className="px-3 py-1.5 bg-success/10 rounded text-[11px] font-medium text-success hover:bg-success/20 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          Approve
                        </button>
                        <button
                          onClick={() => handleDecision(approval.id, "rejected")}
                          disabled={processingId === approval.id}
                          className="px-3 py-1.5 bg-destructive/10 rounded text-[11px] font-medium text-destructive hover:bg-destructive/20 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          Reject
                        </button>
                      </div>
                    ) : (
                      <span className="text-[11px] text-muted-foreground">—</span>
                    )}
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