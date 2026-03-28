"use client"

import { useEffect, useState } from "react"
import { useParams, useRouter } from "next/navigation"
import { ArrowLeft, Key, Shield, MoreHorizontal, Trash2, Copy, Check, Plus, Wrench, Save, Loader2, Globe } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { toast } from "sonner"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  getToken,
  getGuardrailStatus,
  listPolicies,
  revokeToken,
  updateTokenMcpTools,
  updateTokenIpRestrictions,
  type TokenRow,
  type GuardrailsStatus,
  type PolicyRow,
} from "@/lib/api"
import { GuardrailPresetDialog } from "@/components/guardrails/guardrail-preset-dialog"
import { ToolPicker } from "@/components/mcp/tool-picker"

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
    <Badge variant={variants[purpose] || "outline"} className="text-xs">
      {labels[purpose] || purpose}
    </Badge>
  )
}

function StatusBadge({ isActive }: { isActive: boolean }) {
  return (
    <Badge variant={isActive ? "success" : "destructive"} className="text-xs">
      {isActive ? "Active" : "Revoked"}
    </Badge>
  )
}

export default function TokenDetailPage() {
  const params = useParams()
  const router = useRouter()
  const tokenId = params.id as string

  const [token, setToken] = useState<TokenRow | null>(null)
  const [guardrailStatus, setGuardrailStatus] = useState<GuardrailsStatus | null>(null)
  const [policies, setPolicies] = useState<PolicyRow[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [guardrailDialogOpen, setGuardrailDialogOpen] = useState(false)
  const [mcpAllowedTools, setMcpAllowedTools] = useState<string[] | null>(null)
  const [mcpBlockedTools, setMcpBlockedTools] = useState<string[] | null>(null)
  const [savingMcp, setSavingMcp] = useState(false)
  const [allowedIps, setAllowedIps] = useState<string[] | null>(null)
  const [blockedIps, setBlockedIps] = useState<string[] | null>(null)
  const [savingIpRestrictions, setSavingIpRestrictions] = useState(false)

  useEffect(() => {
    loadToken()
  }, [tokenId])

  const loadToken = async () => {
    setLoading(true)
    try {
      const [tokenData, guardrails, allPolicies] = await Promise.all([
        getToken(tokenId),
        getGuardrailStatus(tokenId).catch(() => null),
        listPolicies({ limit: 100 }),
      ])
      setToken(tokenData)
      setGuardrailStatus(guardrails)
      // Cast JSON arrays to string[] for local state (backend returns JSON but we expect arrays)
      setMcpAllowedTools(
        Array.isArray(tokenData.mcp_allowed_tools)
          ? tokenData.mcp_allowed_tools as string[]
          : null
      )
      setMcpBlockedTools(
        Array.isArray(tokenData.mcp_blocked_tools)
          ? tokenData.mcp_blocked_tools as string[]
          : null
      )
      setAllowedIps(
        Array.isArray(tokenData.allowed_ips)
          ? tokenData.allowed_ips as string[]
          : null
      )
      setBlockedIps(
        Array.isArray(tokenData.blocked_ips)
          ? tokenData.blocked_ips as string[]
          : null
      )

      // Filter policies attached to this token
      const tokenPolicyIds = new Set(tokenData.policy_ids || [])
      setPolicies(allPolicies.filter((p) => tokenPolicyIds.has(p.id)))
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load token")
    } finally {
      setLoading(false)
    }
  }

  const handleRevoke = async () => {
    if (!confirm("Are you sure you want to revoke this token? This action cannot be undone.")) return

    try {
      await revokeToken(tokenId)
      router.push("/tokens")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to revoke token")
    }
  }

  const handleCopyTokenId = () => {
    if (token) {
      navigator.clipboard.writeText(token.id)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  const handleSaveMcpTools = async () => {
    setSavingMcp(true)
    try {
      await updateTokenMcpTools(tokenId, {
        mcp_allowed_tools: mcpAllowedTools,
        mcp_blocked_tools: mcpBlockedTools,
      })
      // Update the local token state to reflect the changes
      if (token) {
        setToken({
          ...token,
          mcp_allowed_tools: mcpAllowedTools,
          mcp_blocked_tools: mcpBlockedTools,
        })
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save MCP tools")
    } finally {
      setSavingMcp(false)
    }
  }

  const handleSaveIpRestrictions = async () => {
    setSavingIpRestrictions(true)
    try {
      await updateTokenIpRestrictions(tokenId, {
        allowed_ips: allowedIps,
        blocked_ips: blockedIps,
      })
      // Update the local token state to reflect the changes
      if (token) {
        setToken({
          ...token,
          allowed_ips: allowedIps,
          blocked_ips: blockedIps,
        })
      }
      toast.success("IP restrictions saved")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save IP restrictions")
    } finally {
      setSavingIpRestrictions(false)
    }
  }

  if (loading) {
    return (
      <div className="flex-1 p-8 flex items-center justify-center">
        <div className="text-muted-foreground">Loading token...</div>
      </div>
    )
  }

  if (error || !token) {
    return (
      <div className="flex-1 p-8">
        <div className="text-destructive">{error || "Token not found"}</div>
        <Link href="/tokens">
          <Button variant="outline" className="mt-4">
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back to Tokens
          </Button>
        </Link>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link href="/tokens">
              <Button variant="ghost" size="icon-sm">
                <ArrowLeft className="h-4 w-4" />
              </Button>
            </Link>
            <div className="flex flex-col gap-1">
              <div className="flex items-center gap-2">
                <Key className="h-5 w-5 text-muted-foreground" />
                <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
                  {token.name}
                </h1>
                <StatusBadge isActive={token.is_active} />
                <PurposeBadge purpose={token.purpose} />
              </div>
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <code className="px-2 py-0.5 bg-muted rounded font-mono text-xs">
                  {token.id.slice(0, 24)}...
                </code>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={handleCopyTokenId}
                >
                  {copied ? (
                    <Check className="h-3 w-3 text-green-500" />
                  ) : (
                    <Copy className="h-3 w-3" />
                  )}
                </Button>
              </div>
            </div>
          </div>

          <DropdownMenu>
            <DropdownMenuTrigger>
              <Button variant="ghost" size="icon-sm">
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                className="text-destructive"
                onClick={handleRevoke}
                disabled={!token.is_active}
              >
                <Trash2 className="h-4 w-4 mr-2" />
                Revoke Token
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {/* Token Details */}
        <div className="grid gap-4 md:grid-cols-3">
          <div className="p-4 border rounded-lg">
            <p className="text-sm text-muted-foreground">External User</p>
            <p className="text-lg font-medium">{token.external_user_id || "—"}</p>
          </div>
          <div className="p-4 border rounded-lg">
            <p className="text-sm text-muted-foreground">Spend Cap</p>
            <p className="text-lg font-medium">
              {token.spend_cap_usd ? `$${token.spend_cap_usd.toFixed(2)}` : "Unlimited"}
            </p>
          </div>
          <div className="p-4 border rounded-lg">
            <p className="text-sm text-muted-foreground">Spend Used</p>
            <p className="text-lg font-medium">
              {token.spend_used_usd ? `$${token.spend_used_usd.toFixed(2)}` : "$0.00"}
            </p>
          </div>
        </div>

        {/* Tabs */}
        <Tabs defaultValue="guardrails" className="flex-1">
          <TabsList>
            <TabsTrigger value="guardrails" className="gap-2">
              <Shield className="h-4 w-4" />
              Guardrails
            </TabsTrigger>
            <TabsTrigger value="mcp-tools" className="gap-2">
              <Wrench className="h-4 w-4" />
              MCP Tools
            </TabsTrigger>
            <TabsTrigger value="ip-restrictions" className="gap-2">
              <Globe className="h-4 w-4" />
              IP Restrictions
            </TabsTrigger>
            <TabsTrigger value="policies">Policies ({policies.length})</TabsTrigger>
          </TabsList>

          <TabsContent value="guardrails" className="mt-4 space-y-4">
            {/* Guardrails Quick Actions */}
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-lg font-medium">Active Guardrails</h3>
                <p className="text-sm text-muted-foreground">
                  Pre-configured content safety and PII protection rules
                </p>
              </div>
              <Button onClick={() => setGuardrailDialogOpen(true)} className="gap-2">
                <Plus className="h-4 w-4" />
                Enable Guardrails
              </Button>
            </div>

            {/* Guardrail Status */}
            {guardrailStatus?.has_guardrails ? (
              <div className="bg-card border rounded-xl overflow-hidden">
                <table className="w-full">
                  <thead className="bg-muted/50 border-b">
                    <tr className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                      <th className="px-4 py-3 text-left">Source</th>
                      <th className="px-4 py-3 text-left">Policy</th>
                      <th className="px-4 py-3 text-left">Presets</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr className="border-b last:border-0">
                      <td className="px-4 py-3">
                        <Badge variant="outline" className="text-xs">
                          {guardrailStatus.source || "sdk"}
                        </Badge>
                      </td>
                      <td className="px-4 py-3">
                        {guardrailStatus.policy_name ? (
                          <Link
                            href={`/policies/${guardrailStatus.policy_id}`}
                            className="text-sm font-medium hover:underline"
                          >
                            {guardrailStatus.policy_name}
                          </Link>
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-wrap gap-1">
                          {guardrailStatus.presets.map((preset) => (
                            <Badge key={preset} variant="secondary" className="text-xs">
                              {preset.replace(/_/g, " ")}
                            </Badge>
                          ))}
                        </div>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="bg-card border rounded-xl p-8 text-center">
                <Shield className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
                <p className="text-muted-foreground">No guardrails enabled</p>
                <p className="text-sm text-muted-foreground/70 mt-1">
                  Enable preset guardrails for content safety and PII protection
                </p>
                <Button
                  className="mt-4 gap-2"
                  onClick={() => setGuardrailDialogOpen(true)}
                >
                  <Plus className="h-4 w-4" />
                  Enable Guardrails
                </Button>
              </div>
            )}
          </TabsContent>

          <TabsContent value="mcp-tools" className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-lg font-medium">MCP Tool Access</h3>
                <p className="text-sm text-muted-foreground">
                  Configure which MCP tools this token can access
                </p>
              </div>
              <Button className="gap-2" onClick={handleSaveMcpTools} disabled={savingMcp}>
                {savingMcp ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                {savingMcp ? "Saving..." : "Save Changes"}
              </Button>
            </div>

            <div className="bg-card border rounded-xl p-6">
              <ToolPicker
                allowedTools={mcpAllowedTools}
                blockedTools={mcpBlockedTools}
                onAllowedChange={setMcpAllowedTools}
                onBlockedChange={setMcpBlockedTools}
              />
            </div>
          </TabsContent>

          <TabsContent value="ip-restrictions" className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-lg font-medium">IP Restrictions</h3>
                <p className="text-sm text-muted-foreground">
                  Control which IP addresses can use this token
                </p>
              </div>
              <Button className="gap-2" onClick={handleSaveIpRestrictions} disabled={savingIpRestrictions}>
                {savingIpRestrictions ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                {savingIpRestrictions ? "Saving..." : "Save Changes"}
              </Button>
            </div>

            <div className="bg-card border rounded-xl p-6 space-y-6">
              {/* Allowed IPs */}
              <div className="space-y-2">
                <label className="text-sm font-medium">Allowed IPs</label>
                <p className="text-xs text-muted-foreground">
                  Only allow requests from these IP addresses. Leave empty to allow all IPs.
                </p>
                <textarea
                  className="w-full min-h-[100px] p-3 border rounded-lg text-sm font-mono resize-y"
                  placeholder="192.168.0.0/16&#10;10.0.0.1&#10;203.0.113.0/24"
                  value={allowedIps?.join("\n") || ""}
                  onChange={(e) => {
                    const lines = e.target.value.split("\n").map((l) => l.trim()).filter(Boolean)
                    setAllowedIps(lines.length > 0 ? lines : null)
                  }}
                />
              </div>

              {/* Blocked IPs */}
              <div className="space-y-2">
                <label className="text-sm font-medium">Blocked IPs</label>
                <p className="text-xs text-muted-foreground">
                  Block requests from these IP addresses. Takes precedence over allowed IPs.
                </p>
                <textarea
                  className="w-full min-h-[100px] p-3 border rounded-lg text-sm font-mono resize-y"
                  placeholder="192.168.1.100&#10;10.0.0.50"
                  value={blockedIps?.join("\n") || ""}
                  onChange={(e) => {
                    const lines = e.target.value.split("\n").map((l) => l.trim()).filter(Boolean)
                    setBlockedIps(lines.length > 0 ? lines : null)
                  }}
                />
              </div>

              {/* Helper text */}
              <div className="text-xs text-muted-foreground bg-muted/50 p-3 rounded-lg">
                <p className="font-medium mb-1">IP Format Examples:</p>
                <ul className="list-disc list-inside space-y-0.5">
                  <li>Single IP: <code className="font-mono">192.168.1.1</code></li>
                  <li>CIDR range: <code className="font-mono">192.168.0.0/16</code> (matches 192.168.*.*)</li>
                  <li>IPv6: <code className="font-mono">2001:db8::/32</code></li>
                </ul>
              </div>
            </div>
          </TabsContent>

          <TabsContent value="policies" className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-lg font-medium">Attached Policies</h3>
                <p className="text-sm text-muted-foreground">
                  Policies that are attached to this token
                </p>
              </div>
              <Link href="/policies/new">
                <Button variant="outline" className="gap-2">
                  <Plus className="h-4 w-4" />
                  Create Policy
                </Button>
              </Link>
            </div>

            {policies.length > 0 ? (
              <div className="bg-card border rounded-xl overflow-hidden">
                <table className="w-full">
                  <thead className="bg-muted/50 border-b">
                    <tr className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                      <th className="px-4 py-3 text-left">Name</th>
                      <th className="px-4 py-3 text-left">Phase</th>
                      <th className="px-4 py-3 text-left">Mode</th>
                      <th className="px-4 py-3 text-left">Rules</th>
                    </tr>
                  </thead>
                  <tbody>
                    {policies.map((policy) => (
                      <tr key={policy.id} className="border-b last:border-0 hover:bg-muted/30">
                        <td className="px-4 py-3">
                          <Link
                            href={`/policies/${policy.id}`}
                            className="text-sm font-medium hover:underline"
                          >
                            {policy.name}
                          </Link>
                        </td>
                        <td className="px-4 py-3">
                          <Badge variant="outline" className="text-xs">
                            {policy.phase}
                          </Badge>
                        </td>
                        <td className="px-4 py-3">
                          <Badge variant={policy.mode === "enforce" ? "default" : "secondary"} className="text-xs">
                            {policy.mode}
                          </Badge>
                        </td>
                        <td className="px-4 py-3 text-sm text-muted-foreground">
                          {policy.rules.length} rules
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="bg-card border rounded-xl p-8 text-center">
                <p className="text-muted-foreground">No policies attached</p>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </div>

      {/* Guardrail Dialog */}
      <GuardrailPresetDialog
        open={guardrailDialogOpen}
        onOpenChange={setGuardrailDialogOpen}
        initialTokenId={tokenId}
        onSuccess={loadToken}
      />
    </div>
  )
}