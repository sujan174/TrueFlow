"use client"

import { useState, type ReactNode } from "react"
import { ProjectProvider, useProject } from "@/contexts/project-context"
import { ProjectOnboardingModal } from "@/components/onboarding/project-onboarding-modal"

function ProjectOnboardingHandler() {
  const { projects, isLoading } = useProject()
  const [dismissed, setDismissed] = useState(false)

  // Show onboarding when:
  // 1. Not loading
  // 2. No projects exist
  // 3. User hasn't dismissed it
  //
  // With the new user signup flow, each new user gets their own org and project,
  // so this should rarely trigger. It's kept as a safety net.
  const showOnboarding = !isLoading && projects.length === 0 && !dismissed

  return (
    <ProjectOnboardingModal
      isOpen={showOnboarding}
      onClose={() => setDismissed(true)}
    />
  )
}

interface ProjectProviderWrapperProps {
  children: ReactNode
  /** Last project ID from backend (passed from auth sync via cookie) */
  initialProjectId?: string | null
}

export function ProjectProviderWrapper({ children, initialProjectId }: ProjectProviderWrapperProps) {
  return (
    <ProjectProvider initialProjectId={initialProjectId}>
      {children}
      <ProjectOnboardingHandler />
    </ProjectProvider>
  )
}