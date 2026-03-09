"use client";

import { useState } from "react";
import useSWR from "swr";
import { useRouter } from "next/navigation";
import {
    listPrompts,
    createPrompt,
    deletePrompt,
    Prompt,
    CreatePromptRequest,
    swrFetcher,
} from "@/lib/api";
import {
    Plus,
    RefreshCw,
    MessageSquareText,
    FolderOpen,
    Trash2,
    Search,
    GitBranch,
    Tag,
    Loader2,
    Copy,
    AlertTriangle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { CountUp } from "@/components/ui/count-up";
import { EmptyState } from "@/components/empty-state";
import { PageSkeleton } from "@/components/page-skeleton";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
    DialogClose,
} from "@/components/ui/dialog";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

export default function PromptsPage() {
    const router = useRouter();
    const {
        data: prompts = [],
        mutate: mutatePrompts,
        isLoading: loading,
    } = useSWR<Prompt[]>("/prompts", swrFetcher);

    const [createOpen, setCreateOpen] = useState(false);
    const [deleteTarget, setDeleteTarget] = useState<Prompt | null>(null);
    const [searchQuery, setSearchQuery] = useState("");
    const [selectedFolder, setSelectedFolder] = useState<string | null>(null);

    // Derive folders from prompts
    const folders = Array.from(new Set(prompts.map((p) => p.folder))).sort();

    // Filter prompts
    const filtered = prompts.filter((p) => {
        if (selectedFolder && p.folder !== selectedFolder) return false;
        if (searchQuery) {
            const q = searchQuery.toLowerCase();
            return (
                p.name.toLowerCase().includes(q) ||
                p.slug.toLowerCase().includes(q) ||
                p.description.toLowerCase().includes(q)
            );
        }
        return true;
    });

    const handleCreate = async (data: CreatePromptRequest) => {
        await createPrompt(data);
        mutatePrompts();
        setCreateOpen(false);
        toast.success("Prompt created");
    };

    const handleDelete = async () => {
        if (!deleteTarget) return;
        try {
            await deletePrompt(deleteTarget.id);
            mutatePrompts();
            toast.success("Prompt deleted");
            setDeleteTarget(null);
        } catch {
            toast.error("Failed to delete prompt");
        }
    };

    return (
        <div className="space-y-4">
            {/* Header */}
            <div className="flex items-center justify-between animate-fade-in">
                <div>
                    <h1 className="text-lg font-semibold tracking-tight text-white">Prompts</h1>
                    <p className="text-xs text-zinc-500 mt-0.5">
                        Versioned prompt templates with deployment labels and a render API.
                    </p>
                </div>
                <div className="flex items-center gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => mutatePrompts()}
                        disabled={loading}
                        className="bg-black border-white/10 text-zinc-400 hover:text-white hover:bg-white/5"
                    >
                        <RefreshCw
                            className={cn("h-3.5 w-3.5 mr-1.5", loading && "animate-spin")}
                        />
                        Refresh
                    </Button>
                    <Dialog open={createOpen} onOpenChange={setCreateOpen}>
                        <DialogTrigger asChild>
                            <Button size="sm" className="bg-white text-black hover:bg-zinc-200">
                                <Plus className="mr-1.5 h-3.5 w-3.5" /> New Prompt
                            </Button>
                        </DialogTrigger>
                        <DialogContent className="sm:max-w-[440px] bg-zinc-950 border-white/10 p-0">
                            <div className="p-6">
                                <CreatePromptForm
                                    onSubmit={handleCreate}
                                    onCancel={() => setCreateOpen(false)}
                                />
                            </div>
                        </DialogContent>
                    </Dialog>
                </div>
            </div>

            {/* KPI Cards */}
            <div className="grid gap-4 md:grid-cols-3 animate-slide-up">
                <Card className="bg-black border-white/10 hover:border-white/20 transition-colors p-4 relative overflow-hidden group">
                    <div className="flex items-center gap-3">
                        <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-blue-500/20 bg-blue-500/10 text-blue-400 group-hover:bg-blue-500/20 transition-colors">
                            <MessageSquareText className="h-4 w-4" />
                        </div>
                        <div>
                            <div className="text-xl font-semibold tabular-nums text-white">
                                <CountUp value={prompts.length} />
                            </div>
                            <p className="text-[11px] font-medium uppercase tracking-widest text-zinc-500">Total Prompts</p>
                        </div>
                    </div>
                </Card>
                <Card className="bg-black border-white/10 hover:border-white/20 transition-colors p-4 relative overflow-hidden group">
                    <div className="flex items-center gap-3">
                        <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-emerald-500/20 bg-emerald-500/10 text-emerald-400 group-hover:bg-emerald-500/20 transition-colors">
                            <GitBranch className="h-4 w-4" />
                        </div>
                        <div>
                            <div className="text-xl font-semibold tabular-nums text-emerald-400">
                                <CountUp value={prompts.reduce((acc, p) => acc + (p.version_count || 0), 0)} />
                            </div>
                            <p className="text-[11px] font-medium uppercase tracking-widest text-zinc-500">Total Versions</p>
                        </div>
                    </div>
                </Card>
                <Card className="bg-black border-white/10 hover:border-white/20 transition-colors p-4 relative overflow-hidden group">
                    <div className="flex items-center gap-3">
                        <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-violet-500/20 bg-violet-500/10 text-violet-400 group-hover:bg-violet-500/20 transition-colors">
                            <FolderOpen className="h-4 w-4" />
                        </div>
                        <div>
                            <div className="text-xl font-semibold tabular-nums text-violet-400">
                                <CountUp value={folders.length} />
                            </div>
                            <p className="text-[11px] font-medium uppercase tracking-widest text-zinc-500">Folders</p>
                        </div>
                    </div>
                </Card>
            </div>

            {/* Search + Folder filter */}
            <div className="flex gap-3 animate-slide-up stagger-1">
                <div className="relative flex-1 max-w-sm">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-zinc-500" />
                    <Input
                        placeholder="Search prompts..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="pl-9 h-9"
                    />
                </div>
                {folders.length > 1 && (
                    <div className="flex gap-2">
                        <button
                            onClick={() => setSelectedFolder(null)}
                            className={cn(
                                "px-3 py-1 rounded-md text-xs font-medium transition-all border",
                                !selectedFolder
                                    ? "border-white/10 bg-white/5 text-white"
                                    : "border-transparent text-zinc-500 hover:text-white hover:bg-white/5"
                            )}
                        >
                            All
                        </button>
                        {folders.map((f) => (
                            <button
                                key={f}
                                onClick={() =>
                                    setSelectedFolder(selectedFolder === f ? null : f)
                                }
                                className={cn(
                                    "px-3 py-1 rounded-md text-xs font-medium transition-all border",
                                    selectedFolder === f
                                        ? "border-white/10 bg-white/5 text-white"
                                        : "border-transparent text-zinc-500 hover:text-white hover:bg-white/5"
                                )}
                            >
                                {f === "/" ? "Root" : f}
                            </button>
                        ))}
                    </div>
                )}
            </div>

            {/* Prompt Grid */}
            {loading ? (
                <PageSkeleton cards={3} rows={5} />
            ) : filtered.length === 0 ? (
                <EmptyState
                    icon={MessageSquareText}
                    title={searchQuery ? "No matching prompts" : "No prompts yet"}
                    description="Create a prompt template to manage versions, deploy labels, and use the render API."
                    actionLabel="New Prompt"
                    onAction={() => setCreateOpen(true)}
                    className="bg-black/50"
                />
            ) : (
                <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3 animate-slide-up stagger-2">
                    {filtered.map((p) => (
                        <Card
                            key={p.id}
                            className="bg-black border-white/10 hover:border-white/20 transition-all p-4 cursor-pointer group"
                            onClick={() => router.push(`/prompts/${p.id}`)}
                        >
                            <div className="flex items-start justify-between mb-3">
                                <div className="min-w-0 flex-1">
                                    <h3 className="font-semibold text-[13px] text-white truncate group-hover:text-zinc-200 transition-colors">
                                        {p.name}
                                    </h3>
                                    <p className="text-[11px] font-mono text-zinc-500 truncate mt-0.5">
                                        {p.slug}
                                    </p>
                                </div>
                                <button
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        setDeleteTarget(p);
                                    }}
                                    className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-rose-400 transition-all p-1"
                                >
                                    <Trash2 className="h-3.5 w-3.5" />
                                </button>
                            </div>

                            {p.description && (
                                <p className="text-xs text-zinc-400 mb-3 line-clamp-2">
                                    {p.description}
                                </p>
                            )}

                            <div className="flex items-center gap-2 flex-wrap">
                                {p.latest_model && (
                                    <Badge
                                        variant="secondary"
                                        className="text-[10px] font-mono px-1.5 h-5 bg-white/[0.03] border-white/5"
                                    >
                                        {p.latest_model}
                                    </Badge>
                                )}
                                <Badge
                                    variant="outline"
                                    className="text-[10px] px-1.5 h-5 tabular-nums border-white/10 text-zinc-400"
                                >
                                    <GitBranch className="h-2.5 w-2.5 mr-1" />v
                                    {p.latest_version || 0}
                                </Badge>
                                {p.folder !== "/" && (
                                    <Badge
                                        variant="outline"
                                        className="text-[10px] px-1.5 h-5 border-white/10 text-zinc-400"
                                    >
                                        <FolderOpen className="h-2.5 w-2.5 mr-1" />
                                        {p.folder}
                                    </Badge>
                                )}
                                {(p.labels as unknown as string[])?.map((l: string) => (
                                    <Badge
                                        key={l}
                                        className={cn(
                                            "text-[10px] px-1.5 h-5",
                                            l === "production"
                                                ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/20"
                                                : l === "staging"
                                                    ? "bg-amber-500/10 text-amber-400 border-amber-500/20"
                                                    : "bg-blue-500/10 text-blue-400 border-blue-500/20"
                                        )}
                                    >
                                        <Tag className="h-2.5 w-2.5 mr-0.5" />
                                        {l}
                                    </Badge>
                                ))}
                            </div>

                            <div className="mt-3 pt-3 border-t border-white/5 flex items-center justify-between">
                                <span className="text-[10px] text-zinc-500 font-mono">
                                    UPDATED{" "}
                                    {new Date(p.updated_at).toLocaleDateString("en-US", {
                                        month: "short",
                                        day: "numeric",
                                    }).toUpperCase()}
                                </span>
                                <button
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        navigator.clipboard.writeText(
                                            `curl /api/v1/prompts/by-slug/${p.slug}/render?label=production`
                                        );
                                        toast.success("API snippet copied");
                                    }}
                                    className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-white transition-all p-1"
                                    title="Copy API snippet"
                                >
                                    <Copy className="h-3 w-3" />
                                </button>
                            </div>
                        </Card>
                    ))}
                </div>
            )}

            {/* Delete Confirmation */}
            <Dialog
                open={!!deleteTarget}
                onOpenChange={(open) => !open && setDeleteTarget(null)}
            >
                <DialogContent className="bg-zinc-950 border-white/10">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2 text-rose-400">
                            <AlertTriangle className="h-5 w-5" /> Delete Prompt
                        </DialogTitle>
                        <DialogDescription className="text-zinc-500">
                            Delete{" "}
                            <span className="font-mono font-medium text-white">
                                {deleteTarget?.name}
                            </span>
                            ? All versions will be removed. This cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="ghost" className="text-zinc-400 hover:text-white hover:bg-white/5" onClick={() => setDeleteTarget(null)}>
                            Cancel
                        </Button>
                        <Button variant="destructive" onClick={handleDelete}>
                            Delete
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}

