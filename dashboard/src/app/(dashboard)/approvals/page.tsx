"use client"

import { useEffect, useState, useCallback } from "react"
import { ClipboardCheck, RefreshCw, X, Check, Eye } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { toast } from "sonner"
import { listApprovals, decideApproval, type ApprovalRequest } from "@/lib/api"
import { ApprovalDetailModal } from "@/components/approvals/approval-detail-modal"
import { BulkActionBar } from "@/components/approvals/bulk-action-bar"

// Time formatting utilities (reused from pending-approvals-card.tsx)
function formatWaitingSince(createdAt: string): string {
  const created = new Date(createdAt)
  const now = new Date()
  const diffMs = now.getTime() - created.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  if (diffMins < 1) return "<1m"
  if (diffMins < 60) return `${diffMins}m`
  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h`
  const diffDays = Math.floor(diffHours / 24)
  return `${diffDays}d`
}

function formatTimeout(expiresAt: string): string {
  const expires = new Date(expiresAt)
  const now = new Date()
  const diffMs = expires.getTime() - now.getTime()
  if (diffMs <= 0) return "Expired"
  const diffMins = Math.floor(diffMs / 60000)
  if (diffMins < 60) return `${diffMins}m`
  const diffHours = Math.floor(diffMins / 60)
  return `${diffHours}h`
}

function getStatusBadge(status: string) {
  const variants: Record<string, { bg: string; text: string }> = {
    pending: { bg: "bg-muted", text: "text-muted-foreground" },
    approved: { bg: "bg-success/10", text: "text-success" },
    rejected: { bg: "bg-destructive/10", text: "text-destructive" },
    expired: { bg: "bg-warning/20", text: "text-warning" },
  }
  return variants[status] || variants.pending
}

export default function ApprovalsPage() {
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [processingIds, setProcessingIds] = useState<Set<string>>(new Set())
  const [bulkProcessing, setBulkProcessing] = useState<"approved" | "rejected" | null>(null)
  const [modalApproval, setModalApproval] = useState<ApprovalRequest | null>(null)
  const [statusFilter, setStatusFilter] = useState<"all" | "pending" | "approved" | "rejected" | "expired">("all")

  const fetchData = useCallback(async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true)
    try {
      const data = await listApprovals()
      setApprovals(data)
    } catch (error) {
      console.error("Failed to fetch approvals:", error)
      if (!isRefresh) toast.error("Failed to load approvals")
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [])

  // Initial fetch + polling every 8 seconds
  useEffect(() => {
    fetchData()
    const interval = setInterval(() => fetchData(), 8000)
    return () => clearInterval(interval)
  }, [fetchData])

  // Handle individual decision
  const handleDecision = async (id: string, decision: "approved" | "rejected") => {
    setProcessingIds(prev => new Set(prev).add(id))
    try {
      await decideApproval(id, decision)
      // Optimistically update status
      setApprovals(prev =>
        prev.map(a => a.id === id ? { ...a, status: decision } : a)
      )
      // Remove from selection
      setSelectedIds(prev => {
        const next = new Set(prev)
        next.delete(id)
        return next
      })
      // Close modal if open
      if (modalApproval?.id === id) {
        setModalApproval(null)
      }
      toast.success(`Request ${decision}`)
    } catch (error) {
      toast.error(`Failed to ${decision} request`)
    } finally {
      setProcessingIds(prev => {
        const next = new Set(prev)
        next.delete(id)
        return next
      })
    }
  }

  // Handle bulk decision
  const handleBulkDecision = async (decision: "approved" | "rejected") => {
    const pendingSelected = Array.from(selectedIds).filter(id =>
      approvals.find(a => a.id === id)?.status === "pending"
    )

    if (pendingSelected.length === 0) {
      toast.info("No pending requests selected")
      return
    }

    setBulkProcessing(decision)
    setProcessingIds(new Set(pendingSelected))

    try {
      const results = await Promise.allSettled(
        pendingSelected.map(id => decideApproval(id, decision))
      )

      const succeeded = results.filter(r => r.status === "fulfilled").length
      const failed = results.length - succeeded

      // Optimistically update statuses
      setApprovals(prev =>
        prev.map(a => {
          if (pendingSelected.includes(a.id)) {
            return { ...a, status: decision }
          }
          return a
        })
      )

      setSelectedIds(new Set())

      if (failed === 0) {
        toast.success(`${succeeded} requests ${decision}`)
      } else {
        toast.warning(`${succeeded} ${decision}, ${failed} failed`)
      }
    } catch (error) {
      toast.error("Bulk operation failed")
    } finally {
      setBulkProcessing(null)
      setProcessingIds(new Set())
    }
  }

  // Selection handlers
  const toggleSelect = (id: string) => {
    setSelectedIds(prev => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const toggleSelectAll = () => {
    const filtered = filteredApprovals
    const allSelected = filtered.every(a => selectedIds.has(a.id))

    if (allSelected) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(filtered.map(a => a.id)))
    }
  }

  const clearSelection = () => setSelectedIds(new Set())

  // Filter approvals
  const filteredApprovals = statusFilter === "all"
    ? approvals
    : approvals.filter(a => a.status === statusFilter)

  const pendingSelectedCount = Array.from(selectedIds).filter(id =>
    approvals.find(a => a.id === id)?.status === "pending"
  ).length

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight flex items-center gap-2">
              <ClipboardCheck className="h-7 w-7" />
              Approvals Queue
            </h1>
            <p className="text-sm text-muted-foreground">
              Review and approve requests paused by require_approval policies
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => fetchData(true)}
            disabled={refreshing}
            className="gap-2"
          >
            <RefreshCw className={`h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
            Refresh
          </Button>
        </div>

        {/* Filter Pills */}
        <div className="flex items-center gap-2">
          {(["all", "pending", "approved", "rejected", "expired"] as const).map((filter) => (
            <button
              key={filter}
              onClick={() => setStatusFilter(filter)}
              className={`px-3 py-1.5 rounded-lg text-sm font-medium transition-colors ${
                statusFilter === filter
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:bg-muted"
              }`}
            >
              {filter.charAt(0).toUpperCase() + filter.slice(1)}
              {filter === "pending" && approvals.filter(a => a.status === "pending").length > 0 && (
                <span className="ml-1.5 px-1.5 py-0.5 rounded-full bg-primary/20 text-[10px]">
                  {approvals.filter(a => a.status === "pending").length}
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden flex-1">
          {loading ? (
            <div className="h-full flex items-center justify-center">
              <div className="animate-pulse text-muted-foreground">Loading approvals...</div>
            </div>
          ) : filteredApprovals.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center py-16">
              <ClipboardCheck className="h-12 w-12 text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">
                {statusFilter === "all" ? "No approval requests" : `No ${statusFilter} requests`}
              </p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Requests paused by require_approval policies will appear here
              </p>
            </div>
          ) : (
            <div className="overflow-auto">
              <table className="w-full">
                <thead className="bg-muted/50 border-b sticky top-0 z-10">
                  <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                    <th className="px-4 py-3 text-left w-10">
                      <input
                        type="checkbox"
                        checked={filteredApprovals.length > 0 && filteredApprovals.every(a => selectedIds.has(a.id))}
                        onChange={toggleSelectAll}
                        className="w-4 h-4 rounded border-border"
                      />
                    </th>
                    <th className="px-4 py-3 text-left">Request ID</th>
                    <th className="px-4 py-3 text-left">Agent</th>
                    <th className="px-4 py-3 text-left">Endpoint</th>
                    <th className="px-4 py-3 text-left">Upstream</th>
                    <th className="px-4 py-3 text-left">Status</th>
                    <th className="px-4 py-3 text-left">Waiting</th>
                    <th className="px-4 py-3 text-left">Timeout</th>
                    <th className="px-4 py-3 text-right">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredApprovals.map((approval) => {
                    const statusBadge = getStatusBadge(approval.status)
                    const isSelected = selectedIds.has(approval.id)
                    const isProcessing = processingIds.has(approval.id)

                    return (
                      <tr
                        key={approval.id}
                        className={`border-b last:border-0 hover:bg-muted/30 transition-colors ${
                          isSelected ? "bg-primary/5" : ""
                        }`}
                      >
                        <td className="px-4 py-3">
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => toggleSelect(approval.id)}
                            className="w-4 h-4 rounded border-border"
                          />
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-sm font-mono">
                            {approval.id.slice(0, 8)}...
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-sm text-foreground">
                            {approval.request_summary?.agent || approval.token_id?.slice(0, 12) || "—"}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-1.5">
                            <Badge variant="outline" className="text-[10px] px-1.5">
                              {approval.request_summary?.method || "POST"}
                            </Badge>
                            <span className="text-sm text-muted-foreground truncate max-w-[150px]">
                              {approval.request_summary?.path || "/v1/chat/completions"}
                            </span>
                          </div>
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-sm text-muted-foreground">
                            {approval.request_summary?.upstream || "—"}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <span className={`px-2 py-0.5 rounded text-[10px] font-medium ${statusBadge.bg} ${statusBadge.text}`}>
                            {approval.status}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-sm text-foreground">
                            {formatWaitingSince(approval.created_at)}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-sm text-muted-foreground">
                            {formatTimeout(approval.expires_at)}
                          </span>
                        </td>
                        <td className="px-4 py-3 text-right">
                          <div className="flex items-center justify-end gap-2">
                            <Button
                              variant="ghost"
                              size="icon-xs"
                              onClick={() => setModalApproval(approval)}
                              title="View details"
                            >
                              <Eye className="h-4 w-4" />
                            </Button>
                            {approval.status === "pending" && (
                              <>
                                <Button
                                  variant="ghost"
                                  size="xs"
                                  onClick={() => handleDecision(approval.id, "approved")}
                                  disabled={isProcessing}
                                  className="text-success hover:bg-success/10"
                                >
                                  <Check className="h-3.5 w-3.5 mr-1" />
                                  Approve
                                </Button>
                                <Button
                                  variant="ghost"
                                  size="xs"
                                  onClick={() => handleDecision(approval.id, "rejected")}
                                  disabled={isProcessing}
                                  className="text-destructive hover:bg-destructive/10"
                                >
                                  <X className="h-3.5 w-3.5 mr-1" />
                                  Reject
                                </Button>
                              </>
                            )}
                          </div>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Bulk Action Bar */}
      <BulkActionBar
        selectedCount={pendingSelectedCount}
        onApprove={() => handleBulkDecision("approved")}
        onReject={() => handleBulkDecision("rejected")}
        onClearSelection={clearSelection}
        processing={bulkProcessing !== null}
      />

      {/* Detail Modal */}
      <ApprovalDetailModal
        approval={modalApproval}
        open={modalApproval !== null}
        onOpenChange={(open) => !open && setModalApproval(null)}
        onDecision={handleDecision}
        processing={modalApproval ? processingIds.has(modalApproval.id) : false}
      />
    </div>
  )
}