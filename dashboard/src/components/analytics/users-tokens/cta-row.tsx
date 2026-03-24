"use client"

import Link from "next/link"

export function CTARow() {
  return (
    <div className="h-[60px] bg-card border border rounded-[14px] flex items-center justify-between px-6 shadow-sm">
      <div className="flex items-center gap-3">
        <span className="text-[12px] text-muted-foreground">
          Manage your tokens and view detailed usage
        </span>
      </div>
      <div className="flex items-center gap-3">
        <Link
          href="/virtual-keys"
          className="px-4 py-2 bg-primary rounded-lg text-[11px] font-medium text-white hover:bg-foreground/90 transition-colors"
        >
          Go to Token Management
        </Link>
      </div>
    </div>
  )
}