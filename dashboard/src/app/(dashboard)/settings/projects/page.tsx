"use client"

import { useState, useEffect } from "react"
import { useProject } from "@/contexts/project-context"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  FolderPlus,
  MoreHorizontal,
  Pencil,
  Trash2,
  Loader2,
  FolderOpen,
  Check,
} from "lucide-react"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { updateProject, deleteProject, type Project } from "@/lib/api"
import { useRouter, useSearchParams, usePathname } from "next/navigation"
import { cn } from "@/lib/utils"
import { SettingsSidebar } from "../_components/settings-sidebar"
import { usePermissions } from "@/contexts/permissions-context"

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

export default function ProjectsSettingsPage() {
  const { isAdmin } = usePermissions()
  const { projects, selectedProject, selectProject, createProject, refreshProjects, isLoading } = useProject()
  const [showCreateDialog, setShowCreateDialog] = useState(false)
  const [showRenameDialog, setShowRenameDialog] = useState(false)
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [projectToEdit, setProjectToEdit] = useState<Project | null>(null)
  const [name, setName] = useState("")
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const router = useRouter()
  const searchParams = useSearchParams()
  const pathname = usePathname()

  useEffect(() => {
    if (searchParams.get("action") === "new") {
      setShowCreateDialog(true)
      router.replace("/settings/projects")
    }
  }, [searchParams, router])

  const handleCreate = async () => {
    if (!name.trim()) {
      setError("Project name is required")
      return
    }

    setIsSubmitting(true)
    setError(null)

    try {
      await createProject(name.trim())
      setShowCreateDialog(false)
      setName("")
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create project")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleRename = async () => {
    if (!projectToEdit || !name.trim()) {
      setError("Project name is required")
      return
    }

    setIsSubmitting(true)
    setError(null)

    try {
      await updateProject(projectToEdit.id, name.trim())
      await refreshProjects()
      setShowRenameDialog(false)
      setProjectToEdit(null)
      setName("")
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to rename project")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleDelete = async () => {
    if (!projectToEdit) return

    setIsSubmitting(true)
    setError(null)

    try {
      await deleteProject(projectToEdit.id)
      await refreshProjects()
      setShowDeleteDialog(false)
      setProjectToEdit(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete project")
    } finally {
      setIsSubmitting(false)
    }
  }

  const openRenameDialog = (project: Project) => {
    setProjectToEdit(project)
    setName(project.name)
    setShowRenameDialog(true)
  }

  const openDeleteDialog = (project: Project) => {
    setProjectToEdit(project)
    setShowDeleteDialog(true)
  }

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
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
              <h1 className="text-xl font-semibold tracking-tight">Projects</h1>
              <p className="text-sm text-muted-foreground mt-1">
                Create and manage projects for multi-tenant isolation
              </p>
            </div>
            {isAdmin && (
              <Button onClick={() => setShowCreateDialog(true)} className="gap-2">
                <FolderPlus className="h-4 w-4" />
                New Project
              </Button>
            )}
          </header>

          {/* Projects List */}
          <div className="border rounded-lg">
            {projects.length === 0 ? (
              <div className="p-12 text-center">
                <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
                  <FolderOpen className="h-6 w-6 text-muted-foreground" />
                </div>
                <p className="text-sm font-medium mb-1">No projects yet</p>
                <p className="text-sm text-muted-foreground">
                  Create your first project to get started
                </p>
              </div>
            ) : (
              <div className="divide-y">
                {projects.map((project) => (
                  <div
                    key={project.id}
                    className="flex items-center justify-between p-4 hover:bg-muted/30 transition-colors"
                  >
                    <div className="flex items-center gap-4">
                      <div className="w-10 h-10 rounded-lg bg-muted flex items-center justify-center">
                        <FolderOpen className="h-5 w-5 text-muted-foreground" />
                      </div>
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium">{project.name}</span>
                          {selectedProject?.id === project.id && (
                            <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-full bg-primary/10 text-primary">
                              <Check className="h-3 w-3" />
                              Active
                            </span>
                          )}
                        </div>
                        <span className="text-xs text-muted-foreground">
                          Created {project.created_at ? formatRelativeTime(project.created_at) : "N/A"}
                        </span>
                      </div>
                    </div>

                    <div className="flex items-center gap-2">
                      {selectedProject?.id !== project.id && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => selectProject(project.id)}
                        >
                          Select
                        </Button>
                      )}
                      <DropdownMenu>
                        <DropdownMenuTrigger>
                          <Button variant="ghost" size="icon-sm">
                            <MoreHorizontal className="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          {isAdmin && (
                            <>
                              <DropdownMenuItem onClick={() => openRenameDialog(project)}>
                                <Pencil className="mr-2 h-4 w-4" />
                                Rename
                              </DropdownMenuItem>
                              {projects.length > 1 && (
                                <DropdownMenuItem
                                  onClick={() => openDeleteDialog(project)}
                                  className="text-destructive focus:text-destructive"
                                >
                                  <Trash2 className="mr-2 h-4 w-4" />
                                  Delete
                                </DropdownMenuItem>
                              )}
                            </>
                          )}
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Create Project Dialog */}
      <Dialog open={showCreateDialog} onOpenChange={setShowCreateDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Project</DialogTitle>
            <DialogDescription>
              Enter a name for your new project.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="create-name">Name</Label>
              <Input
                id="create-name"
                value={name}
                onChange={(e) => {
                  setName(e.target.value)
                  setError(null)
                }}
                placeholder="My Project"
              />
              {error && <p className="text-sm text-destructive">{error}</p>}
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setShowCreateDialog(false)
                setName("")
                setError(null)
              }}
            >
              Cancel
            </Button>
            <Button onClick={handleCreate} disabled={isSubmitting || !name.trim()}>
              {isSubmitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Creating...
                </>
              ) : (
                "Create"
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Rename Project Dialog */}
      <Dialog open={showRenameDialog} onOpenChange={setShowRenameDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Rename Project</DialogTitle>
            <DialogDescription>
              Enter a new name for this project.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="rename-name">Name</Label>
              <Input
                id="rename-name"
                value={name}
                onChange={(e) => {
                  setName(e.target.value)
                  setError(null)
                }}
              />
              {error && <p className="text-sm text-destructive">{error}</p>}
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setShowRenameDialog(false)
                setProjectToEdit(null)
                setName("")
                setError(null)
              }}
            >
              Cancel
            </Button>
            <Button onClick={handleRename} disabled={isSubmitting || !name.trim()}>
              {isSubmitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Saving...
                </>
              ) : (
                "Save"
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Project Dialog */}
      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete Project</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete "{projectToEdit?.name}"? This action cannot be undone. All associated tokens, policies, and credentials will be deleted.
            </DialogDescription>
          </DialogHeader>
          {error && (
            <div className="py-2">
              <p className="text-sm text-destructive">{error}</p>
            </div>
          )}
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setShowDeleteDialog(false)
                setProjectToEdit(null)
                setError(null)
              }}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={isSubmitting}
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Deleting...
                </>
              ) : (
                "Delete"
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}