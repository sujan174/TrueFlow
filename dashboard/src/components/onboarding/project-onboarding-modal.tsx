"use client"

import { useState } from "react"
import { useProject } from "@/contexts/project-context"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { FolderPlus, Loader2, Sparkles } from "lucide-react"

interface ProjectOnboardingModalProps {
  isOpen: boolean
  onClose: () => void
}

export function ProjectOnboardingModal({ isOpen, onClose }: ProjectOnboardingModalProps) {
  const { createProject } = useProject()
  const [name, setName] = useState("")
  const [isCreating, setIsCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  if (!isOpen) return null

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      setError("Project name is required")
      return
    }

    try {
      setIsCreating(true)
      setError(null)
      await createProject(name.trim())
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create project")
    } finally {
      setIsCreating(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop - non-dismissible for new users */}
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" />

      {/* Modal */}
      <div className="relative w-full max-w-lg bg-card rounded-2xl border shadow-2xl overflow-hidden animate-in fade-in-0 zoom-in-95 duration-300">
        {/* Header with gradient */}
        <div className="relative px-8 pt-10 pb-6 text-center">
          <div className="absolute inset-0 bg-gradient-to-b from-primary/10 to-transparent" />

          {/* Icon */}
          <div className="relative mx-auto w-20 h-20 rounded-2xl bg-primary/20 flex items-center justify-center mb-6">
            <FolderPlus className="w-10 h-10 text-primary" />
          </div>

          <h2 className="relative text-2xl font-bold text-foreground mb-2">
            Welcome to AILink
          </h2>
          <p className="relative text-muted-foreground max-w-sm mx-auto">
            Create your first project to start managing your AI gateway.
            Projects organize your virtual keys, policies, and analytics.
          </p>
        </div>

        {/* Form */}
        <div className="px-8 pb-8">
          <form onSubmit={handleSubmit}>
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="project-name" className="text-sm font-medium">
                  Project name
                </Label>
                <Input
                  id="project-name"
                  value={name}
                  onChange={(e) => {
                    setName(e.target.value)
                    setError(null)
                  }}
                  placeholder="My First Project"
                  className="h-11 bg-background"
                  autoFocus
                  disabled={isCreating}
                />
                {error && (
                  <p className="text-xs text-destructive">{error}</p>
                )}
              </div>

              <Button
                type="submit"
                className="w-full h-11"
                disabled={isCreating || !name.trim()}
              >
                {isCreating ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Creating...
                  </>
                ) : (
                  <>
                    <Sparkles className="mr-2 h-4 w-4" />
                    Create Project
                  </>
                )}
              </Button>
            </div>
          </form>

          <p className="text-xs text-muted-foreground text-center mt-6">
            You can create additional projects from Settings at any time.
          </p>
        </div>
      </div>
    </div>
  )
}