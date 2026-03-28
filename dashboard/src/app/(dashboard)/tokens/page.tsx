"use client"

import { useEffect, useState, useCallback } from "react"
import { useRouter } from "next/navigation"
import { Plus, Key, MoreHorizontal, Trash2, Eye, Users, Copy, Check, ChevronDown, GripVertical, ArrowUpDown } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { toast } from "sonner"
import {
  listTokensWithParams,
  revokeToken,
  createToken,
  listTeams,
  listCredentials,
  type TokenRow,
  type Team,
  type CredentialMeta,
} from "@/lib/api"
import { PROVIDER_PRESETS } from "@/lib/provider-presets"
import { TokenModeSelector, type TokenMode } from "@/components/tokens/token-mode-selector"
import { ByokBadge } from "@/components/tokens/byok-badge"

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

// Token Creation Modal
function CreateTokenModal({
  open,
  onOpenChange,
  teams,
  credentials,
  onSuccess,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  teams: Team[]
  credentials: CredentialMeta[]
  onSuccess: () => void
}) {
  const [tokenMode, setTokenMode] = useState<TokenMode>("managed")
  const [name, setName] = useState("")
  const [teamId, setTeamId] = useState("")
  const [selectedProvider, setSelectedProvider] = useState("OpenAI")
  const [upstreamUrl, setUpstreamUrl] = useState("https://api.openai.com/v1")
  const [externalUserId, setExternalUserId] = useState("")
  const [purpose, setPurpose] = useState<"llm" | "tool" | "both">("llm")
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [createdToken, setCreatedToken] = useState<string | null>(null)

  // Provider-level access control
  const [selectedProviders, setSelectedProviders] = useState<string[]>(["OpenAI"])

  // Per-provider credential mapping
  const [providerCredentials, setProviderCredentials] = useState<Record<string, string>>({})

  // Provider name to ID mapping for credential filtering
  const providerIdMap: Record<string, string> = {
    "OpenAI": "openai",
    "Anthropic": "anthropic",
    "Google Gemini": "gemini",
    "Azure OpenAI": "azure",
    "AWS Bedrock": "bedrock",
    "Cohere": "cohere",
    "Mistral": "mistral",
    "Groq": "groq",
    "Together AI": "together",
    "Ollama": "ollama",
    "OpenRouter": "openrouter",
    "Custom": "custom",
  }

  // Get the currently selected preset
  const currentPreset = PROVIDER_PRESETS.find(p => p.name === selectedProvider) || PROVIDER_PRESETS[0]

  // Toggle provider selection
  const toggleProvider = (providerName: string) => {
    setSelectedProviders(prev => {
      if (prev.includes(providerName)) {
        // Don't allow deselecting all providers
        if (prev.length === 1) return prev
        return prev.filter(p => p !== providerName)
      } else {
        return [...prev, providerName]
      }
    })
  }

  // Sync credential mappings when providers are removed
  useEffect(() => {
    setProviderCredentials(prev => {
      const newCreds = { ...prev }
      Object.keys(newCreds).forEach(provider => {
        if (!selectedProviders.includes(provider)) {
          delete newCreds[provider]
        }
      })
      return newCreds
    })
  }, [selectedProviders])

  // Set single provider (for passthrough mode)
  const setSingleProvider = (providerName: string) => {
    setSelectedProviders([providerName])
    const preset = PROVIDER_PRESETS.find(p => p.name === providerName)
    if (preset?.url) {
      setUpstreamUrl(preset.url)
    }
  }

  // Move provider up in priority
  const moveProviderUp = (index: number) => {
    if (index === 0) return
    const newProviders = [...selectedProviders]
    ;[newProviders[index - 1], newProviders[index]] = [newProviders[index], newProviders[index - 1]]
    setSelectedProviders(newProviders)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      // For managed mode with multiple providers, build upstreams array
      if (tokenMode === "managed" && selectedProviders.length > 0) {
        // Validate that all providers have credentials
        const missingCredentials = selectedProviders.filter(p => !providerCredentials[p])
        if (missingCredentials.length > 0) {
          toast.error(`Please select credentials for: ${missingCredentials.join(", ")}`)
          setIsSubmitting(false)
          return
        }

        // Build upstreams array with per-provider credentials
        const upstreams = selectedProviders.map((provider, index) => {
          const preset = PROVIDER_PRESETS.find(p => p.name === provider)
          return {
            url: preset?.url || upstreamUrl,
            weight: 100,
            priority: index + 1,
            credential_id: providerCredentials[provider],
            allowed_models: preset?.allowed_models || undefined,
          }
        })

        const response = await createToken({
          name,
          team_id: teamId || undefined,
          upstream_url: upstreams[0].url, // Primary upstream URL
          upstreams,
          external_user_id: externalUserId || undefined,
          purpose,
          allowed_providers: selectedProviders.map(p => p.toLowerCase()),
        })
        setCreatedToken(response.token_id)
        toast.success("Token created successfully")
      } else {
        // Passthrough mode - single provider, no stored credential
        const response = await createToken({
          name,
          team_id: teamId || undefined,
          credential_id: null,
          upstream_url: upstreamUrl,
          external_user_id: externalUserId || undefined,
          purpose,
          allowed_providers: selectedProviders.map(p => p.toLowerCase()),
        })
        setCreatedToken(response.token_id)
        toast.success("Token created successfully")
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create token")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleClose = () => {
    setTokenMode("managed")
    setName("")
    setTeamId("")
    setSelectedProvider("OpenAI")
    setUpstreamUrl("https://api.openai.com/v1")
    setExternalUserId("")
    setPurpose("llm")
    setSelectedProviders(["OpenAI"])
    setProviderCredentials({})
    setCreatedToken(null)
    onOpenChange(false)
    if (createdToken) {
      onSuccess()
    }
  }

  if (createdToken) {
    return (
      <Dialog open={open} onOpenChange={handleClose}>
        <DialogContent className="sm:max-w-md" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle className="text-green-600">Token Created</DialogTitle>
            <DialogDescription>
              Copy your token ID now. It will be shown in the list.
            </DialogDescription>
          </DialogHeader>
          <div className="bg-muted rounded-lg p-3 font-mono text-sm break-all">
            <code className="text-xs">{createdToken}</code>
          </div>
          <DialogFooter>
            <Button
              onClick={handleClose}
            >
              Done
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    )
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Create Token</DialogTitle>
          <DialogDescription>
            Create a new virtual API key for your gateway.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Token Mode Selector */}
          <TokenModeSelector value={tokenMode} onChange={setTokenMode} />

          <div className="grid grid-cols-2 gap-3">
            <div className="col-span-2">
              <label className="text-sm font-medium">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                placeholder="My Token"
                required
              />
            </div>

            {/* Provider Selection */}
            <div className="col-span-2">
              {tokenMode === "passthrough" ? (
                // Single provider selection for passthrough mode
                <>
                  <label className="text-sm font-medium">Provider</label>
                  <p className="text-xs text-muted-foreground mb-2">
                    Select the provider for this BYOK token. Your API key will be sent to this provider.
                  </p>
                  <select
                    value={selectedProviders[0] || ""}
                    onChange={(e) => setSingleProvider(e.target.value)}
                    className="w-full mt-2 px-3 py-2 text-sm border rounded-lg bg-background"
                    required
                  >
                    <option value="">Select provider...</option>
                    {PROVIDER_PRESETS.map((preset) => (
                      <option key={preset.name} value={preset.name}>
                        {preset.name}
                      </option>
                    ))}
                  </select>
                </>
              ) : (
                // Multi-provider selection for managed mode
                <>
                  <label className="text-sm font-medium">Allowed Providers</label>
                  <p className="text-xs text-muted-foreground mb-2">
                    Select providers and assign credentials. Each provider can use a different API key.
                  </p>
                  <div className="space-y-2 mt-2">
                    {/* Selected providers with priority and credentials */}
                    {selectedProviders.length > 0 && (
                      <div className="space-y-2 mb-3">
                        {selectedProviders.map((providerName, index) => {
                          const preset = PROVIDER_PRESETS.find(p => p.name === providerName) || PROVIDER_PRESETS[0]
                          const priorityLabel = index === 0 ? "Primary" : `Backup ${index}`
                          // Filter credentials by provider using ID mapping
                          const providerId = providerIdMap[providerName] || providerName.toLowerCase()
                          const providerCredentialsList = credentials.filter(
                            c => c.provider === providerId
                          )
                          return (
                            <div
                              key={providerName}
                              className="p-3 border rounded-lg bg-muted/30 space-y-2"
                            >
                              <div className="flex items-center gap-2">
                                <GripVertical className="h-4 w-4 text-muted-foreground cursor-grab" />
                                <Badge
                                  variant={index === 0 ? "default" : "secondary"}
                                  className="text-[10px] min-w-[60px] justify-center"
                                >
                                  {priorityLabel}
                                </Badge>
                                <span className="text-sm font-medium flex-1">{providerName}</span>
                                <div className="flex items-center gap-1">
                                  <Button
                                    type="button"
                                    variant="ghost"
                                    size="icon-sm"
                                    disabled={index === 0}
                                    onClick={() => moveProviderUp(index)}
                                    className="h-6 w-6"
                                  >
                                    <ArrowUpDown className="h-3 w-3" />
                                  </Button>
                                  <Button
                                    type="button"
                                    variant="ghost"
                                    size="icon-sm"
                                    onClick={() => toggleProvider(providerName)}
                                    className="h-6 w-6 text-muted-foreground hover:text-destructive"
                                  >
                                    <Trash2 className="h-3 w-3" />
                                  </Button>
                                </div>
                              </div>
                              {/* Credential dropdown for this provider */}
                              <div className="pl-6">
                                <select
                                  value={providerCredentials[providerName] || ""}
                                  onChange={(e) => {
                                    setProviderCredentials(prev => ({
                                      ...prev,
                                      [providerName]: e.target.value
                                    }))
                                  }}
                                  className="w-full px-2 py-1.5 text-xs border rounded bg-background"
                                  required
                                >
                                  <option value="">Select credential...</option>
                                  {providerCredentialsList.map((cred) => (
                                    <option key={cred.id} value={cred.id}>
                                      {cred.name}
                                    </option>
                                  ))}
                                </select>
                                {providerCredentialsList.length === 0 && (
                                  <p className="text-[10px] text-amber-600 mt-1">
                                    No {providerName} credentials found. Create one first.
                                  </p>
                                )}
                              </div>
                            </div>
                          )
                        })}
                      </div>
                    )}

                    {/* Provider dropdown to add more */}
                    <select
                      value=""
                      onChange={(e) => {
                        if (e.target.value && !selectedProviders.includes(e.target.value)) {
                          setSelectedProviders([...selectedProviders, e.target.value])
                        }
                        e.target.value = ""
                      }}
                      className="w-full px-3 py-2 text-sm border rounded-lg bg-background"
                    >
                      <option value="">+ Add provider...</option>
                      {PROVIDER_PRESETS.filter(p => !selectedProviders.includes(p.name)).map((preset) => (
                        <option key={preset.name} value={preset.name}>
                          {preset.name}
                        </option>
                      ))}
                    </select>
                  </div>
                </>
              )}
            </div>

            {/* Models for each selected provider */}
            {selectedProviders.length > 0 && tokenMode === "managed" && (
              <div className="col-span-2">
                <label className="text-sm font-medium text-muted-foreground">Available Models by Provider</label>
                <div className="mt-2 space-y-2">
                  {selectedProviders.map((providerName) => {
                    const preset = PROVIDER_PRESETS.find(p => p.name === providerName) || PROVIDER_PRESETS[0]
                    return (
                      <div key={providerName} className="p-2 border rounded-lg bg-muted/20">
                        <span className="text-xs font-medium">{providerName}</span>
                        <div className="flex flex-wrap gap-1 mt-1">
                          {preset.allowed_models.slice(0, 5).map((pattern) => (
                            <Badge key={pattern} variant="outline" className="text-[10px] font-mono">
                              {pattern}
                            </Badge>
                          ))}
                          {preset.allowed_models.length > 5 && (
                            <Badge variant="outline" className="text-[10px]">
                              +{preset.allowed_models.length - 5} more
                            </Badge>
                          )}
                        </div>
                      </div>
                    )
                  })}
                </div>
              </div>
            )}

            {/* Models for passthrough mode (single provider) */}
            {selectedProviders.length > 0 && tokenMode === "passthrough" && (
              <div className="col-span-2">
                <label className="text-sm font-medium text-muted-foreground">Available Models</label>
                <div className="mt-2 p-2 border rounded-lg bg-muted/20">
                  <div className="flex flex-wrap gap-1">
                    {(PROVIDER_PRESETS.find(p => p.name === selectedProviders[0])?.allowed_models || []).slice(0, 8).map((pattern) => (
                      <Badge key={pattern} variant="outline" className="text-[10px] font-mono">
                        {pattern}
                      </Badge>
                    ))}
                  </div>
                </div>
              </div>
            )}

            <div>
              <label className="text-sm font-medium">Team</label>
              <select
                value={teamId}
                onChange={(e) => setTeamId(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              >
                <option value="">No team</option>
                {teams.map((team) => (
                  <option key={team.id} value={team.id}>
                    {team.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-sm font-medium">Purpose</label>
              <select
                value={purpose}
                onChange={(e) => setPurpose(e.target.value as "llm" | "tool" | "both")}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              >
                <option value="llm">LLM</option>
                <option value="tool">Tool</option>
                <option value="both">Both</option>
              </select>
            </div>
            <div>
              <label className="text-sm font-medium">External User ID</label>
              <input
                type="text"
                value={externalUserId}
                onChange={(e) => setExternalUserId(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                placeholder="user_123"
              />
            </div>

            {/* Custom URL for providers like Azure/Bedrock/Custom */}
            {(!currentPreset.url || selectedProvider === "Custom") && (
              <div className="col-span-2">
                <label className="text-sm font-medium">Upstream URL</label>
                <input
                  type="url"
                  value={upstreamUrl}
                  onChange={(e) => setUpstreamUrl(e.target.value)}
                  className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                  placeholder="https://api.provider.com/v1"
                  required
                />
              </div>
            )}
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={handleClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Creating..." : "Create Token"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default function TokensPage() {
  const router = useRouter()
  const [tokens, setTokens] = useState<TokenRow[]>([])
  const [teams, setTeams] = useState<Team[]>([])
  const [credentials, setCredentials] = useState<CredentialMeta[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [createModalOpen, setCreateModalOpen] = useState(false)

  const fetchData = async () => {
    try {
      const [tokensData, teamsData, credsData] = await Promise.all([
        listTokensWithParams({ limit: 100 }),
        listTeams(),
        listCredentials(),
      ])
      setTokens(tokensData)
      setTeams(teamsData)
      setCredentials(credsData)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load tokens")
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchData()
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

  // Create a map of team IDs to team names
  const teamMap = new Map(teams.map((t) => [t.id, t.name]))

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
          <Button className="gap-2" onClick={() => setCreateModalOpen(true)}>
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
                  <th className="px-4 py-3 text-left">Team</th>
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
                    className="border-b last:border-0 hover:bg-muted/30 transition-colors cursor-pointer"
                    onClick={() => router.push(`/tokens/${token.id}`)}
                  >
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium">{token.name}</span>
                        {!token.credential_id && <ByokBadge />}
                      </div>
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
                      {token.team_id ? (
                        <Link
                          href={`/settings/teams/${token.team_id}`}
                          className="text-sm text-primary hover:underline flex items-center gap-1"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <Users className="h-3 w-3" />
                          {teamMap.get(token.team_id) || token.team_id.slice(0, 8)}
                        </Link>
                      ) : (
                        <span className="text-sm text-muted-foreground">—</span>
                      )}
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
                        <DropdownMenuTrigger onClick={(e) => e.stopPropagation()}>
                          <Button variant="ghost" size="icon-sm">
                            <MoreHorizontal className="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onClick={() => router.push(`/tokens/${token.id}`)}>
                            <Eye className="h-4 w-4 mr-2" />
                            View Details
                          </DropdownMenuItem>
                          {token.is_active && (
                            <DropdownMenuItem
                              className="text-destructive"
                              onClick={(e) => {
                                e.stopPropagation()
                                handleRevoke(token.id)
                              }}
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

      {/* Create Token Modal */}
      <CreateTokenModal
        open={createModalOpen}
        onOpenChange={setCreateModalOpen}
        teams={teams}
        credentials={credentials}
        onSuccess={fetchData}
      />
    </div>
  )
}