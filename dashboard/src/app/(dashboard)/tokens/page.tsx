"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { Plus, Key, MoreHorizontal, Trash2, Eye, Users, Copy, Check } from "lucide-react"
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
  const [name, setName] = useState("")
  const [teamId, setTeamId] = useState("")
  const [credentialId, setCredentialId] = useState("")
  const [upstreamUrl, setUpstreamUrl] = useState("https://api.openai.com/v1")
  const [externalUserId, setExternalUserId] = useState("")
  const [purpose, setPurpose] = useState<"llm" | "tool" | "both">("llm")
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [createdToken, setCreatedToken] = useState<string | null>(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      const response = await createToken({
        name,
        team_id: teamId || undefined,
        credential_id: credentialId || undefined,
        upstream_url: upstreamUrl,
        external_user_id: externalUserId || undefined,
        purpose,
      })
      setCreatedToken(response.token_id)
      toast.success("Token created successfully")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create token")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleClose = () => {
    setName("")
    setTeamId("")
    setCredentialId("")
    setUpstreamUrl("https://api.openai.com/v1")
    setExternalUserId("")
    setPurpose("llm")
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
              <label className="text-sm font-medium">Credential</label>
              <select
                value={credentialId}
                onChange={(e) => setCredentialId(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              >
                <option value="">Default</option>
                {credentials.map((cred) => (
                  <option key={cred.id} value={cred.id}>
                    {cred.name} ({cred.provider})
                  </option>
                ))}
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
            <div>
              <label className="text-sm font-medium">Upstream URL</label>
              <input
                type="url"
                value={upstreamUrl}
                onChange={(e) => setUpstreamUrl(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                placeholder="https://api.openai.com/v1"
              />
            </div>
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