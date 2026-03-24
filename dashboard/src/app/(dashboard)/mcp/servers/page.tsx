"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { Plus, Wrench, MoreHorizontal, Trash2, RefreshCw, Key, Eye, Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { toast } from "sonner"
import {
  listMcpServers,
  deleteMcpServer,
  refreshMcpServer,
  reauthMcpServer,
  type McpServerInfo,
} from "@/lib/api"
import { StatusBadge, AuthTypeBadge } from "@/components/mcp/badges"

function formatRelativeTime(seconds: number): string {
  if (seconds < 60) return `${seconds}s ago`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  return `${Math.floor(seconds / 86400)}d ago`
}

export default function McpServersPage() {
  const router = useRouter()
  const [servers, setServers] = useState<McpServerInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [refreshingId, setRefreshingId] = useState<string | null>(null)
  const [reauthingId, setReauthingId] = useState<string | null>(null)

  useEffect(() => {
    fetchServers()
  }, [])

  async function fetchServers() {
    try {
      const data = await listMcpServers()
      setServers(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load MCP servers")
    } finally {
      setLoading(false)
    }
  }

  async function handleDelete(id: string, name: string) {
    if (!confirm(`Are you sure you want to delete "${name}"?`)) return

    try {
      await deleteMcpServer(id)
      setServers(servers.filter((s) => s.id !== id))
      toast.success(`Server "${name}" deleted`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete server")
    }
  }

  async function handleRefresh(id: string) {
    setRefreshingId(id)
    try {
      const tools = await refreshMcpServer(id)
      setServers(
        servers.map((s) =>
          s.id === id
            ? { ...s, tool_count: tools.length, tools: tools.map((t) => t.name), status: "Connected" }
            : s
        )
      )
      toast.success(`Refreshed ${tools.length} tools`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to refresh server")
    } finally {
      setRefreshingId(null)
    }
  }

  async function handleReauth(id: string, name: string) {
    setReauthingId(id)
    try {
      const result = await reauthMcpServer(id)
      if (result.success) {
        toast.success(`Re-authenticated "${name}"`)
      } else {
        toast.error(result.error || "Re-authentication failed")
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Re-authentication failed")
    } finally {
      setReauthingId(null)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              MCP Servers
            </h1>
            <p className="text-sm text-muted-foreground">
              Manage Model Context Protocol servers and tools
            </p>
          </div>
          <Link href="/mcp/servers/new">
            <Button className="gap-2">
              <Plus className="h-4 w-4" />
              Add Server
            </Button>
          </Link>
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading MCP servers...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : servers.length === 0 ? (
            <div className="p-8 text-center">
              <Wrench className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No MCP servers registered</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Add your first MCP server to enable tool integration
              </p>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Name</th>
                  <th className="px-4 py-3 text-left">Endpoint</th>
                  <th className="px-4 py-3 text-left">Auth</th>
                  <th className="px-4 py-3 text-left">Status</th>
                  <th className="px-4 py-3 text-left">Tools</th>
                  <th className="px-4 py-3 text-left">Last Refresh</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {servers.map((server) => (
                  <tr
                    key={server.id}
                    className="border-b last:border-0 hover:bg-muted/30 transition-colors"
                  >
                    <td className="px-4 py-3">
                      <span className="text-sm font-medium">{server.name}</span>
                      {server.server_info && (
                        <span className="text-xs text-muted-foreground ml-2">
                          v{server.server_info.version}
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <code className="text-xs text-muted-foreground font-mono truncate max-w-[200px] block">
                        {server.endpoint.replace(/^https?:\/\//, "")}
                      </code>
                    </td>
                    <td className="px-4 py-3">
                      <AuthTypeBadge authType={server.auth_type} />
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={server.status} />
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm">{server.tool_count}</span>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {formatRelativeTime(server.last_refreshed_secs_ago)}
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
                          <DropdownMenuItem onClick={() => router.push(`/mcp/servers/${server.id}`)}>
                            <Eye className="h-4 w-4 mr-2" />
                            View Details
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={() => handleRefresh(server.id)}
                            disabled={refreshingId === server.id}
                          >
                            {refreshingId === server.id ? (
                              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                            ) : (
                              <RefreshCw className="h-4 w-4 mr-2" />
                            )}
                            Refresh Tools
                          </DropdownMenuItem>
                          {server.auth_type === "oauth2" && (
                            <DropdownMenuItem
                              onClick={() => handleReauth(server.id, server.name)}
                              disabled={reauthingId === server.id}
                            >
                              {reauthingId === server.id ? (
                                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                              ) : (
                                <Key className="h-4 w-4 mr-2" />
                              )}
                              Re-authenticate
                            </DropdownMenuItem>
                          )}
                          <DropdownMenuItem
                            className="text-destructive"
                            onClick={() => handleDelete(server.id, server.name)}
                          >
                            <Trash2 className="h-4 w-4 mr-2" />
                            Delete
                          </DropdownMenuItem>
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