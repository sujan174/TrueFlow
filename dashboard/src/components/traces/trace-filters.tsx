"use client"

import type { AuditFilters } from "@/lib/api"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { X } from "lucide-react"

interface TraceFiltersProps {
  filters: AuditFilters
  onChange: (filters: AuditFilters) => void
}

const POLICY_RESULTS = [
  { value: "allow", label: "Allowed" },
  { value: "deny", label: "Denied" },
  { value: "shadow_deny", label: "Shadow Deny" },
  { value: "hitl_approved", label: "HITL Approved" },
  { value: "hitl_rejected", label: "HITL Rejected" },
]

const HTTP_METHODS = ["GET", "POST", "PUT", "DELETE", "PATCH"]

export function TraceFilters({ filters, onChange }: TraceFiltersProps) {
  const updateFilter = (key: keyof AuditFilters, value: string | null | undefined) => {
    onChange({
      ...filters,
      [key]: value === "" || value === "all" || value == null ? undefined : value,
    })
  }

  const clearFilters = () => {
    onChange({})
  }

  const hasFilters = Object.values(filters).some((v) => v !== undefined && v !== "")

  return (
    <div className="flex flex-wrap items-center gap-3">
      {/* Status Filter */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Status:</span>
        <Select
          value={filters.status !== undefined ? String(filters.status) : "all"}
          onValueChange={(v) => updateFilter("status", v === "all" ? undefined : v)}
        >
          <SelectTrigger className="w-[100px] h-8">
            <SelectValue placeholder="All" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            <SelectItem value="200">200</SelectItem>
            <SelectItem value="400">400</SelectItem>
            <SelectItem value="401">401</SelectItem>
            <SelectItem value="403">403</SelectItem>
            <SelectItem value="404">404</SelectItem>
            <SelectItem value="429">429</SelectItem>
            <SelectItem value="500">500</SelectItem>
            <SelectItem value="502">502</SelectItem>
            <SelectItem value="503">503</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Method Filter */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Method:</span>
        <Select
          value={filters.method ?? "all"}
          onValueChange={(v) => updateFilter("method", v)}
        >
          <SelectTrigger className="w-[80px] h-8">
            <SelectValue placeholder="All" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            {HTTP_METHODS.map((method) => (
              <SelectItem key={method} value={method}>
                {method}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Policy Result Filter */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Policy:</span>
        <Select
          value={filters.policy_result ?? "all"}
          onValueChange={(v) => updateFilter("policy_result", v)}
        >
          <SelectTrigger className="w-[130px] h-8">
            <SelectValue placeholder="All" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            {POLICY_RESULTS.map((result) => (
              <SelectItem key={result.value} value={result.value}>
                {result.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Model Filter */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Model:</span>
        <Input
          placeholder="Search model..."
          value={filters.model ?? ""}
          onChange={(e) => updateFilter("model", e.target.value)}
          className="w-[150px] h-8"
        />
      </div>

      {/* Path Filter */}
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Path:</span>
        <Input
          placeholder="Search path..."
          value={filters.path_contains ?? ""}
          onChange={(e) => updateFilter("path_contains", e.target.value)}
          className="w-[150px] h-8"
        />
      </div>

      {/* Clear Filters */}
      {hasFilters && (
        <Button
          variant="ghost"
          size="sm"
          onClick={clearFilters}
          className="h-8 gap-1 text-muted-foreground"
        >
          <X className="h-3 w-3" />
          Clear
        </Button>
      )}
    </div>
  )
}