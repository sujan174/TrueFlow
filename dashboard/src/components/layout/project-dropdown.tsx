"use client"

import { useState } from "react"
import { useRouter } from "next/navigation"
import { ChevronDown, Check, Plus, Settings, FolderOpen } from "lucide-react"
import { useProject } from "@/contexts/project-context"
import { cn } from "@/lib/utils"

export function ProjectDropdown() {
  const { projects, selectedProject, selectProject, isLoading } = useProject()
  const [isOpen, setIsOpen] = useState(false)
  const router = useRouter()

  if (isLoading) {
    return (
      <div className="h-8 w-32 animate-pulse bg-muted rounded-md" />
    )
  }

  if (projects.length === 0) {
    return null
  }

  const handleSelectProject = (projectId: string) => {
    selectProject(projectId)
    setIsOpen(false)
  }

  const handleManageProjects = () => {
    setIsOpen(false)
    router.push("/settings/projects")
  }

  const handleNewProject = () => {
    setIsOpen(false)
    // For now, navigate to settings/projects to create
    router.push("/settings/projects?action=new")
  }

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 h-8 px-3 bg-muted/50 hover:bg-muted rounded-md text-sm font-medium transition-colors"
      >
        <FolderOpen className="w-4 h-4 text-muted-foreground" />
        <span className="max-w-[120px] truncate">
          {selectedProject?.name || "Select project"}
        </span>
        <ChevronDown
          className={cn(
            "w-3.5 h-3.5 text-muted-foreground transition-transform",
            isOpen && "rotate-180"
          )}
        />
      </button>

      {isOpen && (
        <>
          {/* Backdrop */}
          <div
            className="fixed inset-0 z-10"
            onClick={() => setIsOpen(false)}
          />

          {/* Dropdown */}
          <div className="absolute left-0 top-full mt-1 w-56 z-20 bg-popover rounded-lg border shadow-lg overflow-hidden">
            {/* Project list */}
            <div className="max-h-48 overflow-y-auto py-1">
              {projects.map((project) => (
                <button
                  key={project.id}
                  onClick={() => handleSelectProject(project.id)}
                  className="w-full flex items-center justify-between px-3 py-2 text-sm hover:bg-muted transition-colors"
                >
                  <span className="truncate">{project.name}</span>
                  {selectedProject?.id === project.id && (
                    <Check className="w-4 h-4 text-primary shrink-0" />
                  )}
                </button>
              ))}
            </div>

            {/* Actions */}
            <div className="border-t py-1">
              <button
                onClick={handleNewProject}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted transition-colors"
              >
                <Plus className="w-4 h-4 text-muted-foreground" />
                <span>New Project</span>
              </button>
              <button
                onClick={handleManageProjects}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted transition-colors"
              >
                <Settings className="w-4 h-4 text-muted-foreground" />
                <span>Manage Projects</span>
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  )
}