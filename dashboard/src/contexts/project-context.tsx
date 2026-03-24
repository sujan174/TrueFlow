"use client"

import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react"
import { listProjects, createProject as apiCreateProject, updateLastProject, type Project } from "@/lib/api"

interface ProjectContextType {
  projects: Project[]
  selectedProjectId: string | null
  selectedProject: Project | null
  isLoading: boolean
  error: string | null
  selectProject: (id: string) => void
  createProject: (name: string) => Promise<Project>
  refreshProjects: () => Promise<void>
  initializeFromBackend: (lastProjectId: string | null) => void
}

const ProjectContext = createContext<ProjectContextType | null>(null)

const STORAGE_KEY = "trueflow_project_id"

interface ProjectProviderProps {
  children: ReactNode
  /** Last project ID from backend (passed from auth sync) */
  initialProjectId?: string | null
}

export function ProjectProvider({ children, initialProjectId }: ProjectProviderProps) {
  const [projects, setProjects] = useState<Project[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [backendProjectId, setBackendProjectId] = useState<string | null>(initialProjectId || null)

  const fetchProjects = useCallback(async () => {
    try {
      setIsLoading(true)
      setError(null)
      const data = await listProjects()
      setProjects(data)

      // Priority for selecting project:
      // 1. Backend last_project_id (if valid)
      // 2. localStorage (if valid)
      // 3. First project in list
      if (backendProjectId && data.some(p => p.id === backendProjectId)) {
        setSelectedProjectId(backendProjectId)
        localStorage.setItem(STORAGE_KEY, backendProjectId)
      } else {
        const storedId = localStorage.getItem(STORAGE_KEY)
        if (storedId && data.some(p => p.id === storedId)) {
          setSelectedProjectId(storedId)
        } else if (data.length > 0) {
          const firstId = data[0].id
          setSelectedProjectId(firstId)
          localStorage.setItem(STORAGE_KEY, firstId)
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load projects")
    } finally {
      setIsLoading(false)
    }
  }, [backendProjectId])

  useEffect(() => {
    fetchProjects()
  }, [fetchProjects])

  const selectProject = useCallback(async (id: string) => {
    setSelectedProjectId(id)
    localStorage.setItem(STORAGE_KEY, id)

    // Persist to backend (non-blocking, fallback to localStorage if it fails)
    try {
      await updateLastProject(id)
    } catch (err) {
      // Silently fail - localStorage is the fallback
      console.warn("Failed to persist project selection to backend:", err)
    }
  }, [])

  const createProject = useCallback(async (name: string): Promise<Project> => {
    const project = await apiCreateProject(name)
    await fetchProjects()
    await selectProject(project.id)
    return project
  }, [fetchProjects, selectProject])

  const refreshProjects = useCallback(async () => {
    await fetchProjects()
  }, [fetchProjects])

  const initializeFromBackend = useCallback((lastProjectId: string | null) => {
    setBackendProjectId(lastProjectId)
  }, [])

  const selectedProject = projects.find(p => p.id === selectedProjectId) || null

  return (
    <ProjectContext.Provider
      value={{
        projects,
        selectedProjectId,
        selectedProject,
        isLoading,
        error,
        selectProject,
        createProject,
        refreshProjects,
        initializeFromBackend,
      }}
    >
      {children}
    </ProjectContext.Provider>
  )
}

export function useProject() {
  const context = useContext(ProjectContext)
  if (!context) {
    throw new Error("useProject must be used within a ProjectProvider")
  }
  return context
}