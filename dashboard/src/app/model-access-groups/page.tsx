"use client";

import { useState } from "react";
import useSWR from "swr";
import {
    listModelAccessGroups,
    createModelAccessGroup,
    updateModelAccessGroup,
    deleteModelAccessGroup,
    ModelAccessGroup,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Plus, Trash2, ShieldCheck, X, Pencil, Loader2, Check } from "lucide-react";
import { toast } from "sonner";

// Common model suggestions — keep in sync with pricing-tab.tsx
const POPULAR_MODELS = [
    "gpt-4.1",
    "gpt-4.1-mini",
    "gpt-4.1-nano",
    "gpt-4o",
    "gpt-4o-mini",
    "o4-mini",
    "o3",
    "o3-mini",
    "claude-sonnet-4-20250514",
    "claude-opus-4-20250514",
    "claude-3-7-sonnet-20250219",
    "claude-3-5-haiku-20241022",
    "gemini-2.5-pro-preview-06-05",
    "gemini-2.5-flash-preview-05-20",
    "gemini-2.0-flash",
    "deepseek-chat",
    "deepseek-reasoner",
    "llama-4-maverick-17b-128e",
    "llama-4-scout-17b-16e",
    "mistral-large-latest",
];

export default function ModelAccessGroupsPage() {
    const { data: groups = [], mutate, isLoading } = useSWR<ModelAccessGroup[]>(
        "/model-access-groups",
        () => listModelAccessGroups(),
        { refreshInterval: 30000 }
    );
    const [createOpen, setCreateOpen] = useState(false);
    const [editGroup, setEditGroup] = useState<ModelAccessGroup | null>(null);
    const [name, setName] = useState("");
    const [models, setModels] = useState<string[]>([]);
    const [modelInput, setModelInput] = useState("");
    const [saving, setSaving] = useState(false);

    const resetForm = () => { setName(""); setModels([]); setModelInput(""); };

    const openCreate = () => { resetForm(); setCreateOpen(true); };

    const openEdit = (g: ModelAccessGroup) => {
        setEditGroup(g);
        setName(g.name);
        setModels([...g.allowed_models]);
        setModelInput("");
    };

    const addModel = (m: string) => {
        const trimmed = m.trim();
        if (!trimmed || models.includes(trimmed)) return;
        setModels(prev => [...prev, trimmed]);
        setModelInput("");
    };

    const removeModel = (m: string) => setModels(prev => prev.filter(x => x !== m));

    const handleCreate = async () => {
        if (!name.trim() || models.length === 0) {
            toast.error("Name and at least one model are required");
            return;
        }
        setSaving(true);
        try {
            await createModelAccessGroup({ name: name.trim(), allowed_models: models });
            mutate();
            setCreateOpen(false);
            resetForm();
            toast.success(`Group "${name}" created`);
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to create");
        } finally {
            setSaving(false);
        }
    };

    const handleUpdate = async () => {
        if (!editGroup || !name.trim() || models.length === 0) {
            toast.error("Name and at least one model required");
            return;
        }
        setSaving(true);
        try {
            await updateModelAccessGroup(editGroup.id, { name: name.trim(), allowed_models: models });
            mutate();
            setEditGroup(null);
            toast.success("Group updated");
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to update");
        } finally {
            setSaving(false);
        }
    };

    const handleDelete = async (g: ModelAccessGroup) => {
        if (!confirm(`Delete group "${g.name}"? Tokens using this group will lose model restrictions.`)) return;
        try {
            await deleteModelAccessGroup(g.id);
            mutate();
            toast.success(`Deleted "${g.name}"`);
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to delete");
        }
    };

    const ModelForm = () => (
        <div className="space-y-4 pt-2">
            <div className="space-y-2">
                <Label>Group Name</Label>
                <Input
                    placeholder="e.g. gpt4-only, premium-models"
                    value={name}
                    onChange={e => setName(e.target.value)}
                />
            </div>
            <div className="space-y-2">
                <Label>Allowed Models</Label>
                <div className="flex gap-2">
                    <Input
                        placeholder="model name or glob (e.g. gpt-4*)"
                        value={modelInput}
                        onChange={e => setModelInput(e.target.value)}
                        onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); addModel(modelInput); } }}
                    />
                    <Button variant="outline" size="sm" onClick={() => addModel(modelInput)} className="shrink-0">
                        <Plus className="h-3.5 w-3.5" />
                    </Button>
                </div>
                {/* Quick-add popular models */}
                <div className="flex flex-wrap gap-2 mt-2">
                    {POPULAR_MODELS.map(m => (
                        <button
                            key={m}
                            onClick={() => addModel(m)}
                            disabled={models.includes(m)}
                            className="text-[10px] px-2 py-0.5 rounded border border-border/60 hover:border-primary/40 hover:bg-primary/5 transition-colors disabled:opacity-40 disabled:cursor-not-allowed font-mono"
                        >
                            {m}
                        </button>
                    ))}
                </div>
                {/* Selected models */}
                {models.length > 0 && (
                    <div className="flex flex-wrap gap-2 mt-2 p-2 rounded-md bg-muted/30 border border-border/40">
                        {models.map(m => (
                            <Badge key={m} variant="secondary" className="gap-1 text-[11px] font-mono pr-1">
                                {m}
                                <button onClick={() => removeModel(m)} className="hover:text-destructive ml-0.5">
                                    <X className="h-2.5 w-2.5" />
                                </button>
                            </Badge>
                        ))}
                    </div>
                )}
            </div>
            <Button
                onClick={editGroup ? handleUpdate : handleCreate}
                disabled={saving || !name.trim() || models.length === 0}
                className="w-full"
            >
                {saving ? (
                    <><Loader2 className="h-4 w-4 mr-2 animate-spin" /> Saving…</>
                ) : editGroup ? "Update Group" : "Create Group"}
            </Button>
        </div>
    );

    if (isLoading) {
        return (
            <div className="flex items-center justify-center py-32">
                <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
            </div>
        );
    }

    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <p className="text-xs text-muted-foreground">
                    Restrict which LLM models a token or team can access. Assign groups to tokens via the virtual key settings.
                </p>
                <Dialog open={createOpen} onOpenChange={v => { setCreateOpen(v); if (!v) resetForm(); }}>
                    <DialogTrigger asChild>
                        <Button size="sm" className="gap-2 shrink-0 ml-4" onClick={openCreate}>
                            <Plus className="h-3.5 w-3.5" /> New Group
                        </Button>
                    </DialogTrigger>
                    <DialogContent className="sm:max-w-[480px]">
                        <DialogHeader>
                            <DialogTitle>Create Model Access Group</DialogTitle>
                        </DialogHeader>
                        <ModelForm />
                    </DialogContent>
                </Dialog>
            </div>

            {groups.length === 0 ? (
                <Card>
                    <CardContent className="py-16 text-center">
                        <ShieldCheck className="h-10 w-10 mx-auto text-muted-foreground/20 mb-4" />
                        <h3 className="text-base font-medium">No model access groups</h3>
                        <p className="text-xs text-muted-foreground mt-1 max-w-xs mx-auto">
                            Create a group to restrict which models a virtual key can call. Useful for cost control and compliance.
                        </p>
                    </CardContent>
                </Card>
            ) : (
                <div className="grid gap-3 md:grid-cols-2">
                    {groups.map(g => (
                        <Card key={g.id} className="group relative">
                            <CardContent className="p-4">
                                <div className="flex items-start justify-between gap-2 mb-3">
                                    <div className="flex items-center gap-2 min-w-0">
                                        <div className="h-7 w-7 rounded-md bg-emerald-500/10 flex items-center justify-center shrink-0">
                                            <ShieldCheck className="h-3.5 w-3.5 text-emerald-500" />
                                        </div>
                                        <div className="min-w-0">
                                            <p className="font-medium text-sm truncate">{g.name}</p>
                                            <p className="text-[10px] text-muted-foreground">{g.allowed_models.length} model{g.allowed_models.length !== 1 ? "s" : ""} allowed</p>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
                                        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => openEdit(g)}>
                                            <Pencil className="h-3 w-3" />
                                        </Button>
                                        <Button variant="ghost" size="icon" className="h-7 w-7 text-destructive" onClick={() => handleDelete(g)}>
                                            <Trash2 className="h-3 w-3" />
                                        </Button>
                                    </div>
                                </div>
                                <div className="flex flex-wrap gap-1">
                                    {g.allowed_models.map(m => (
                                        <Badge key={m} variant="outline" className="text-[10px] font-mono h-5">
                                            {m}
                                        </Badge>
                                    ))}
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}

            {/* Edit Dialog */}
            <Dialog open={!!editGroup} onOpenChange={v => { if (!v) { setEditGroup(null); resetForm(); } }}>
                <DialogContent className="sm:max-w-[480px]">
                    <DialogHeader>
                        <DialogTitle>Edit Model Access Group</DialogTitle>
                    </DialogHeader>
                    <ModelForm />
                </DialogContent>
            </Dialog>
        </div>
    );
}