// ── Create Prompt Form ──────────────────────────────────

function CreatePromptForm({
    onSubmit,
    onCancel,
}: {
    onSubmit: (data: CreatePromptRequest) => Promise<void>;
    onCancel: () => void;
}) {
    const [loading, setLoading] = useState(false);
    const [name, setName] = useState("");
    const [slug, setSlug] = useState("");
    const [description, setDescription] = useState("");
    const [folder, setFolder] = useState("/");

    const autoSlug = name
        .toLowerCase()
        .replace(/[^a-z0-9-]/g, "-")
        .replace(/-+/g, "-")
        .replace(/^-|-$/g, "");

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        try {
            await onSubmit({
                name,
                slug: slug || autoSlug,
                description: description || undefined,
                folder: folder || "/",
            });
        } catch (err) {
            toast.error(
                err instanceof Error ? err.message : "Failed to create prompt"
            );
        } finally {
            setLoading(false);
        }
    };

    return (
        <form onSubmit={handleSubmit}>
            <DialogHeader>
                <DialogTitle className="text-white">New Prompt</DialogTitle>
                <DialogDescription className="text-zinc-500">
                    Create a versioned prompt template.
                </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
                <div className="space-y-1.5">
                    <Label htmlFor="pname" className="text-xs text-zinc-400">
                        Name
                    </Label>
                    <Input
                        id="pname"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        placeholder="e.g. Customer Support Agent"
                        className="bg-black"
                        required
                    />
                </div>
                <div className="space-y-1.5">
                    <Label htmlFor="pslug" className="text-xs text-zinc-400">
                        Slug
                    </Label>
                    <Input
                        id="pslug"
                        value={slug}
                        onChange={(e) => setSlug(e.target.value)}
                        placeholder={autoSlug || "auto-generated-from-name"}
                        className="font-mono text-sm bg-black text-zinc-300"
                    />
                    <p className="text-[10px] text-zinc-500">
                        URL-safe key for the render API: /prompts/by-slug/
                        {slug || autoSlug || "..."}/render
                    </p>
                </div>
                <div className="space-y-1.5">
                    <Label htmlFor="pdesc" className="text-xs text-zinc-400">
                        Description (optional)
                    </Label>
                    <Input
                        id="pdesc"
                        value={description}
                        onChange={(e) => setDescription(e.target.value)}
                        placeholder="What does this prompt do?"
                        className="bg-black"
                    />
                </div>
                <div className="space-y-1.5">
                    <Label htmlFor="pfolder" className="text-xs text-zinc-400">
                        Folder
                    </Label>
                    <Input
                        id="pfolder"
                        value={folder}
                        onChange={(e) => setFolder(e.target.value)}
                        placeholder="/"
                        className="bg-black"
                    />
                    <p className="text-[10px] text-zinc-500">
                        Organize prompts into folders, e.g. /agents/support
                    </p>
                </div>
            </div>
            <DialogFooter className="border-t border-white/10 pt-4 mt-2">
                <DialogClose asChild>
                    <Button variant="ghost" type="button" onClick={onCancel} className="text-zinc-400 hover:text-white hover:bg-white/5">
                        Cancel
                    </Button>
                </DialogClose>
                <Button type="submit" disabled={loading || !name.trim()} className="bg-white text-black hover:bg-zinc-200">
                    {loading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                    {loading ? "Creating..." : "Create Prompt"}
                </Button>
            </DialogFooter>
        </form>
    );
}
