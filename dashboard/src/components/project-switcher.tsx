"use client";

import * as React from "react";
import { ChevronsUpDown, Plus, Check, Building2, Trash2, AlertTriangle, Layers } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useProject } from "@/contexts/project-context";
import { toast } from "sonner";

interface ProjectSwitcherProps extends React.HTMLAttributes<HTMLDivElement> {
    collapsed?: boolean;
}

export function ProjectSwitcher({ className, collapsed }: ProjectSwitcherProps) {
    const { projects, selectedProjectId, selectProject, createProject, deleteProject } = useProject();
    const [open, setOpen] = React.useState(false);
    const [showNewProjectDialog, setShowNewProjectDialog] = React.useState(false);
    const [showDeleteDialog, setShowDeleteDialog] = React.useState(false);
    const [projectToDelete, setProjectToDelete] = React.useState<{ id: string; name: string } | null>(null);
    const [newProjectName, setNewProjectName] = React.useState("");
    const [deleteConfirmName, setDeleteConfirmName] = React.useState("");
    const [deleting, setDeleting] = React.useState(false);

    const selectedProject = projects.find((p) => p.id === selectedProjectId);

    // The "default" project is the first (oldest) one — the gateway blocks deleting it too.
    const defaultProjectId = projects.length > 0 ? projects[0].id : null;

    const handleCreate = async () => {
        if (!newProjectName.trim()) return;
        await createProject(newProjectName.trim());
        setShowNewProjectDialog(false);
        setNewProjectName("");
    };

    const openDeleteDialog = (e: React.MouseEvent, project: { id: string; name: string }) => {
        e.stopPropagation(); // Don't select the project
        setProjectToDelete(project);
        setDeleteConfirmName("");
        setShowDeleteDialog(true);
        setOpen(false);
    };

    const handleDelete = async () => {
        if (!projectToDelete) return;
        if (deleteConfirmName !== projectToDelete.name) {
            toast.error("Project name doesn't match");
            return;
        }
        setDeleting(true);
        try {
            await deleteProject(projectToDelete.id);
            setShowDeleteDialog(false);
            setProjectToDelete(null);
        } catch (e: unknown) {
            const err = e as Error;
            const msg = err.message || "";
            if (msg.includes("400")) {
                toast.error("Cannot delete the default project");
            } else if (msg.includes("403")) {
                toast.error("Only admins can delete projects");
            } else {
                toast.error("Failed to delete project");
            }
        } finally {
            setDeleting(false);
        }
    };

    return (
        <>
            {/* Create Project Dialog */}
            <Dialog open={showNewProjectDialog} onOpenChange={setShowNewProjectDialog}>
                <DialogContent className="bg-zinc-950 border-white/10 text-white">
                    <DialogHeader>
                        <DialogTitle className="text-white font-medium">Create Project</DialogTitle>
                        <DialogDescription className="text-zinc-500 text-[13px]">
                            Add a new project to manage isolated resources.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        <div className="grid gap-2">
                            <Label htmlFor="name" className="text-zinc-400 text-xs uppercase tracking-widest">Project Name</Label>
                            <Input
                                id="name"
                                placeholder="Ex. Marketing Prod"
                                value={newProjectName}
                                onChange={(e) => setNewProjectName(e.target.value)}
                                onKeyDown={(e) => e.key === "Enter" && handleCreate()}
                                className="bg-black border-white/10 text-white placeholder:text-zinc-600 focus-visible:ring-white/20"
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="ghost" onClick={() => setShowNewProjectDialog(false)}>Cancel</Button>
                        <Button variant="default" onClick={handleCreate} disabled={!newProjectName.trim()}>Create</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Delete Project Confirmation Dialog */}
            <Dialog open={showDeleteDialog} onOpenChange={(v) => { if (!deleting) setShowDeleteDialog(v); }}>
                <DialogContent className="bg-zinc-950 border-rose-500/20 text-white">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2 text-rose-500 font-medium">
                            <AlertTriangle className="h-5 w-5" />
                            Delete Project
                        </DialogTitle>
                        <DialogDescription className="pt-1 text-zinc-400 text-[13px]">
                            This will permanently delete{" "}
                            <strong className="text-white font-medium">{projectToDelete?.name}</strong>{" "}
                            and <strong className="text-rose-400 font-medium">all of its tokens, credentials, policies, and audit logs</strong>.
                            This action cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="grid gap-3 py-4">
                        <Label htmlFor="confirm-name" className="text-[12px] text-zinc-500">
                            Type <code className="font-mono bg-white/5 px-1.5 py-0.5 rounded text-white">{projectToDelete?.name}</code> to confirm
                        </Label>
                        <Input
                            id="confirm-name"
                            placeholder={projectToDelete?.name}
                            value={deleteConfirmName}
                            onChange={(e) => setDeleteConfirmName(e.target.value)}
                            onKeyDown={(e) => e.key === "Enter" && handleDelete()}
                            className="font-mono bg-black border-rose-500/30 text-rose-100 placeholder:text-rose-900/50 focus-visible:ring-rose-500/40"
                        />
                    </div>
                    <DialogFooter>
                        <Button variant="ghost" onClick={() => setShowDeleteDialog(false)} disabled={deleting}>
                            Cancel
                        </Button>
                        <Button
                            variant="destructive"
                            onClick={handleDelete}
                            disabled={deleteConfirmName !== projectToDelete?.name || deleting}
                        >
                            {deleting ? "Deleting…" : "Delete Project"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Project Dropdown */}
            <DropdownMenu open={open} onOpenChange={setOpen}>
                <DropdownMenuTrigger asChild>
                    <Button
                        variant="outline"
                        role="combobox"
                        aria-expanded={open}
                        aria-label="Select a project"
                        className={cn("w-full justify-between hover:bg-white/5 hover:text-white transition-colors", className)}
                    >
                        <div className="flex items-center gap-2 overflow-hidden">
                            <Layers className="h-4 w-4 text-zinc-500 shrink-0" />
                            <span className="truncate text-zinc-300 font-medium">{selectedProject?.name || "Select Project..."}</span>
                        </div>
                        <ChevronsUpDown className="ml-2 h-3.5 w-3.5 opacity-50 shrink-0" />
                    </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent className="w-[220px] p-1 bg-zinc-950 border-white/[0.06]" align="end">
                    <DropdownMenuGroup>
                        <DropdownMenuLabel className="text-[10px] uppercase tracking-widest text-zinc-500 px-2 py-1.5">Projects</DropdownMenuLabel>
                        {projects.map((project) => {
                            const isDefault = project.id === defaultProjectId;
                            return (
                                <DropdownMenuItem
                                    key={project.id}
                                    onSelect={() => {
                                        selectProject(project.id);
                                        setOpen(false);
                                    }}
                                    className="text-[13px] group pr-2 focus:bg-white/5 focus:text-white rounded-sm py-1.5 cursor-pointer"
                                >
                                    <Layers className="mr-2 h-3.5 w-3.5 flex-shrink-0 text-zinc-500 group-focus:text-white transition-colors" />
                                    <span className="truncate flex-1 font-medium text-zinc-300 group-focus:text-white">{project.name}</span>
                                    <Check
                                        className={cn(
                                            "h-3.5 w-3.5 flex-shrink-0 mr-1 text-white",
                                            selectedProjectId === project.id ? "opacity-100" : "opacity-0"
                                        )}
                                    />
                                    {/* Delete button — hidden for default project */}
                                    {!isDefault && (
                                        <button
                                            onClick={(e) => openDeleteDialog(e, project)}
                                            className="ml-1 h-5 w-5 flex items-center justify-center rounded opacity-0 group-hover:opacity-100 hover:bg-rose-500/10 hover:text-rose-400 transition-all flex-shrink-0"
                                            title="Delete project (admin only)"
                                        >
                                            <Trash2 className="h-3 w-3" />
                                        </button>
                                    )}
                                </DropdownMenuItem>
                            );
                        })}
                    </DropdownMenuGroup>
                    <DropdownMenuSeparator className="bg-white/[0.06]" />
                    <DropdownMenuItem
                        onSelect={() => {
                            setOpen(false);
                            setShowNewProjectDialog(true);
                        }}
                        className="text-[13px] focus:bg-white/5 focus:text-white rounded-sm py-1.5 cursor-pointer text-zinc-400"
                    >
                        <Plus className="mr-2 h-3.5 w-3.5" />
                        Create Project
                    </DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </>
    );
}
