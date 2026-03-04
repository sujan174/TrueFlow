"use client";

import React, { createContext, useContext, useEffect, useState, useCallback } from "react";
import { listProjects, createProject as apiCreateProject, updateProject as apiUpdateProject, deleteProject as apiDeleteProject, Project } from "@/lib/api";
import { toast } from "sonner";


interface ProjectContextType {
    projects: Project[];
    selectedProjectId: string | null;
    isLoading: boolean;
    selectProject: (projectId: string) => void;
    createProject: (name: string) => Promise<void>;
    updateProject: (id: string, name: string) => Promise<void>;
    deleteProject: (id: string) => Promise<void>;
    refreshProjects: () => Promise<void>;
}

const ProjectContext = createContext<ProjectContextType | undefined>(undefined);

export function ProjectProvider({ children }: { children: React.ReactNode }) {
    const [projects, setProjects] = useState<Project[]>([]);
    const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(true);


    const refreshProjects = useCallback(async () => {
        try {
            const data = await listProjects();
            setProjects(data);

            // Auto-select if none selected
            const cached = localStorage.getItem("trueflow_project_id");
            if (cached && data.find(p => p.id === cached)) {
                setSelectedProjectId(cached);
            } else if (data.length > 0) {
                // Default to first one (likely 'default')
                const defaultProj = data[0].id;
                setSelectedProjectId(defaultProj);
                localStorage.setItem("trueflow_project_id", defaultProj);
            }
        } catch {
            toast.error("Failed to load projects");
        } finally {
            setIsLoading(false);
        }
    }, []);

    useEffect(() => {
        refreshProjects();
    }, [refreshProjects]);

    const selectProject = (projectId: string) => {
        setSelectedProjectId(projectId);
        localStorage.setItem("trueflow_project_id", projectId);
        // Reload page to force all data fetches to refresh with new ID
        // This is a crude but effective way to ensure all useStates/useEffects in pages reset
        window.location.reload();
    };

    const createProject = async (name: string) => {
        try {
            const newProj = await apiCreateProject(name);
            toast.success("Project created");
            await refreshProjects();
            selectProject(newProj.id);
        } catch (e) {
            toast.error("Failed to create project");
            throw e;
        }
    };

    const updateProject = async (id: string, name: string) => {
        try {
            await apiUpdateProject(id, name);
            toast.success("Project updated");
            await refreshProjects();
        } catch (e) {
            toast.error("Failed to update project");
            throw e;
        }
    };

    const deleteProject = async (id: string) => {
        try {
            await apiDeleteProject(id);
            toast.success("Project deleted");
            await refreshProjects();
            // If deleted current project, switch to default
            if (id === selectedProjectId) {
                // Find default or first available
                // We'll trust refreshProjects will fix the state or we force a reload knowing the ID is gone
                localStorage.removeItem("trueflow_project_id");
                window.location.reload();
            }
        } catch (e) {
            toast.error("Failed to delete project");
            throw e;
        }
    };

    return (
        <ProjectContext.Provider value={{
            projects,
            selectedProjectId,
            isLoading,
            selectProject,
            createProject,
            updateProject,
            deleteProject,
            refreshProjects
        }}>
            {children}
        </ProjectContext.Provider>
    );
}

export function useProject() {
    const context = useContext(ProjectContext);
    if (context === undefined) {
        throw new Error("useProject must be used within a ProjectProvider");
    }
    return context;
}
