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
  Check,
  Loader2,
} from "lucide-react"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Badge } from "@/components/ui/badge"
import { updateProject, deleteProject, type Project } from "@/lib/api"
import { useRouter, useSearchParams } from "next/navigation"

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

  // Check for action=new query param
  useEffect(() => {
    if (searchParams.get("action") === "new") {
      setShowCreateDialog(true)
      // Clear the query param
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
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Projects
            </h1>
            <p className="text-sm text-muted-foreground">
              Manage your projects and their settings
            </p>
          </div>
          <Button className="gap-2" onClick={() => setShowCreateDialog(true)}>
            <FolderPlus className="h-4 w-4" />
            New Project
          </Button>
        </div>

        {/* Projects List */}
        <div className="bg-card border rounded-lg divide-y">
          {projects.length === 0 ? (
            <div className="p-8 text-center text-muted-foreground">
              <FolderPlus className="h-10 w-10 mx-auto mb-3 opacity-50" />
              <p>No projects yet</p>
              <p className="text-sm">Create your first project to get started</p>
            </div>
          ) : (
            projects.map((project) => (
              <div
                key={project.id}
                className="flex items-center justify-between p-4 hover:bg-muted/30 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <div className="flex flex-col">
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{project.name}</span>
                      {selectedProject?.id === project.id && (
                        <Badge variant="secondary" className="text-xs">
                          Active
                        </Badge>
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
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Create Project Dialog */}
      <Dialog open={showCreateDialog} onOpenChange={setShowCreateDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create New Project</DialogTitle>
            <DialogDescription>
              Enter a name for your new project.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="create-name">Project Name</Label>
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
                "Create Project"
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
              <Label htmlFor="rename-name">Project Name</Label>
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
                "Save Changes"
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
              Are you sure you want to delete "{projectToEdit?.name}"? This action
              cannot be undone. All associated tokens, policies, and credentials
              will be deleted.
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
                "Delete Project"
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}