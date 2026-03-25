"use client"

import { Check, X, XCircle } from "lucide-react"
import { Button } from "@/components/ui/button"

interface BulkActionBarProps {
  selectedCount: number
  onApprove: () => void
  onReject: () => void
  onClearSelection: () => void
  processing: boolean
}

export function BulkActionBar({
  selectedCount,
  onApprove,
  onReject,
  onClearSelection,
  processing,
}: BulkActionBarProps) {
  if (selectedCount === 0) return null

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 bg-card border rounded-xl shadow-lg px-5 py-3 flex items-center gap-4 animate-in fade-in slide-in-from-bottom-4 duration-300">
      <span className="text-sm font-medium">
        {selectedCount} pending request{selectedCount !== 1 ? "s" : ""} selected
      </span>

      <div className="h-5 w-px bg-border" />

      <Button
        variant="ghost"
        size="sm"
        onClick={onClearSelection}
        disabled={processing}
        className="text-muted-foreground"
      >
        <XCircle className="h-4 w-4 mr-1" />
        Clear
      </Button>

      <Button
        variant="outline"
        size="sm"
        onClick={onApprove}
        disabled={processing}
        className="bg-success/10 text-success border-success/20 hover:bg-success/20"
      >
        <Check className="h-4 w-4 mr-1" />
        Approve All
      </Button>

      <Button
        variant="outline"
        size="sm"
        onClick={onReject}
        disabled={processing}
        className="bg-destructive/10 text-destructive border-destructive/20 hover:bg-destructive/20"
      >
        <X className="h-4 w-4 mr-1" />
        Reject All
      </Button>
    </div>
  )
}