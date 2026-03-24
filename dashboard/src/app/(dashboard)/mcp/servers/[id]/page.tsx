"use client"

import { useEffect, useState } from "react"
import { useRouter, useParams } from "next/navigation"
import Link from "next/link"
import {
  ArrowLeft,
  Wrench,
  Trash2,
  RefreshCw,
  Key,
  ExternalLink,
  Copy,
  CheckCircle,
  XCircle,
  Loader2,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { toast } from "sonner"
import {
  getMcpServer,
  getMcpServerTools,
  refreshMcpServer,
  deleteMcpServer,
  reauthMcpServer,
  type McpServerInfo,
  type McpToolDef,
} from "@/lib/api"
import { StatusBadge, AuthTypeBadge } from "@/components/mcp/badges"

function ToolCard({ tool }: { tool: McpToolDef }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="border rounded-lg p-4 space-y-2">
      <div className="flex items-start justify-between">
        <div>
          <code className="text-sm font-mono font-medium">{tool.name}</code>
          {tool.description && (
            <p className="text-xs text-muted-foreground mt-1">{tool.description}</p>
          )}
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? "Hide Schema" : "Show Schema"}
        </Button>
      </div>

      {expanded && (
        <div className="mt-3 p-3 bg-muted/50 rounded-md">
          <p className="text-xs font-medium text-muted-foreground mb-2">Input Schema</p>
          <pre className="text-xs font-mono overflow-auto max-h-48">
            {JSON.stringify(tool.input_schema, null, 2)}
          </pre>
        </div>
      )}
    </div>
  )
}

export default function McpServerDetailPage() {
  const router = useRouter()
  const params = useParams()
  const serverId = params.id as string

  const [server, setServer] = useState<McpServerInfo | null>(null)
  const [tools, setTools] = useState<McpToolDef[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [reauthing, setReauthing] = useState(false)
  const [deleting, setDeleting] = useState(false)

  useEffect(() => {
    fetchTools()
  }, [serverId])

  async function fetchTools() {
    setLoading(true)
    try {
      const [serverData, toolsData] = await Promise.all([
        getMcpServer(serverId),
        getMcpServerTools(serverId),
      ])
      setServer(serverData)
      setTools(toolsData)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load server")
    } finally {
      setLoading(false)
    }
  }

  async function handleRefresh() {
    setRefreshing(true)
    try {
      const updatedTools = await refreshMcpServer(serverId)
      setTools(updatedTools)
      toast.success(`Refreshed ${updatedTools.length} tools`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to refresh")
    } finally {
      setRefreshing(false)
    }
  }

  async function handleReauth() {
    setReauthing(true)
    try {
      const result = await reauthMcpServer(serverId)
      if (result.success) {
        toast.success("Re-authentication successful")
      } else {
        toast.error(result.error || "Re-authentication failed")
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Re-authentication failed")
    } finally {
      setReauthing(false)
    }
  }

  async function handleDelete() {
    if (!confirm("Are you sure you want to delete this server? This action cannot be undone.")) return

    setDeleting(true)
    try {
      await deleteMcpServer(serverId)
      toast.success("Server deleted")
      router.push("/mcp/servers")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete")
    } finally {
      setDeleting(false)
    }
  }

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
    toast.success("Copied to clipboard")
  }

  if (loading) {
    return (
      <div className="flex-1 p-6 lg:p-8 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex-1 p-6 lg:p-8 flex flex-col items-center justify-center gap-4">
        <XCircle className="h-12 w-12 text-destructive" />
        <div className="text-destructive">{error}</div>
        <Button variant="outline" onClick={() => router.push("/mcp/servers")}>
          Back to Servers
        </Button>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-6 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Link href="/mcp/servers">
            <Button variant="ghost" size="icon-sm">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <div className="flex-1">
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold tracking-tight">{server?.name || "MCP Server"}</h1>
              {server && <StatusBadge status={server.status} />}
            </div>
            <p className="text-sm text-muted-foreground">MCP Server Configuration</p>
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              className="gap-2"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              {refreshing ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="h-4 w-4" />
              )}
              Refresh
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="gap-2"
              onClick={handleReauth}
              disabled={reauthing}
            >
              {reauthing ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Key className="h-4 w-4" />
              )}
              Re-auth
            </Button>
          </div>
        </div>

        {/* Server Info Card */}
        <div className="bg-card border rounded-xl p-6 space-y-6">
          <h2 className="font-semibold">Server Information</h2>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-6">
            <div className="space-y-1">
              <p className="text-xs text-muted-foreground">Server ID</p>
              <div className="flex items-center gap-2">
                <code className="text-sm font-mono truncate">{serverId}</code>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => copyToClipboard(serverId)}
                >
                  <Copy className="h-3 w-3" />
                </Button>
              </div>
            </div>

            <div className="space-y-1">
              <p className="text-xs text-muted-foreground">Authentication</p>
              <AuthTypeBadge authType={server?.auth_type || "none"} />
            </div>

            <div className="space-y-1">
              <p className="text-xs text-muted-foreground">Tools Available</p>
              <p className="text-sm font-medium">{server?.tool_count || tools.length}</p>
            </div>

            <div className="space-y-1">
              <p className="text-xs text-muted-foreground">Status</p>
              {server && <StatusBadge status={server.status} />}
            </div>
          </div>
        </div>

        {/* Tools Section */}
        <div className="bg-card border rounded-xl p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="font-semibold">Available Tools</h2>
            <Badge variant="outline">{tools.length} tools</Badge>
          </div>

          {tools.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              <Wrench className="h-8 w-8 mx-auto mb-2 opacity-50" />
              <p>No tools available</p>
              <p className="text-xs mt-1">Refresh the server to fetch tools</p>
            </div>
          ) : (
            <div className="space-y-3">
              {tools.map((tool) => (
                <ToolCard key={tool.name} tool={tool} />
              ))}
            </div>
          )}
        </div>

        {/* Danger Zone */}
        <div className="bg-destructive/5 border border-destructive/20 rounded-xl p-6 space-y-4">
          <h2 className="font-semibold text-destructive">Danger Zone</h2>
          <p className="text-sm text-muted-foreground">
            Deleting this server will remove all tool configurations. This action cannot be undone.
          </p>
          <Button
            variant="destructive"
            size="sm"
            className="gap-2"
            onClick={handleDelete}
            disabled={deleting}
          >
            {deleting ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Trash2 className="h-4 w-4" />
            )}
            Delete Server
          </Button>
        </div>
      </div>
    </div>
  )
}