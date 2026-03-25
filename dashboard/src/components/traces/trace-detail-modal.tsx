"use client"

import type { AuditLogRowType, AuditLogDetailRow } from "@/lib/api"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  formatLatency,
  formatTokens,
  formatCost,
  getPolicyResultColor,
  getPolicyResultDisplay,
} from "@/lib/types/audit"
import { Loader2 } from "lucide-react"

interface TraceDetailModalProps {
  log: AuditLogRowType | null
  detail: AuditLogDetailRow | null
  loading: boolean
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function TraceDetailModal({
  log,
  detail,
  loading,
  open,
  onOpenChange,
}: TraceDetailModalProps) {
  if (!log) return null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-4xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            Request Details
            <span className="font-mono text-sm text-muted-foreground">
              {log.id.slice(0, 8)}...
            </span>
          </DialogTitle>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {/* Metadata Section */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Status</span>
                <StatusBadge status={log.upstream_status} errorType={log.error_type} />
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Method</span>
                <Badge variant="outline" className="w-fit mt-1">{log.method}</Badge>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Latency</span>
                <span className="text-sm font-medium">{formatLatency(log.response_latency_ms)}</span>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Cost</span>
                <span className="text-sm font-medium">{formatCost(log.estimated_cost_usd)}</span>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Model</span>
                <span className="text-sm">{log.model || "—"}</span>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Tokens</span>
                <span className="text-sm">{formatTokens(log.prompt_tokens, log.completion_tokens)}</span>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">TTFT</span>
                <span className="text-sm">{detail?.ttft_ms ? formatLatency(detail.ttft_ms) : "—"}</span>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-muted-foreground">Streaming</span>
                <span className="text-sm">{log.is_streaming ? "Yes" : "No"}</span>
              </div>
            </div>

            {/* Path */}
            <div className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Path</span>
              <code className="text-sm bg-muted px-2 py-1 rounded">{log.path}</code>
            </div>

            {/* Upstream URL */}
            {detail?.upstream_url && (
              <div className="flex flex-col gap-1">
                <span className="text-xs text-muted-foreground">Upstream URL</span>
                <code className="text-sm bg-muted px-2 py-1 rounded truncate">{detail.upstream_url}</code>
              </div>
            )}

            {/* Policy Result */}
            <div className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Policy Result</span>
              <div className="flex items-center gap-2">
                <span className={`px-2 py-0.5 rounded text-xs font-medium ${getPolicyResultColor(log.policy_result)}`}>
                  {getPolicyResultDisplay(log.policy_result)}
                </span>
                {detail?.policy_mode && (
                  <Badge variant="outline" className="text-[10px]">{detail.policy_mode}</Badge>
                )}
              </div>
              {detail?.deny_reason && (
                <p className="text-sm text-destructive mt-1">{detail.deny_reason}</p>
              )}
            </div>

            {/* Bodies Tabs */}
            {(detail?.request_body || detail?.response_body) && (
              <Tabs defaultValue="request" className="mt-2">
                <TabsList>
                  <TabsTrigger value="request">Request</TabsTrigger>
                  <TabsTrigger value="response">Response</TabsTrigger>
                  <TabsTrigger value="headers">Headers</TabsTrigger>
                </TabsList>
                <TabsContent value="request" className="mt-2">
                  {detail?.request_body ? (
                    <JsonViewer data={detail.request_body} />
                  ) : (
                    <p className="text-sm text-muted-foreground py-4">
                      Request body not available (log level may be too low)
                    </p>
                  )}
                </TabsContent>
                <TabsContent value="response" className="mt-2">
                  {detail?.response_body ? (
                    <JsonViewer data={detail.response_body} />
                  ) : (
                    <p className="text-sm text-muted-foreground py-4">
                      Response body not available (log level may be too low)
                    </p>
                  )}
                </TabsContent>
                <TabsContent value="headers" className="mt-2">
                  <div className="grid gap-4">
                    <div>
                      <span className="text-xs text-muted-foreground">Request Headers</span>
                      {detail?.request_headers ? (
                        <JsonViewer data={JSON.stringify(detail.request_headers, null, 2)} />
                      ) : (
                        <p className="text-sm text-muted-foreground py-2">Not available</p>
                      )}
                    </div>
                    <div>
                      <span className="text-xs text-muted-foreground">Response Headers</span>
                      {detail?.response_headers ? (
                        <JsonViewer data={JSON.stringify(detail.response_headers, null, 2)} />
                      ) : (
                        <p className="text-sm text-muted-foreground py-2">Not available</p>
                      )}
                    </div>
                  </div>
                </TabsContent>
              </Tabs>
            )}

            {/* Log Level Notice */}
            {log.log_level !== null && log.log_level < 1 && (
              <div className="bg-muted/50 border rounded-lg p-3 text-sm text-muted-foreground">
                <strong>Note:</strong> Log level is set to metadata only. Request/response bodies are not captured.
                Increase log level to see full request details.
              </div>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

function StatusBadge({ status, errorType }: { status: number | null; errorType: string | null }) {
  if (errorType) {
    return (
      <span className="px-2 py-0.5 rounded text-xs font-medium bg-destructive/10 text-destructive w-fit">
        Error: {errorType}
      </span>
    )
  }
  if (status === null) {
    return <span className="text-sm text-muted-foreground">—</span>
  }
  const colorClass = status < 300
    ? "bg-success/10 text-success"
    : status < 400
    ? "bg-warning/10 text-warning"
    : "bg-destructive/10 text-destructive"

  return (
    <span className={`px-2 py-0.5 rounded text-xs font-medium ${colorClass} w-fit`}>
      {status}
    </span>
  )
}

function JsonViewer({ data }: { data: string }) {
  try {
    const parsed = JSON.parse(data)
    return (
      <pre className="text-xs bg-muted/50 p-3 rounded-lg overflow-auto max-h-[300px] font-mono">
        {JSON.stringify(parsed, null, 2)}
      </pre>
    )
  } catch {
    return (
      <pre className="text-xs bg-muted/50 p-3 rounded-lg overflow-auto max-h-[300px] font-mono whitespace-pre-wrap">
        {data}
      </pre>
    )
  }
}