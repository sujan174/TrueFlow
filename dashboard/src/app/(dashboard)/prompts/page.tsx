"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { Plus, FileText, MoreHorizontal, Trash2, Eye, Folder, Tag, GitBranch } from "lucide-react"
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
  listPrompts,
  createPrompt,
  deletePrompt,
  listPromptFolders,
  type PromptListResponse,
} from "@/lib/api"
import { LABEL_COLORS, type DeploymentLabel } from "@/lib/types/prompt"

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

function LabelBadge({ label }: { label: string }) {
  const colors = LABEL_COLORS[label as DeploymentLabel] || {
    bg: "bg-muted",
    text: "text-muted-foreground",
    border: "border-border",
  }
  return (
    <Badge variant="outline" className={`${colors.bg} ${colors.text} ${colors.border} text-[10px]`}>
      {label}
    </Badge>
  )
}

function CreatePromptModal({
  open,
  onOpenChange,
  folders,
  onSuccess,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  folders: string[]
  onSuccess: () => void
}) {
  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [description, setDescription] = useState("")
  const [folder, setFolder] = useState("/")
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      await createPrompt({
        name,
        slug: slug || undefined,
        description: description || undefined,
        folder: folder || undefined,
      })
      toast.success("Prompt created successfully")
      handleClose()
      onSuccess()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create prompt")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleClose = () => {
    setName("")
    setSlug("")
    setDescription("")
    setFolder("/")
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Create Prompt</DialogTitle>
          <DialogDescription>
            Create a new prompt template. You can add versions after creation.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium">Name *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="Customer Support Agent"
              required
            />
          </div>
          <div>
            <label className="text-sm font-medium">Slug</label>
            <input
              type="text"
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background font-mono"
              placeholder="customer-support-agent"
            />
            <p className="text-xs text-muted-foreground mt-1">
              URL-safe identifier. Auto-generated from name if empty.
            </p>
          </div>
          <div>
            <label className="text-sm font-medium">Description</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background min-h-[80px]"
              placeholder="Describe what this prompt does..."
            />
          </div>
          <div>
            <label className="text-sm font-medium">Folder</label>
            <select
              value={folder}
              onChange={(e) => setFolder(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
            >
              <option value="/">/ (root)</option>
              {folders.filter(f => f !== "/").map((f) => (
                <option key={f} value={f}>{f}</option>
              ))}
            </select>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={handleClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting || !name.trim()}>
              {isSubmitting ? "Creating..." : "Create Prompt"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default function PromptsPage() {
  const router = useRouter()
  const [prompts, setPrompts] = useState<PromptListResponse[]>([])
  const [folders, setFolders] = useState<string[]>([])
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [createModalOpen, setCreateModalOpen] = useState(false)

  const fetchData = async () => {
    try {
      const [promptsData, foldersData] = await Promise.all([
        listPrompts(selectedFolder || undefined),
        listPromptFolders(),
      ])
      setPrompts(promptsData)
      setFolders(foldersData)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load prompts")
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchData()
  }, [selectedFolder])

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this prompt? All versions will be removed.")) return

    try {
      await deletePrompt(id)
      setPrompts(prompts.filter((p) => p.id !== id))
      toast.success("Prompt deleted")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete prompt")
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Prompts
            </h1>
            <p className="text-sm text-muted-foreground">
              Manage prompt templates with version control
            </p>
          </div>
          <Button className="gap-2" onClick={() => setCreateModalOpen(true)}>
            <Plus className="h-4 w-4" />
            Create Prompt
          </Button>
        </div>

        {/* Folder Filter */}
        {folders.length > 1 && (
          <div className="flex items-center gap-2">
            <Folder className="h-4 w-4 text-muted-foreground" />
            <div className="flex gap-1">
              <Button
                variant={selectedFolder === null ? "secondary" : "ghost"}
                size="sm"
                onClick={() => setSelectedFolder(null)}
              >
                All
              </Button>
              {folders.map((folder) => (
                <Button
                  key={folder}
                  variant={selectedFolder === folder ? "secondary" : "ghost"}
                  size="sm"
                  onClick={() => setSelectedFolder(folder)}
                >
                  {folder === "/" ? "root" : folder}
                </Button>
              ))}
            </div>
          </div>
        )}

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading prompts...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : prompts.length === 0 ? (
            <div className="p-8 text-center">
              <FileText className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No prompts yet</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Create your first prompt template to get started
              </p>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Name</th>
                  <th className="px-4 py-3 text-left">Slug</th>
                  <th className="px-4 py-3 text-left">Folder</th>
                  <th className="px-4 py-3 text-left">Model</th>
                  <th className="px-4 py-3 text-left">Versions</th>
                  <th className="px-4 py-3 text-left">Labels</th>
                  <th className="px-4 py-3 text-left">Updated</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {prompts.map((prompt) => (
                  <tr
                    key={prompt.id}
                    className="border-b last:border-0 hover:bg-muted/30 transition-colors cursor-pointer"
                    onClick={() => router.push(`/prompts/${prompt.id}`)}
                  >
                    <td className="px-4 py-3">
                      <div className="flex flex-col">
                        <span className="text-sm font-medium">{prompt.name}</span>
                        {prompt.description && (
                          <span className="text-xs text-muted-foreground truncate max-w-[200px]">
                            {prompt.description}
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <code className="text-xs text-muted-foreground font-mono">
                        {prompt.slug}
                      </code>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground flex items-center gap-1">
                        <Folder className="h-3 w-3" />
                        {prompt.folder === "/" ? "root" : prompt.folder}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {prompt.latest_model || "—"}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm flex items-center gap-1">
                        <GitBranch className="h-3 w-3 text-muted-foreground" />
                        {prompt.version_count}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex gap-1 flex-wrap">
                        {prompt.labels.length > 0 ? (
                          prompt.labels.map((label) => (
                            <LabelBadge key={label} label={label} />
                          ))
                        ) : (
                          <span className="text-sm text-muted-foreground">—</span>
                        )}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {formatRelativeTime(prompt.updated_at)}
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
                          <DropdownMenuItem onClick={() => router.push(`/prompts/${prompt.id}`)}>
                            <Eye className="h-4 w-4 mr-2" />
                            View & Edit
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            className="text-destructive"
                            onClick={(e) => {
                              e.stopPropagation()
                              handleDelete(prompt.id)
                            }}
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

      {/* Create Prompt Modal */}
      <CreatePromptModal
        open={createModalOpen}
        onOpenChange={setCreateModalOpen}
        folders={folders}
        onSuccess={fetchData}
      />
    </div>
  )
}