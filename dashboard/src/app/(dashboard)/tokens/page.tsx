"use client"

import { useEffect, useState } from "react"
import { Plus, Key, MoreHorizontal, Trash2, Eye } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { toast } from "sonner"
import {
  listTokensWithParams,
  revokeToken,
  type TokenRow,
} from "@/lib/api"

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

function PurposeBadge({ purpose }: { purpose: string }) {
  const variants: Record<string, "default" | "secondary" | "outline"> = {
    llm: "default",
    tool: "secondary",
    both: "outline",
  }

  const labels: Record<string, string> = {
    llm: "LLM",
    tool: "Tool",
    both: "Both",
  }

  return (
    <Badge variant={variants[purpose] || "outline"} className="text-[10px]">
      {labels[purpose] || purpose}
    </Badge>
  )
}

function StatusBadge({ isActive }: { isActive: boolean }) {
  return (
    <Badge
      variant={isActive ? "success" : "destructive"}
      className="text-[10px]"
    >
      {isActive ? "Active" : "Revoked"}
    </Badge>
  )
}

export default function TokensPage() {
  const [tokens, setTokens] = useState<TokenRow[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    async function fetchTokens() {
      try {
        const data = await listTokensWithParams({ limit: 100 })
        setTokens(data)
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load tokens")
      } finally {
        setLoading(false)
      }
    }
    fetchTokens()
  }, [])

  const handleRevoke = async (id: string) => {
    if (!confirm("Are you sure you want to revoke this token?")) return

    try {
      await revokeToken(id)
      setTokens(tokens.map((t) => (t.id === id ? { ...t, is_active: false } : t)))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to revoke token")
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Tokens
            </h1>
            <p className="text-sm text-muted-foreground">
              Manage virtual API keys for your gateway
            </p>
          </div>
          <Button className="gap-2">
            <Plus className="h-4 w-4" />
            Create Token
          </Button>
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading tokens...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : tokens.length === 0 ? (
            <div className="p-8 text-center">
              <Key className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No tokens yet</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Create your first token to get started
              </p>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Name</th>
                  <th className="px-4 py-3 text-left">Token ID</th>
                  <th className="px-4 py-3 text-left">Purpose</th>
                  <th className="px-4 py-3 text-left">Status</th>
                  <th className="px-4 py-3 text-left">External User</th>
                  <th className="px-4 py-3 text-left">Created</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {tokens.map((token) => (
                  <tr
                    key={token.id}
                    className="border-b last:border-0 hover:bg-muted/30 transition-colors"
                  >
                    <td className="px-4 py-3">
                      <span className="text-sm font-medium">{token.name}</span>
                    </td>
                    <td className="px-4 py-3">
                      <code className="text-xs text-muted-foreground font-mono">
                        {token.id.slice(0, 20)}...
                      </code>
                    </td>
                    <td className="px-4 py-3">
                      <PurposeBadge purpose={token.purpose} />
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge isActive={token.is_active} />
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {token.external_user_id || "—"}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {formatRelativeTime(token.created_at)}
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
                            <Eye className="h-4 w-4 mr-2" />
                            View Details
                          </DropdownMenuItem>
                          {token.is_active && (
                            <DropdownMenuItem
                              className="text-destructive"
                              onClick={() => handleRevoke(token.id)}
                            >
                              <Trash2 className="h-4 w-4 mr-2" />
                              Revoke
                            </DropdownMenuItem>
                          )}
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  )
}