"use client";

import { useEffect, useState } from "react";
import { useProject } from "@/contexts/project-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { AlertTriangle, Trash2, Save, ShieldOff } from "lucide-react";
import { toast } from "sonner";
import { useRouter } from "next/navigation";
import { purgeProjectData } from "@/lib/api";

export default function ProjectSettingsPage() {
    const { projects, selectedProjectId, updateProject, deleteProject } = useProject();
    const router = useRouter();

    // Find the currently selected project
    const project = projects.find(p => p.id === selectedProjectId);

    // Local state for the form
    const [projectName, setProjectName] = useState("");
    const [isRenaming, setIsRenaming] = useState(false);

    // Local state for deletion confirmation
    const [deleteConfirmation, setDeleteConfirmation] = useState("");
    const [isDeleting, setIsDeleting] = useState(false);
    const [isPurging, setIsPurging] = useState(false);
    const [purgeConfirmation, setPurgeConfirmation] = useState("");

    // Initialize state when project loads
    useEffect(() => {
        if (project) {
            setProjectName(project.name);
        }
    }, [project]);

    if (!project) {
        return <div className="p-4 animate-pulse text-muted-foreground">Loading project details...</div>;
    }

    const handleRename = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!projectName.trim()) return;

        setIsRenaming(true);
        try {
            await updateProject(project.id, projectName);
            // Success toast is handled in context
        } catch (error) {
            // Error toast is handled in context
            console.error(error);
        } finally {
            setIsRenaming(false);
        }
    };

    const handleDelete = async () => {
        if (deleteConfirmation !== project.name) {
            toast.error("Please type the project name to confirm deletion");
            return;
        }

        if (confirm("Are you absolutely sure? This action cannot be undone.")) {
            setIsDeleting(true);
            try {
                await deleteProject(project.id);
                router.push("/");
            } catch (error) {
                console.error(error);
                setIsDeleting(false);
            }
        }
    };

    const handlePurge = async () => {
        if (purgeConfirmation !== `PURGE ${project.name}`) {
            toast.error(`Type PURGE ${project.name} exactly`);
            return;
        }
        if (!confirm("FINAL WARNING: This will permanently erase all audit logs, sessions, and usage data. Real credentials and tokens are unaffected but logs cannot be recovered. Continue?")) return;
        setIsPurging(true);
        try {
            const result = await purgeProjectData(project.id);
            toast.success(result.message || "Project data purged");
            setPurgeConfirmation("");
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Purge failed");
        } finally {
            setIsPurging(false);
        }
    };

    return (
        <div className="flex flex-col gap-4 max-w-4xl animate-fade-in p-4 pt-2">
            {/* General Settings */}

            {/* General Settings */}
            <Card>
                <CardHeader>
                    <CardTitle>General</CardTitle>
                    <CardDescription>
                        Update your project&apos;s display name and view unique identifiers.
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-6">
                    <div className="grid gap-2">
                        <Label htmlFor="projectId">Project ID</Label>
                        <Input
                            id="projectId"
                            value={project.id}
                            readOnly
                            className="bg-muted font-mono"
                        />
                        <p className="text-xs text-muted-foreground">
                            Used when making API requests to identify this project.
                        </p>
                    </div>

                    <form onSubmit={handleRename} className="grid gap-2">
                        <Label htmlFor="projectName">Project Name</Label>
                        <div className="flex gap-2">
                            <Input
                                id="projectName"
                                value={projectName}
                                onChange={(e) => setProjectName(e.target.value)}
                                placeholder="My Awesome Project"
                            />
                            <Button
                                type="submit"
                                disabled={isRenaming || !projectName.trim() || projectName === project.name}
                            >
                                {isRenaming ? "Saving..." : <><Save className="mr-2 h-4 w-4" /> Save Name</>}
                            </Button>
                        </div>
                    </form>
                </CardContent>
            </Card>

            {/* Danger Zone */}
            <Card className="border-destructive/20 bg-destructive/5">
                <CardHeader>
                    <CardTitle className="text-destructive flex items-center gap-2">
                        <AlertTriangle className="h-5 w-5" /> Danger Zone
                    </CardTitle>
                    <CardDescription>
                        Irreversible actions. Proceed with caution.
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="rounded-md border border-destructive/20 bg-background p-4">
                        <h3 className="font-semibold text-destructive mb-2">Delete Project</h3>
                        <p className="text-[13px] text-muted-foreground mb-4">
                            This will permanently delete the project <strong>{project.name}</strong> and all its associated resources (tokens, logs, services). This action cannot be undone.
                        </p>

                        <div className="space-y-2">
                            <Label htmlFor="deleteConfirm" className="text-destructive">
                                Type <strong>{project.name}</strong> to confirm
                            </Label>
                            <div className="flex gap-2 items-center">
                                <Input
                                    id="deleteConfirm"
                                    value={deleteConfirmation}
                                    onChange={(e) => setDeleteConfirmation(e.target.value)}
                                    placeholder={project.name}
                                    className="border-destructive/30 focus-visible:ring-destructive/30"
                                />
                                <Button
                                    variant="destructive"
                                    onClick={handleDelete}
                                    disabled={isDeleting || deleteConfirmation !== project.name}
                                >
                                    {isDeleting ? "Deleting..." : <><Trash2 className="mr-2 h-4 w-4" /> Delete Project</>}
                                </Button>
                            </div>
                        </div>
                    </div>

                    {/* GDPR: Purge project data */}
                    <div className="rounded-md border border-destructive/20 bg-background p-4">
                        <div className="flex items-center gap-2 mb-2">
                            <ShieldOff className="h-4 w-4 text-destructive" />
                            <h3 className="font-semibold text-destructive">Purge All Data (GDPR Erasure)</h3>
                        </div>
                        <p className="text-[13px] text-muted-foreground mb-3">
                            Permanently erase all <strong>audit logs, sessions, and usage data</strong> for this project.
                            The project itself, virtual keys, and credentials are <em>not</em> deleted.
                            This satisfies GDPR Article 17 (Right to Erasure) for data subjects.
                        </p>
                        <div className="rounded-md bg-destructive/5 border border-destructive/10 p-2.5 mb-3 text-[12px] text-amber-400 flex items-start gap-2">
                            <AlertTriangle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
                            This action is <strong>irreversible</strong>. Purged audit data cannot be recovered.
                        </div>
                        <div className="space-y-2">
                            <Label htmlFor="purgeConfirm" className="text-destructive">
                                Type <strong>PURGE {project.name}</strong> to confirm
                            </Label>
                            <div className="flex gap-2 items-center">
                                <Input
                                    id="purgeConfirm"
                                    value={purgeConfirmation}
                                    onChange={(e) => setPurgeConfirmation(e.target.value)}
                                    placeholder={`PURGE ${project.name}`}
                                    className="border-destructive/30 focus-visible:ring-destructive/30 font-mono"
                                />
                                <Button
                                    variant="destructive"
                                    onClick={handlePurge}
                                    disabled={isPurging || purgeConfirmation !== `PURGE ${project.name}`}
                                >
                                    {isPurging ? "Purging..." : <><ShieldOff className="mr-2 h-4 w-4" /> Purge Data</>}
                                </Button>
                            </div>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
