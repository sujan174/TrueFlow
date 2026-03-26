"use client"

import { useEffect, useState } from "react"
import { Plus, MoreHorizontal, Users, Trash2, Edit, BarChart3 } from "lucide-react"
import { Button } from "@/components/ui/button"
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
  listTeams,
  createTeam,
  updateTeam,
  deleteTeam,
  getTeamSpend,
  type Team,
  type CreateTeamRequest,
  type TeamSpend,
} from "@/lib/api"
import { cn } from "@/lib/utils"
import { SettingsSidebar } from "../_components/settings-sidebar"
import { useRouter } from "next/navigation"
import { usePermissions } from "@/contexts/permissions-context"

function formatCurrency(num: number): string {
  return "$" + num.toFixed(2)
}

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffDays < 1) return "today"
  if (diffDays < 7) return `${diffDays}d ago`
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`
  return date.toLocaleDateString()
}

function BudgetProgress({ budget, spent }: { budget: number | null; spent: number }) {
  if (!budget) {
    return <span className="text-xs text-muted-foreground">No limit</span>
  }

  const percentage = Math.min((spent / budget) * 100, 100)
  const isOver = spent > budget

  return (
    <div className="flex items-center gap-3">
      <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden min-w-[80px]">
        <div
          className={cn(
            "h-full transition-all rounded-full",
            isOver ? "bg-destructive" : percentage > 80 ? "bg-warning" : "bg-primary"
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <span className={cn(
        "text-xs font-mono tabular-nums",
        isOver ? "text-destructive" : "text-muted-foreground"
      )}>
        {formatCurrency(spent)} / {formatCurrency(budget)}
      </span>
    </div>
  )
}

function BudgetDurationBadge({ duration }: { duration: string | null }) {
  if (!duration) return null

  const labels: Record<string, string> = {
    daily: "Daily",
    weekly: "Weekly",
    monthly: "Monthly",
    yearly: "Yearly",
  }

  return (
    <span className="text-xs text-muted-foreground">
      {labels[duration] || duration}
    </span>
  )
}

function TeamFormModal({
  open,
  onOpenChange,
  team,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  team?: Team
  onSubmit: (data: CreateTeamRequest) => Promise<void>
}) {
  const [name, setName] = useState(team?.name || "")
  const [description, setDescription] = useState(team?.description || "")
  const [maxBudget, setMaxBudget] = useState(team?.max_budget_usd?.toString() || "")
  const [budgetDuration, setBudgetDuration] = useState<string>(team?.budget_duration || "monthly")
  const [allowedModels, setAllowedModels] = useState(team?.allowed_models?.join(", ") || "")
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (team) {
      setName(team.name)
      setDescription(team.description || "")
      setMaxBudget(team.max_budget_usd?.toString() || "")
      setBudgetDuration(team.budget_duration || "monthly")
      setAllowedModels(team.allowed_models?.join(", ") || "")
    } else {
      setName("")
      setDescription("")
      setMaxBudget("")
      setBudgetDuration("monthly")
      setAllowedModels("")
    }
  }, [team, open])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      await onSubmit({
        name,
        description: description || undefined,
        max_budget_usd: maxBudget ? parseFloat(maxBudget) : undefined,
        budget_duration: budgetDuration as "daily" | "weekly" | "monthly" | "yearly",
        allowed_models: allowedModels
          ? allowedModels.split(",").map((m) => m.trim()).filter(Boolean)
          : undefined,
      })
      onOpenChange(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to save team")
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{team ? "Edit Team" : "Create Team"}</DialogTitle>
          <DialogDescription>
            {team
              ? "Update team settings and budget limits."
              : "Create a new team to group tokens and track spending."}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium">Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="Engineering Team"
              required
            />
          </div>
          <div>
            <label className="text-sm font-medium">Description</label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="Optional description"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-sm font-medium">Budget Limit ($)</label>
              <input
                type="number"
                value={maxBudget}
                onChange={(e) => setMaxBudget(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                placeholder="1000"
                min="0"
                step="0.01"
              />
            </div>
            <div>
              <label className="text-sm font-medium">Budget Period</label>
              <select
                value={budgetDuration}
                onChange={(e) => setBudgetDuration(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              >
                <option value="daily">Daily</option>
                <option value="weekly">Weekly</option>
                <option value="monthly">Monthly</option>
                <option value="yearly">Yearly</option>
              </select>
            </div>
          </div>
          <div>
            <label className="text-sm font-medium">Allowed Models</label>
            <input
              type="text"
              value={allowedModels}
              onChange={(e) => setAllowedModels(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="gpt-4*, claude-* (comma-separated, glob patterns)"
            />
            <p className="text-xs text-muted-foreground mt-1">
              Use glob patterns like gpt-4* or claude-* for wildcards
            </p>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Saving..." : team ? "Update" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default function TeamsPage() {
  const router = useRouter()
  const { isAdmin } = usePermissions()
  const [teams, setTeams] = useState<Team[]>([])
  const [teamSpends, setTeamSpends] = useState<Record<string, number>>({})
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [modalOpen, setModalOpen] = useState(false)
  const [editingTeam, setEditingTeam] = useState<Team | undefined>()

  useEffect(() => {
    async function fetchTeams() {
      try {
        const data = await listTeams()
        setTeams(data)

        // Fetch current period spend for each team
        const spendPromises = data.map(async (team) => {
          try {
            const spends = await getTeamSpend(team.id)
            // Get the most recent period's spend (first item, sorted by date DESC)
            const currentSpend = spends.length > 0 ? spends[0].total_spend_usd : 0
            return { teamId: team.id, spend: currentSpend }
          } catch {
            return { teamId: team.id, spend: 0 }
          }
        })

        const spendResults = await Promise.all(spendPromises)
        const spendMap: Record<string, number> = {}
        spendResults.forEach(({ teamId, spend }) => {
          spendMap[teamId] = spend
        })
        setTeamSpends(spendMap)
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load teams")
      } finally {
        setLoading(false)
      }
    }
    fetchTeams()
  }, [])

  const handleCreate = async (data: CreateTeamRequest) => {
    const team = await createTeam(data)
    setTeams([...teams, team])
    toast.success("Team created successfully")
  }

  const handleUpdate = async (data: CreateTeamRequest) => {
    if (!editingTeam) return
    const updated = await updateTeam(editingTeam.id, data)
    setTeams(teams.map((t) => (t.id === editingTeam.id ? updated : t)))
    setEditingTeam(undefined)
    toast.success("Team updated successfully")
  }

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this team?")) return
    try {
      await deleteTeam(id)
      setTeams(teams.filter((t) => t.id !== id))
      toast.success("Team deleted")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete team")
    }
  }

  return (
    <div className="flex-1 flex min-w-0">
      <SettingsSidebar />

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-w-0 overflow-auto">
        <div className="flex-1 p-6 lg:p-8">
          {/* Header */}
          <header className="flex items-start justify-between mb-8">
            <div>
              <h1 className="text-xl font-semibold tracking-tight">Teams</h1>
              <p className="text-sm text-muted-foreground mt-1">
                Group tokens into teams for budget tracking and access control
              </p>
            </div>
            {isAdmin && (
              <Button
                onClick={() => {
                  setEditingTeam(undefined)
                  setModalOpen(true)
                }}
                className="gap-2"
              >
                <Plus className="h-4 w-4" />
                Create Team
              </Button>
            )}
          </header>

          {/* Table */}
          <div className="border rounded-lg">
            {loading ? (
              <div className="p-12 text-center">
                <div className="w-8 h-8 border-2 border-muted-foreground border-t-foreground rounded-full animate-spin mx-auto" />
              </div>
            ) : error ? (
              <div className="p-12 text-center">
                <p className="text-sm text-destructive">{error}</p>
              </div>
            ) : teams.length === 0 ? (
              <div className="p-12 text-center">
                <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
                  <Users className="h-6 w-6 text-muted-foreground" />
                </div>
                <p className="text-sm font-medium mb-1">No teams yet</p>
                <p className="text-sm text-muted-foreground">
                  Create your first team to organize tokens
                </p>
              </div>
            ) : (
              <table className="w-full">
                <thead>
                  <tr className="border-b bg-muted/30">
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Team</th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Budget</th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Period</th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Models</th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Created</th>
                    <th className="px-4 py-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      <span className="sr-only">Actions</span>
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {teams.map((team) => (
                    <tr
                      key={team.id}
                      className="hover:bg-muted/30 transition-colors cursor-pointer"
                      onClick={() => router.push(`/settings/teams/${team.id}`)}
                    >
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-3">
                          <div className="w-8 h-8 rounded-lg bg-muted flex items-center justify-center">
                            <Users className="h-4 w-4 text-muted-foreground" />
                          </div>
                          <div>
                            <span className="text-sm font-medium">{team.name}</span>
                            {team.description && (
                              <p className="text-xs text-muted-foreground">{team.description}</p>
                            )}
                          </div>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <BudgetProgress budget={team.max_budget_usd} spent={teamSpends[team.id] || 0} />
                      </td>
                      <td className="px-4 py-3">
                        <BudgetDurationBadge duration={team.budget_duration} />
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-xs text-muted-foreground">
                          {team.allowed_models && team.allowed_models.length > 0
                            ? `${team.allowed_models.length} model${team.allowed_models.length !== 1 ? "s" : ""}`
                            : "All models"}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-xs text-muted-foreground">
                          {formatRelativeTime(team.created_at)}
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
                            <DropdownMenuItem
                              onClick={(e) => {
                                e.stopPropagation()
                                router.push(`/settings/teams/${team.id}`)
                              }}
                            >
                              <BarChart3 className="h-4 w-4 mr-2" />
                              View Details
                            </DropdownMenuItem>
                            {isAdmin && (
                              <>
                                <DropdownMenuItem
                                  onClick={(e) => {
                                    e.stopPropagation()
                                    setEditingTeam(team)
                                    setModalOpen(true)
                                  }}
                                >
                                  <Edit className="h-4 w-4 mr-2" />
                                  Edit
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  className="text-destructive"
                                  onClick={(e) => {
                                    e.stopPropagation()
                                    handleDelete(team.id)
                                  }}
                                >
                                  <Trash2 className="h-4 w-4 mr-2" />
                                  Delete
                                </DropdownMenuItem>
                              </>
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

      {/* Team Form Modal */}
      <TeamFormModal
        open={modalOpen}
        onOpenChange={(open) => {
          setModalOpen(open)
          if (!open) setEditingTeam(undefined)
        }}
        team={editingTeam}
        onSubmit={editingTeam ? handleUpdate : handleCreate}
      />
    </div>
  )
}