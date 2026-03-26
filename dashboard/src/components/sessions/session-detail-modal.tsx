"use client"

import { useState, useEffect, useRef } from "react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { toast } from "sonner"
import {
  Play,
  Pause,
  CheckCircle,
  DollarSign,
  Clock,
  Zap,
  Loader2,
  AlertCircle,
  RefreshCw,
} from "lucide-react"
import { SessionStatusBadge } from "./session-status-badge"
import {
  getSession,
  getSessionEntity,
  updateSessionStatus,
  setSessionSpendCap,
  type SessionRow,
  type SessionDetail,
  type SessionEntity,
} from "@/lib/api"
import { formatRelativeTime, formatCost, formatLatency } from "@/lib/types/audit"

interface SessionDetailModalProps {
  session: SessionRow | null
  projectId: string
  open: boolean
  onOpenChange: (open: boolean) => void
  onSessionUpdated: () => void
}

export function SessionDetailModal({
  session,
  projectId,
  open,
  onOpenChange,
  onSessionUpdated,
}: SessionDetailModalProps) {
  const [detail, setDetail] = useState<SessionDetail | null>(null)
  const [entity, setEntity] = useState<SessionEntity | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [processing, setProcessing] = useState(false)
  const [editingCap, setEditingCap] = useState(false)
  const [newCap, setNewCap] = useState<string>("")

  // AbortController ref for race condition prevention
  const abortControllerRef = useRef<AbortController | null>(null)

  // Fetch session detail and entity when modal opens
  useEffect(() => {
    // Abort any previous request
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
    }

    if (open && session?.session_id) {
      // Create new AbortController for this request
      abortControllerRef.current = new AbortController()
      const signal = abortControllerRef.current.signal

      setLoading(true)
      setError(null)
      Promise.all([
        getSession(session.session_id, projectId),
        getSessionEntity(session.session_id, projectId),
      ])
        .then(([detailData, entityData]) => {
          // Check if request was aborted
          if (signal.aborted) return
          setDetail(detailData)
          setEntity(entityData)
          setNewCap(entityData.spend_cap_usd?.toString() || "")
        })
        .catch((err) => {
          if (signal.aborted) return // Ignore aborted requests
          console.error("Failed to fetch session details:", err)
          setError(err instanceof Error ? err.message : "Failed to load session details")
        })
        .finally(() => {
          if (!signal.aborted) {
            setLoading(false)
          }
        })
    } else {
      setDetail(null)
      setEntity(null)
      setError(null)
    }

    // Cleanup on unmount or when session changes
    return () => {
      if (abortControllerRef.current) {
        abortControllerRef.current.abort()
      }
    }
  }, [open, session?.session_id, projectId])

  const handleStatusChange = async (newStatus: "paused" | "active" | "completed") => {
    if (!session?.session_id || !entity) return

    setProcessing(true)
    try {
      const updated = await updateSessionStatus(session.session_id, projectId, newStatus)
      setEntity(updated)
      toast.success(`Session ${newStatus === "active" ? "resumed" : newStatus === "paused" ? "paused" : "completed"}`)
      onSessionUpdated()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : `Failed to ${newStatus} session`)
    } finally {
      setProcessing(false)
    }
  }

  const handleSaveCap = async () => {
    if (!session?.session_id || !entity) return

    const capValue = newCap === "" ? null : parseFloat(newCap)
    if (newCap !== "" && (isNaN(capValue as number) || (capValue as number) < 0)) {
      toast.error("Please enter a valid positive number")
      return
    }

    setProcessing(true)
    try {
      const updated = await setSessionSpendCap(session.session_id, projectId, capValue)
      setEntity(updated)
      setEditingCap(false)
      toast.success("Spend cap updated")
      onSessionUpdated()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to update spend cap")
    } finally {
      setProcessing(false)
    }
  }

  if (!session) return null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-4xl max-h-[90vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            Session Details
            {entity && <SessionStatusBadge status={entity.status} />}
          </DialogTitle>
          <DialogDescription>
            {session.session_id || "No session ID"}
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center py-12 gap-4">
            <AlertCircle className="h-12 w-12 text-muted-foreground/50" />
            <p className="text-muted-foreground">{error}</p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                // Re-trigger fetch by resetting state
                setError(null)
                setLoading(true)
                // Force re-fetch by calling the effect
                if (session?.session_id) {
                  Promise.all([
                    getSession(session.session_id, projectId),
                    getSessionEntity(session.session_id, projectId),
                  ])
                    .then(([detailData, entityData]) => {
                      setDetail(detailData)
                      setEntity(entityData)
                      setNewCap(entityData.spend_cap_usd?.toString() || "")
                    })
                    .catch((err) => {
                      setError(err instanceof Error ? err.message : "Failed to load session details")
                    })
                    .finally(() => setLoading(false))
                }
              }}
              className="gap-2"
            >
              <RefreshCw className="h-4 w-4" />
              Retry
            </Button>
          </div>
        ) : (
          <div className="flex-1 overflow-auto space-y-6">
            {/* Session Info Grid */}
            {entity && (
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div className="bg-muted/50 rounded-lg p-3">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                    <DollarSign className="h-3 w-3" />
                    Total Cost
                  </div>
                  <div className="text-lg font-semibold">
                    {formatCost(entity.total_cost_usd)}
                  </div>
                </div>
                <div className="bg-muted/50 rounded-lg p-3">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                    <Zap className="h-3 w-3" />
                    Total Tokens
                  </div>
                  <div className="text-lg font-semibold">
                    {entity.total_tokens.toLocaleString()}
                  </div>
                </div>
                <div className="bg-muted/50 rounded-lg p-3">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                    <Clock className="h-3 w-3" />
                    Requests
                  </div>
                  <div className="text-lg font-semibold">
                    {entity.total_requests.toLocaleString()}
                  </div>
                </div>
                <div className="bg-muted/50 rounded-lg p-3">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                    <DollarSign className="h-3 w-3" />
                    Spend Cap
                  </div>
                  {editingCap ? (
                    <div className="flex items-center gap-2">
                      <Input
                        type="number"
                        step="0.01"
                        min="0"
                        value={newCap}
                        onChange={(e) => setNewCap(e.target.value)}
                        className="h-7 w-20 text-sm"
                        placeholder="USD"
                      />
                      <Button
                        size="sm"
                        onClick={handleSaveCap}
                        disabled={processing}
                      >
                        {processing ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save"}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setEditingCap(false)
                          setNewCap(entity.spend_cap_usd?.toString() || "")
                        }}
                      >
                        Cancel
                      </Button>
                    </div>
                  ) : (
                    <div
                      className="text-lg font-semibold cursor-pointer hover:text-primary"
                      onClick={() => setEditingCap(true)}
                      title="Click to edit"
                    >
                      {entity.spend_cap_usd ? formatCost(entity.spend_cap_usd) : "No limit"}
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Status Actions */}
            {entity && entity.status !== "expired" && entity.status !== "completed" && (
              <div className="flex items-center gap-2">
                {entity.status === "active" && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => handleStatusChange("paused")}
                    disabled={processing}
                    className="gap-2"
                  >
                    {processing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Pause className="h-4 w-4" />}
                    Pause Session
                  </Button>
                )}
                {entity.status === "paused" && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => handleStatusChange("active")}
                    disabled={processing}
                    className="gap-2"
                  >
                    {processing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
                    Resume Session
                  </Button>
                )}
                {(entity.status === "active" || entity.status === "paused") && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => handleStatusChange("completed")}
                    disabled={processing}
                    className="gap-2"
                  >
                    {processing ? <Loader2 className="h-4 w-4 animate-spin" /> : <CheckCircle className="h-4 w-4" />}
                    Complete Session
                  </Button>
                )}
              </div>
            )}

            <Separator />

            {/* Requests Table */}
            <div>
              <h3 className="text-sm font-semibold mb-3">Requests ({detail?.requests?.length || 0})</h3>
              {detail?.requests && detail.requests.length > 0 ? (
                <div className="border rounded-lg overflow-hidden">
                  <div className="max-h-[300px] overflow-auto">
                    <table className="w-full text-sm">
                      <thead className="bg-muted/50 border-b sticky top-0">
                        <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                          <th className="px-3 py-2 text-left">Time</th>
                          <th className="px-3 py-2 text-left">Method</th>
                          <th className="px-3 py-2 text-left">Model</th>
                          <th className="px-3 py-2 text-left">Tokens</th>
                          <th className="px-3 py-2 text-left">Cost</th>
                          <th className="px-3 py-2 text-left">Latency</th>
                          <th className="px-3 py-2 text-left">Status</th>
                        </tr>
                      </thead>
                      <tbody>
                        {detail.requests.map((req, idx) => (
                          <tr key={req.id || idx} className="border-b last:border-0 hover:bg-muted/30">
                            <td className="px-3 py-2 text-xs text-muted-foreground">
                              {formatRelativeTime(req.created_at)}
                            </td>
                            <td className="px-3 py-2">
                              <Badge variant="outline" className="text-[10px]">
                                {req.method}
                              </Badge>
                            </td>
                            <td className="px-3 py-2 text-xs">
                              {req.model || "—"}
                            </td>
                            <td className="px-3 py-2 text-xs">
                              {req.prompt_tokens !== null && req.completion_tokens !== null
                                ? `${(req.prompt_tokens + req.completion_tokens).toLocaleString()}`
                                : "—"}
                            </td>
                            <td className="px-3 py-2 text-xs">
                              {formatCost(req.estimated_cost_usd)}
                            </td>
                            <td className="px-3 py-2 text-xs">
                              {formatLatency(req.response_latency_ms)}
                            </td>
                            <td className="px-3 py-2">
                              <Badge
                                variant={req.upstream_status && req.upstream_status < 400 ? "success" : "destructive"}
                                className="text-[10px]"
                              >
                                {req.upstream_status || "—"}
                              </Badge>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              ) : (
                <div className="text-sm text-muted-foreground text-center py-8">
                  No requests in this session
                </div>
              )}
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}