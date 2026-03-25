"use client"

import { Check, X, Copy } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { toast } from "sonner"
import type { ApprovalRequest } from "@/lib/api"

interface ApprovalDetailModalProps {
  approval: ApprovalRequest | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onDecision: (id: string, decision: "approved" | "rejected") => void
  processing: boolean
}

function formatJsonPreview(preview: string | undefined): string {
  if (!preview) return "No body preview available"
  try {
    return JSON.stringify(JSON.parse(preview), null, 2)
  } catch {
    return preview
  }
}

export function ApprovalDetailModal({
  approval,
  open,
  onOpenChange,
  onDecision,
  processing,
}: ApprovalDetailModalProps) {
  if (!approval) return null

  const isPending = approval.status === "pending"

  const handleCopyId = () => {
    navigator.clipboard.writeText(approval.id)
    toast.success("Request ID copied")
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            Request Details
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={handleCopyId}
              title="Copy request ID"
            >
              <Copy className="h-3.5 w-3.5" />
            </Button>
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {/* Status Badge */}
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Status:</span>
            <span className={`px-2 py-0.5 rounded text-xs font-medium ${
              approval.status === "pending" ? "bg-muted text-muted-foreground" :
              approval.status === "approved" ? "bg-success/10 text-success" :
              approval.status === "rejected" ? "bg-destructive/10 text-destructive" :
              "bg-warning/20 text-warning"
            }`}>
              {approval.status}
            </span>
          </div>

          {/* Request Metadata Grid */}
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Method
              </label>
              <p className="text-sm font-medium">{approval.request_summary?.method || "POST"}</p>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Path
              </label>
              <p className="text-sm font-mono truncate">{approval.request_summary?.path || "/v1/chat/completions"}</p>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Agent
              </label>
              <p className="text-sm">{approval.request_summary?.agent || "—"}</p>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Upstream
              </label>
              <p className="text-sm">{approval.request_summary?.upstream || "—"}</p>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Token ID
              </label>
              <p className="text-sm font-mono">{approval.token_id || "—"}</p>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Request ID
              </label>
              <p className="text-sm font-mono">{approval.id}</p>
            </div>
          </div>

          {/* Timestamps */}
          <div className="grid grid-cols-2 gap-4 pt-2 border-t">
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Created At
              </label>
              <p className="text-sm text-muted-foreground">
                {new Date(approval.created_at).toLocaleString()}
              </p>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Expires At
              </label>
              <p className="text-sm text-muted-foreground">
                {new Date(approval.expires_at).toLocaleString()}
              </p>
            </div>
          </div>

          {/* Body Preview */}
          {approval.request_summary?.body_preview && (
            <div className="space-y-2 pt-2 border-t">
              <label className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                Body Preview
              </label>
              <pre className="text-xs font-mono bg-muted/50 p-3 rounded-lg overflow-x-auto max-h-[200px] overflow-y-auto">
                {formatJsonPreview(approval.request_summary.body_preview)}
              </pre>
            </div>
          )}
        </div>

        {/* Footer with Actions */}
        {isPending && (
          <DialogFooter className="gap-2 sm:gap-2">
            <Button
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={processing}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                onDecision(approval.id, "rejected")
              }}
              disabled={processing}
              loading={processing}
            >
              <X className="h-4 w-4 mr-1" />
              Reject
            </Button>
            <Button
              variant="default"
              className="bg-success hover:bg-success/90"
              onClick={() => {
                onDecision(approval.id, "approved")
              }}
              disabled={processing}
              loading={processing}
            >
              <Check className="h-4 w-4 mr-1" />
              Approve
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  )
}