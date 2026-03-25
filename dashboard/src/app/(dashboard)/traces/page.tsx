"use client"

import { useEffect, useState, useCallback } from "react"
import { FileText, RefreshCw, Eye } from "lucide-react"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import { listAuditLogs, getAuditLogDetail, type AuditLogRowType, type AuditLogDetailRow, type AuditFilters } from "@/lib/api"
import { TraceTable } from "@/components/traces/trace-table"
import { TraceDetailModal } from "@/components/traces/trace-detail-modal"
import { TraceFilters } from "@/components/traces/trace-filters"

export default function TracesPage() {
  const [logs, setLogs] = useState<AuditLogRowType[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [filters, setFilters] = useState<AuditFilters>({})
  const [selectedLog, setSelectedLog] = useState<AuditLogRowType | null>(null)
  const [selectedLogDetail, setSelectedLogDetail] = useState<AuditLogDetailRow | null>(null)
  const [loadingDetail, setLoadingDetail] = useState(false)
  const [projectId, setProjectId] = useState<string>("")

  // Get project ID from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem("lastProjectId")
    if (stored) {
      setProjectId(stored)
    }
  }, [])

  const fetchData = useCallback(async (isRefresh = false) => {
    if (!projectId) return
    if (isRefresh) setRefreshing(true)
    try {
      const data = await listAuditLogs(projectId, filters, 100)
      setLogs(data)
    } catch (error) {
      console.error("Failed to fetch audit logs:", error)
      if (!isRefresh) toast.error("Failed to load request log")
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [projectId, filters])

  // Initial fetch + polling every 8 seconds (like approvals)
  useEffect(() => {
    fetchData()
    const interval = setInterval(() => fetchData(), 8000)
    return () => clearInterval(interval)
  }, [fetchData])

  const handleSelectLog = async (log: AuditLogRowType) => {
    setSelectedLog(log)
    setLoadingDetail(true)
    try {
      const detail = await getAuditLogDetail(log.id, projectId)
      setSelectedLogDetail(detail)
    } catch (error) {
      console.error("Failed to fetch log detail:", error)
      toast.error("Failed to load request details")
    } finally {
      setLoadingDetail(false)
    }
  }

  const handleCloseModal = () => {
    setSelectedLog(null)
    setSelectedLogDetail(null)
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight flex items-center gap-2">
              <FileText className="h-7 w-7" />
              Request Log
            </h1>
            <p className="text-sm text-muted-foreground">
              Search and filter all logged requests with detailed trace information
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

        {/* Filters */}
        <TraceFilters filters={filters} onChange={setFilters} />

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden flex-1">
          {loading ? (
            <div className="h-full flex items-center justify-center py-16">
              <div className="animate-pulse text-muted-foreground">Loading requests...</div>
            </div>
          ) : logs.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center py-16">
              <FileText className="h-12 w-12 text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No requests found</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                {Object.keys(filters).length > 0
                  ? "Try adjusting your filters"
                  : "Requests will appear here as they are logged"}
              </p>
            </div>
          ) : (
            <TraceTable logs={logs} onSelect={handleSelectLog} />
          )}
        </div>
      </div>

      {/* Detail Modal */}
      <TraceDetailModal
        log={selectedLog}
        detail={selectedLogDetail}
        loading={loadingDetail}
        open={selectedLog !== null}
        onOpenChange={(open) => !open && handleCloseModal()}
      />
    </div>
  )
}