"use client"

import { useState } from "react"
import { ChevronDown, Check, FolderOpen } from "lucide-react"
import { useProject } from "@/contexts/project-context"
import { cn } from "@/lib/utils"

export function ProjectSwitcher() {
  const { projects, selectedProject, selectProject, isLoading } = useProject()
  const [isOpen, setIsOpen] = useState(false)

  if (isLoading) {
    return (
      <div className="h-9 px-3 animate-pulse bg-muted rounded-lg mx-3" />
    )
  }

  if (projects.length === 0) {
    return null
  }

  return (
    <div className="relative px-3 py-2">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center justify-between h-9 px-3 bg-muted hover:bg-muted/80 rounded-lg text-foreground text-[13px] font-medium transition-colors"
      >
        <div className="flex items-center gap-2">
          <FolderOpen className="w-4 h-4 text-muted-foreground" />
          <span className="truncate">{selectedProject?.name || "Select project"}</span>
        </div>
        <ChevronDown className={cn("w-4 h-4 text-muted-foreground transition-transform", isOpen && "rotate-180")} />
      </button>

      {isOpen && (
        <>
          <div
            className="fixed inset-0 z-10"
            onClick={() => setIsOpen(false)}
          />
          <div className="absolute left-3 right-3 top-full mt-1 z-20 bg-popover rounded-lg border overflow-hidden shadow-lg">
            {projects.map((project) => (
              <button
                key={project.id}
                onClick={() => {
                  selectProject(project.id)
                  setIsOpen(false)
                }}
                className="w-full flex items-center justify-between px-3 h-9 text-[13px] text-foreground hover:bg-muted transition-colors"
              >
                <span className="truncate">{project.name}</span>
                {selectedProject?.id === project.id && (
                  <Check className="w-4 h-4 text-primary" />
                )}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  )
}