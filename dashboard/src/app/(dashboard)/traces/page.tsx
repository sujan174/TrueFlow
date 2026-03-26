"use client"

import { useEffect, useState, useCallback } from "react"
import { useSearchParams, useRouter } from "next/navigation"
import { FileText, RefreshCw, Layers, Eye, MessageSquare } from "lucide-react"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import { cn } from "@/lib/utils"
import {
  listAuditLogs,
  getAuditLogDetail,
  listSessions,
  type AuditLogRowType,
  type AuditLogDetailRow,
  type AuditFilters,
  type SessionRow,
} from "@/lib/api"
import { TraceTable } from "@/components/traces/trace-table"
import { TraceDetailModal } from "@/components/traces/trace-detail-modal"
import { TraceFilters } from "@/components/traces/trace-filters"
import { SessionTable } from "@/components/sessions/session-table"
import { SessionDetailModal } from "@/components/sessions/session-detail-modal"
import { useProject } from "@/contexts/project-context"

const tabs = [
  { name: "Traces", id: "Traces", icon: FileText },
  { name: "Sessions", id: "Sessions", icon: Layers },
]

export default function TracesPage() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const activeTab = searchParams.get("tab") || "Traces"

  const { selectedProjectId } = useProject()

  // Traces state
  const [logs, setLogs] = useState<AuditLogRowType[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [filters, setFilters] = useState<AuditFilters>({})
  const [selectedLog, setSelectedLog] = useState<AuditLogRowType | null>(null)
  const [selectedLogDetail, setSelectedLogDetail] = useState<AuditLogDetailRow | null>(null)
  const [loadingDetail, setLoadingDetail] = useState(false)

  // Sessions state
  const [sessions, setSessions] = useState<SessionRow[]>([])
  const [loadingSessions, setLoadingSessions] = useState(true)
  const [refreshingSessions, setRefreshingSessions] = useState(false)
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null)

  const handleTabChange = (tabId: string) => {
    const params = new URLSearchParams(searchParams.toString())
    params.set("tab", tabId)
    router.push(`/traces?${params.toString()}`)
  }

  // Fetch traces
  const fetchTraces = useCallback(async (isRefresh = false) => {
    if (!selectedProjectId) return
    if (isRefresh) setRefreshing(true)
    try {
      const data = await listAuditLogs(selectedProjectId, filters, 100)
      setLogs(data)
    } catch (error) {
      console.error("Failed to fetch audit logs:", error)
      if (!isRefresh) toast.error("Failed to load request log")
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [selectedProjectId, filters])

  // Fetch sessions
  const fetchSessions = useCallback(async (isRefresh = false) => {
    if (!selectedProjectId) return
    if (isRefresh) setRefreshingSessions(true)
    try {
      const data = await listSessions(selectedProjectId, undefined, 100)
      setSessions(data)
    } catch (error) {
      console.error("Failed to fetch sessions:", error)
      if (!isRefresh) toast.error("Failed to load sessions")
    } finally {
      setLoadingSessions(false)
      setRefreshingSessions(false)
    }
  }, [selectedProjectId])

  // Initial fetch + polling for traces
  useEffect(() => {
    if (activeTab === "Traces") {
      fetchTraces()
      const interval = setInterval(() => fetchTraces(), 8000)
      return () => clearInterval(interval)
    }
  }, [fetchTraces, activeTab])

  // Initial fetch + polling for sessions
  useEffect(() => {
    if (activeTab === "Sessions") {
      fetchSessions()
      const interval = setInterval(() => fetchSessions(), 8000)
      return () => clearInterval(interval)
    }
  }, [fetchSessions, activeTab])

  const handleSelectLog = async (log: AuditLogRowType) => {
    if (!selectedProjectId) return
    setSelectedLog(log)
    setLoadingDetail(true)
    try {
      const detail = await getAuditLogDetail(log.id, selectedProjectId)
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

  const handleSelectSession = (session: SessionRow) => {
    setSelectedSession(session)
  }

  const handleSessionUpdated = () => {
    fetchSessions(true)
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight flex items-center gap-2">
              {activeTab === "Traces" ? (
                <>
                  <FileText className="h-7 w-7" />
                  Request Log
                </>
              ) : (
                <>
                  <Layers className="h-7 w-7" />
                  Sessions
                </>
              )}
            </h1>
            <p className="text-sm text-muted-foreground">
              {activeTab === "Traces"
                ? "Search and filter all logged requests with detailed trace information"
                : "View and manage conversation sessions with budget controls"}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => activeTab === "Traces" ? fetchTraces(true) : fetchSessions(true)}
            disabled={activeTab === "Traces" ? refreshing : refreshingSessions}
            className="gap-2"
          >
            <RefreshCw className={`h-4 w-4 ${refreshing || refreshingSessions ? "animate-spin" : ""}`} />
            Refresh
          </Button>
        </div>

        {/* Tab Bar */}
        <div className="flex items-center gap-0.5 p-1 bg-card/50 backdrop-blur-sm border rounded-xl overflow-x-auto scrollbar-thin">
          {tabs.map((tab) => {
            const isActive = tab.id === activeTab
            return (
              <button
                key={tab.id}
                onClick={() => handleTabChange(tab.id)}
                className={cn(
                  "relative px-4 py-2 rounded-lg text-[13px] font-medium transition-all duration-200 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
                  isActive
                    ? "bg-primary text-primary-foreground font-semibold shadow-sm"
                    : "text-muted-foreground hover:text-foreground hover:bg-muted/80"
                )}
                type="button"
              >
                <tab.icon className="h-4 w-4 inline-block mr-1.5" />
                {tab.name}
              </button>
            )
          })}
        </div>

        {/* Tab Content */}
        {activeTab === "Traces" && (
          <>
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
          </>
        )}

        {activeTab === "Sessions" && (
          <div className="bg-card border rounded-xl shadow-sm overflow-hidden flex-1">
            {loadingSessions ? (
              <div className="h-full flex items-center justify-center py-16">
                <div className="animate-pulse text-muted-foreground">Loading sessions...</div>
              </div>
            ) : sessions.length === 0 ? (
              <div className="h-full flex flex-col items-center justify-center py-16">
                <Layers className="h-12 w-12 text-muted-foreground/50 mb-4" />
                <p className="text-muted-foreground">No sessions found</p>
                <p className="text-sm text-muted-foreground/70 mt-1">
                  Sessions are created when requests include an X-Session-Id header
                </p>
              </div>
            ) : (
              <SessionTable sessions={sessions} onSelect={handleSelectSession} />
            )}
          </div>
        )}
      </div>

      {/* Detail Modals */}
      <TraceDetailModal
        log={selectedLog}
        detail={selectedLogDetail}
        loading={loadingDetail}
        open={selectedLog !== null}
        onOpenChange={(open) => !open && handleCloseModal()}
      />

      {selectedProjectId && (
        <SessionDetailModal
          session={selectedSession}
          projectId={selectedProjectId}
          open={selectedSession !== null}
          onOpenChange={(open) => !open && setSelectedSession(null)}
          onSessionUpdated={handleSessionUpdated}
        />
      )}
    </div>
  )
}