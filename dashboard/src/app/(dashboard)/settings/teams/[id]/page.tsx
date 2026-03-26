"use client"

import { useEffect, useState } from "react"
import { useParams, useRouter } from "next/navigation"
import { ArrowLeft, Users, Plus, Trash2, DollarSign, TrendingUp, Clock } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
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
  getTeamSpend,
  listTeamMembers,
  addTeamMember,
  removeTeamMember,
  listUsers,
  listTokensWithParams,
  type TeamSpend,
  type TeamMember,
  type User,
  type TokenRow,
} from "@/lib/api"
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  BarChart,
  Bar,
} from "recharts"

// Mock team data - in real app, fetch from API
const mockTeam = {
  id: "team-1",
  name: "Engineering Team",
  description: "Core engineering team tokens",
  max_budget_usd: 1000,
  budget_duration: "monthly",
  allowed_models: ["gpt-4*", "claude-*"],
}

function formatCurrency(num: number): string {
  return "$" + num.toFixed(2)
}

function formatNumber(num: number): string {
  if (num >= 1000000) return (num / 1000000).toFixed(1) + "M"
  if (num >= 1000) return (num / 1000).toFixed(1) + "k"
  return num.toString()
}

function AddMemberModal({
  open,
  onOpenChange,
  users,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  users: User[]
  onSubmit: (userId: string, role: string) => Promise<void>
}) {
  const [selectedUser, setSelectedUser] = useState("")
  const [role, setRole] = useState("member")
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!selectedUser) return
    setIsSubmitting(true)
    try {
      await onSubmit(selectedUser, role)
      onOpenChange(false)
      setSelectedUser("")
      setRole("member")
    } catch (error) {
      // Error handled in parent
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add Team Member</DialogTitle>
          <DialogDescription>Add a user to this team.</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium">User</label>
            <select
              value={selectedUser}
              onChange={(e) => setSelectedUser(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              required
            >
              <option value="">Select a user...</option>
              {users.map((user) => (
                <option key={user.id} value={user.id}>
                  {user.email} {user.name && `(${user.name})`}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="text-sm font-medium">Role</label>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
            >
              <option value="admin">Admin</option>
              <option value="member">Member</option>
              <option value="viewer">Viewer</option>
            </select>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Adding..." : "Add Member"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default function TeamDetailPage() {
  const params = useParams()
  const router = useRouter()
  const teamId = params.id as string

  const [team, setTeam] = useState(mockTeam)
  const [members, setMembers] = useState<TeamMember[]>([])
  const [tokens, setTokens] = useState<TokenRow[]>([])
  const [spend, setSpend] = useState<TeamSpend[]>([])
  const [users, setUsers] = useState<User[]>([])
  const [loading, setLoading] = useState(true)
  const [addMemberOpen, setAddMemberOpen] = useState(false)

  useEffect(() => {
    async function fetchData() {
      try {
        const [membersData, tokensData, spendData, usersData] = await Promise.all([
          listTeamMembers(teamId),
          listTokensWithParams({ team_id: teamId, limit: 100 }),
          getTeamSpend(teamId),
          listUsers(),
        ])
        setMembers(membersData)
        setTokens(tokensData)
        setSpend(spendData)
        setUsers(usersData)
      } catch (err) {
        console.error("Failed to load team data:", err)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [teamId])

  const handleAddMember = async (userId: string, role: string) => {
    try {
      const member = await addTeamMember(teamId, { user_id: userId, role })
      setMembers([...members, member])
      toast.success("Member added successfully")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to add member")
      throw error
    }
  }

  const handleRemoveMember = async (userId: string) => {
    if (!confirm("Remove this member from the team?")) return
    try {
      await removeTeamMember(teamId, userId)
      setMembers(members.filter((m) => m.user_id !== userId))
      toast.success("Member removed")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to remove member")
    }
  }

  // Prepare chart data
  const spendChartData = spend
    .slice()
    .reverse()
    .map((s) => ({
      date: s.period,
      spend: Number(s.total_spend_usd),
      requests: s.total_requests,
      tokens: s.total_tokens_used,
    }))

  const totalSpend = spend.reduce((sum, s) => sum + Number(s.total_spend_usd), 0)
  const totalRequests = spend.reduce((sum, s) => sum + s.total_requests, 0)
  const totalTokens = spend.reduce((sum, s) => sum + s.total_tokens_used, 0)

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-muted-foreground">Loading team data...</div>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-6 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Link href="/settings/teams">
            <Button variant="ghost" size="icon-sm">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">{team.name}</h1>
            {team.description && (
              <p className="text-sm text-muted-foreground">{team.description}</p>
            )}
          </div>
        </div>

        {/* Stats Cards */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-card border rounded-xl p-4 flex items-center gap-4">
            <div className="w-10 h-10 rounded-lg bg-primary/10 flex items-center justify-center">
              <DollarSign className="h-5 w-5 text-primary" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Total Spend</p>
              <p className="text-xl font-semibold">{formatCurrency(totalSpend)}</p>
              {team.max_budget_usd && (
                <p className="text-xs text-muted-foreground">
                  of {formatCurrency(team.max_budget_usd)} budget
                </p>
              )}
            </div>
          </div>
          <div className="bg-card border rounded-xl p-4 flex items-center gap-4">
            <div className="w-10 h-10 rounded-lg bg-blue-500/10 flex items-center justify-center">
              <TrendingUp className="h-5 w-5 text-blue-500" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Total Requests</p>
              <p className="text-xl font-semibold">{formatNumber(totalRequests)}</p>
            </div>
          </div>
          <div className="bg-card border rounded-xl p-4 flex items-center gap-4">
            <div className="w-10 h-10 rounded-lg bg-green-500/10 flex items-center justify-center">
              <Clock className="h-5 w-5 text-green-500" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Tokens Used</p>
              <p className="text-xl font-semibold">{formatNumber(totalTokens)}</p>
            </div>
          </div>
        </div>

        {/* Charts */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Spend Over Time */}
          <div className="bg-card border rounded-xl p-4">
            <h3 className="text-sm font-semibold mb-4">Spend Over Time</h3>
            {spendChartData.length > 0 ? (
              <ResponsiveContainer width="100%" height={200}>
                <LineChart data={spendChartData}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                  <XAxis dataKey="date" className="text-xs" />
                  <YAxis className="text-xs" tickFormatter={(v) => "$" + v} />
                  <Tooltip
                    formatter={(value) => formatCurrency(Number(value))}
                    labelFormatter={(label) => `Date: ${label}`}
                  />
                  <Line
                    type="monotone"
                    dataKey="spend"
                    stroke="hsl(var(--primary))"
                    strokeWidth={2}
                    dot={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-[200px] flex items-center justify-center text-muted-foreground text-sm">
                No spend data yet
              </div>
            )}
          </div>

          {/* Requests Over Time */}
          <div className="bg-card border rounded-xl p-4">
            <h3 className="text-sm font-semibold mb-4">Requests Over Time</h3>
            {spendChartData.length > 0 ? (
              <ResponsiveContainer width="100%" height={200}>
                <BarChart data={spendChartData}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                  <XAxis dataKey="date" className="text-xs" />
                  <YAxis className="text-xs" tickFormatter={(v) => formatNumber(v)} />
                  <Tooltip formatter={(value) => formatNumber(Number(value))} />
                  <Bar dataKey="requests" fill="hsl(var(--primary))" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-[200px] flex items-center justify-center text-muted-foreground text-sm">
                No request data yet
              </div>
            )}
          </div>
        </div>

        {/* Two Column Layout */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Team Members */}
          <div className="bg-card border rounded-xl overflow-hidden">
            <div className="p-4 border-b flex items-center justify-between">
              <h3 className="text-sm font-semibold">Team Members</h3>
              <Button size="sm" className="gap-1" onClick={() => setAddMemberOpen(true)}>
                <Plus className="h-3 w-3" />
                Add
              </Button>
            </div>
            {members.length === 0 ? (
              <div className="p-4 text-center text-muted-foreground text-sm">
                No members yet
              </div>
            ) : (
              <div className="divide-y">
                {members.map((member) => (
                  <div key={member.id} className="p-3 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div className="w-8 h-8 rounded-full bg-muted flex items-center justify-center">
                        <Users className="h-4 w-4 text-muted-foreground" />
                      </div>
                      <div>
                        <p className="text-sm font-medium">{member.user_id}</p>
                        <p className="text-xs text-muted-foreground">{member.role}</p>
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      className="text-muted-foreground hover:text-destructive"
                      onClick={() => handleRemoveMember(member.user_id)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Tokens in Team */}
          <div className="bg-card border rounded-xl overflow-hidden">
            <div className="p-4 border-b flex items-center justify-between">
              <h3 className="text-sm font-semibold">Assigned Tokens</h3>
              <Link href="/tokens">
                <Button size="sm" variant="outline" className="gap-1">
                  <Plus className="h-3 w-3" />
                  Manage
                </Button>
              </Link>
            </div>
            {tokens.length === 0 ? (
              <div className="p-4 text-center text-muted-foreground text-sm">
                No tokens assigned
              </div>
            ) : (
              <div className="divide-y max-h-[300px] overflow-auto">
                {tokens.slice(0, 10).map((token) => (
                  <div key={token.id} className="p-3 flex items-center justify-between">
                    <div>
                      <p className="text-sm font-medium">{token.name}</p>
                      <p className="text-xs text-muted-foreground font-mono">
                        {token.id.slice(0, 20)}...
                      </p>
                    </div>
                    <Badge variant={token.is_active ? "success" : "destructive"} className="text-[10px]">
                      {token.is_active ? "Active" : "Revoked"}
                    </Badge>
                  </div>
                ))}
                {tokens.length > 10 && (
                  <div className="p-2 text-center">
                    <Link href="/tokens" className="text-xs text-primary hover:underline">
                      View all {tokens.length} tokens
                    </Link>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Model Access */}
        {team.allowed_models && team.allowed_models.length > 0 && (
          <div className="bg-card border rounded-xl p-4">
            <h3 className="text-sm font-semibold mb-3">Allowed Models</h3>
            <div className="flex flex-wrap gap-2">
              {team.allowed_models.map((model, i) => (
                <Badge key={i} variant="outline" className="font-mono text-xs">
                  {model}
                </Badge>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Add Member Modal */}
      <AddMemberModal
        open={addMemberOpen}
        onOpenChange={setAddMemberOpen}
        users={users}
        onSubmit={handleAddMember}
      />
    </div>
  )
}