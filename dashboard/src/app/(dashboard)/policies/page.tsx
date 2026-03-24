"use client"

import { useEffect, useState } from "react"
import { Plus, Shield, MoreHorizontal, Eye, Trash2, Edit, Copy, History } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  listPolicies,
  deletePolicy,
  type PolicyRow,
  type Action,
} from "@/lib/api"
import { isGuardrailAction, getActionDisplayName } from "@/lib/types/policy"

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return "just now"
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return date.toLocaleDateString()
}

function ModeBadge({ mode }: { mode: string }) {
  const variants: Record<string, "default" | "secondary" | "outline"> = {
    enforce: "default",
    shadow: "secondary",
  }

  return (
    <Badge variant={variants[mode] || "outline"} className="text-[10px]">
      {mode === "enforce" ? "Enforce" : "Shadow"}
    </Badge>
  )
}

function PhaseBadge({ phase }: { phase: string }) {
  const variants: Record<string, "default" | "secondary" | "outline"> = {
    pre: "outline",
    post: "secondary",
  }

  return (
    <Badge variant={variants[phase] || "outline"} className="text-[10px]">
      {phase === "pre" ? "Pre-flight" : "Post-flight"}
    </Badge>
  )
}

function getUniqueActions(rules: { then: Action | Action[] }[]): string[] {
  const actions = new Set<string>()
  for (const rule of rules) {
    const thenArray = Array.isArray(rule.then) ? rule.then : [rule.then]
    for (const action of thenArray) {
      actions.add(getActionDisplayName(action))
    }
  }
  return Array.from(actions).slice(0, 3)
}

function hasGuardrailActions(rules: { then: Action | Action[] }[]): boolean {
  for (const rule of rules) {
    const thenArray = Array.isArray(rule.then) ? rule.then : [rule.then]
    for (const action of thenArray) {
      if (isGuardrailAction(action)) return true
    }
  }
  return false
}

export default function PoliciesPage() {
  const [policies, setPolicies] = useState<PolicyRow[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    async function fetchPolicies() {
      try {
        const data = await listPolicies({ limit: 100 })
        setPolicies(data)
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load policies")
      } finally {
        setLoading(false)
      }
    }
    fetchPolicies()
  }, [])

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this policy? This action cannot be undone.")) return

    try {
      await deletePolicy(id)
      setPolicies(policies.filter((p) => p.id !== id))
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to delete policy")
    }
  }

  const handleDuplicate = async (policy: PolicyRow) => {
    // TODO: Implement duplication - create new policy with same rules
    alert(`Duplicating "${policy.name}" - coming soon!`)
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Policies
            </h1>
            <p className="text-sm text-muted-foreground">
              Manage rules for request validation, routing, and guardrails
            </p>
          </div>
          <Link href="/policies/new">
            <Button className="gap-2">
              <Plus className="h-4 w-4" />
              Create Policy
            </Button>
          </Link>
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading policies...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : policies.length === 0 ? (
            <div className="p-8 text-center">
              <Shield className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No policies yet</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Create your first policy to enforce rules on requests
              </p>
              <Link href="/policies/new">
                <Button className="mt-4 gap-2">
                  <Plus className="h-4 w-4" />
                  Create Policy
                </Button>
              </Link>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Name</th>
                  <th className="px-4 py-3 text-left">Mode</th>
                  <th className="px-4 py-3 text-left">Phase</th>
                  <th className="px-4 py-3 text-left">Actions</th>
                  <th className="px-4 py-3 text-left">Guardrails</th>
                  <th className="px-4 py-3 text-left">Rules</th>
                  <th className="px-4 py-3 text-left">Created</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {policies.map((policy) => {
                  const uniqueActions = getUniqueActions(policy.rules)
                  const hasGuardrails = hasGuardrailActions(policy.rules)

                  return (
                    <tr
                      key={policy.id}
                      className="border-b last:border-0 hover:bg-muted/30 transition-colors"
                    >
                      <td className="px-4 py-3">
                        <Link
                          href={`/policies/${policy.id}`}
                          className="text-sm font-medium hover:underline"
                        >
                          {policy.name}
                        </Link>
                      </td>
                      <td className="px-4 py-3">
                        <ModeBadge mode={policy.mode} />
                      </td>
                      <td className="px-4 py-3">
                        <PhaseBadge phase={policy.phase} />
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-wrap gap-1">
                          {uniqueActions.map((action) => (
                            <Badge key={action} variant="outline" className="text-[10px]">
                              {action}
                            </Badge>
                          ))}
                          {uniqueActions.length < getUniqueActions(policy.rules).length && (
                            <Badge variant="outline" className="text-[10px]">
                              +{getUniqueActions(policy.rules).length - uniqueActions.length}
                            </Badge>
                          )}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        {hasGuardrails ? (
                          <Badge variant="default" className="text-[10px] bg-green-600">
                            Active
                          </Badge>
                        ) : (
                          <span className="text-sm text-muted-foreground">—</span>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-sm text-muted-foreground">
                          {policy.rules.length} rule{policy.rules.length !== 1 ? "s" : ""}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-sm text-muted-foreground">
                          {formatRelativeTime(policy.created_at)}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <DropdownMenu>
                          <DropdownMenuTrigger>
                            <Button variant="ghost" size="icon-sm">
                              <MoreHorizontal className="h-4 w-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem>
                              <Link href={`/policies/${policy.id}`} className="flex items-center">
                                <Edit className="h-4 w-4 mr-2" />
                                Edit
                              </Link>
                            </DropdownMenuItem>
                            <DropdownMenuItem>
                              <Link href={`/policies/${policy.id}?tab=history`} className="flex items-center">
                                <History className="h-4 w-4 mr-2" />
                                View History
                              </Link>
                            </DropdownMenuItem>
                            <DropdownMenuItem onClick={() => handleDuplicate(policy)}>
                              <Copy className="h-4 w-4 mr-2" />
                              Duplicate
                            </DropdownMenuItem>
                            <DropdownMenuSeparator />
                            <DropdownMenuItem
                              className="text-destructive"
                              onClick={() => handleDelete(policy.id)}
                            >
                              <Trash2 className="h-4 w-4 mr-2" />
                              Delete
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  )
}