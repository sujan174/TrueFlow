"use client"

import { useState, useEffect } from "react"
import { Wrench, Plus, X, Info, Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import {
  listMcpServers,
  type McpServerInfo,
} from "@/lib/api"

interface ToolPickerProps {
  allowedTools: string[] | null
  blockedTools: string[] | null
  onAllowedChange: (tools: string[] | null) => void
  onBlockedChange: (tools: string[] | null) => void
}

export function ToolPicker({
  allowedTools,
  blockedTools,
  onAllowedChange,
  onBlockedChange,
}: ToolPickerProps) {
  const [servers, setServers] = useState<McpServerInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [newAllowedPattern, setNewAllowedPattern] = useState("")
  const [newBlockedPattern, setNewBlockedPattern] = useState("")

  useEffect(() => {
    async function loadServers() {
      try {
        const data = await listMcpServers()
        setServers(data)
      } catch {
        // Ignore errors - servers may not be configured
      } finally {
        setLoading(false)
      }
    }
    loadServers()
  }, [])

  const addAllowedPattern = () => {
    if (!newAllowedPattern.trim()) return
    const current = allowedTools || []
    onAllowedChange([...current, newAllowedPattern.trim()])
    setNewAllowedPattern("")
  }

  const addBlockedPattern = () => {
    if (!newBlockedPattern.trim()) return
    const current = blockedTools || []
    onBlockedChange([...current, newBlockedPattern.trim()])
    setNewBlockedPattern("")
  }

  const removeAllowedPattern = (index: number) => {
    const current = allowedTools || []
    const updated = current.filter((_, i) => i !== index)
    onAllowedChange(updated.length > 0 ? updated : null)
  }

  const removeBlockedPattern = (index: number) => {
    const current = blockedTools || []
    const updated = current.filter((_, i) => i !== index)
    onBlockedChange(updated.length > 0 ? updated : null)
  }

  const addServerTools = (serverName: string) => {
    const pattern = `mcp__${serverName}__*`
    const current = allowedTools || []
    if (!current.includes(pattern)) {
      onAllowedChange([...current, pattern])
    }
  }

  return (
    <div className="space-y-6">
      {/* Info Banner */}
      <div className="bg-muted/50 border rounded-lg p-4 flex gap-3">
        <Info className="h-4 w-4 text-muted-foreground mt-0.5 shrink-0" />
        <div className="text-sm text-muted-foreground">
          <p className="font-medium text-foreground">MCP Tool Access Control</p>
          <p className="mt-1">
            Control which MCP tools this token can access. Use glob patterns like{" "}
            <code className="px-1 py-0.5 bg-muted rounded text-xs">mcp__slack__*</code> to allow all tools from a server.
          </p>
          <p className="mt-1 text-xs">
            <strong>Allowlist:</strong> NULL = all allowed, [] = none allowed
          </p>
          <p className="text-xs">
            <strong>Blocklist:</strong> Takes priority over allowlist
          </p>
        </div>
      </div>

      {/* Quick Add from Registered Servers */}
      {!loading && servers.length > 0 && (
        <div className="space-y-2">
          <Label>Quick Add from Servers</Label>
          <div className="flex flex-wrap gap-2">
            {servers.map((server) => (
              <Button
                key={server.id}
                variant="outline"
                size="sm"
                onClick={() => addServerTools(server.name)}
                className="gap-1"
              >
                <Wrench className="h-3 w-3" />
                {server.name}
                <Badge variant="secondary" className="text-[10px] ml-1">
                  {server.tool_count}
                </Badge>
              </Button>
            ))}
          </div>
        </div>
      )}

      {loading && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading registered servers...
        </div>
      )}

      <Separator />

      {/* Allowed Tools */}
      <div className="space-y-3">
        <div>
          <Label>Allowed Tools</Label>
          <p className="text-xs text-muted-foreground mt-1">
            Only these tools will be accessible (supports glob patterns)
          </p>
        </div>

        <div className="flex gap-2">
          <Input
            placeholder="e.g., mcp__slack__send_message or mcp__slack__*"
            value={newAllowedPattern}
            onChange={(e) => setNewAllowedPattern(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addAllowedPattern()}
          />
          <Button variant="outline" onClick={addAllowedPattern}>
            <Plus className="h-4 w-4" />
          </Button>
        </div>

        <div className="flex flex-wrap gap-2">
          {(allowedTools || []).length === 0 ? (
            <div className="text-sm text-muted-foreground italic">
              {allowedTools === null ? "All tools allowed (no restriction)" : "No tools allowed"}
            </div>
          ) : (
            allowedTools!.map((pattern, index) => (
              <Badge key={index} variant="default" className="gap-1">
                <code className="text-xs">{pattern}</code>
                <button
                  onClick={() => removeAllowedPattern(index)}
                  className="ml-1 hover:bg-primary-foreground/20 rounded-full p-0.5"
                >
                  <X className="h-3 w-3" />
                </button>
              </Badge>
            ))
          )}
        </div>

        <Button
          variant="ghost"
          size="sm"
          onClick={() => onAllowedChange(null)}
          className="text-xs"
        >
          Reset to allow all
        </Button>
      </div>

      <Separator />

      {/* Blocked Tools */}
      <div className="space-y-3">
        <div>
          <Label>Blocked Tools</Label>
          <p className="text-xs text-muted-foreground mt-1">
            These tools will always be blocked (takes priority over allowlist)
          </p>
        </div>

        <div className="flex gap-2">
          <Input
            placeholder="e.g., mcp__admin__delete_*"
            value={newBlockedPattern}
            onChange={(e) => setNewBlockedPattern(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addBlockedPattern()}
          />
          <Button variant="outline" onClick={addBlockedPattern}>
            <Plus className="h-4 w-4" />
          </Button>
        </div>

        <div className="flex flex-wrap gap-2">
          {(blockedTools || []).length === 0 ? (
            <div className="text-sm text-muted-foreground italic">
              No tools blocked
            </div>
          ) : (
            blockedTools!.map((pattern, index) => (
              <Badge key={index} variant="destructive" className="gap-1">
                <code className="text-xs">{pattern}</code>
                <button
                  onClick={() => removeBlockedPattern(index)}
                  className="ml-1 hover:bg-destructive-foreground/20 rounded-full p-0.5"
                >
                  <X className="h-3 w-3" />
                </button>
              </Badge>
            ))
          )}
        </div>
      </div>
    </div>
  )
}