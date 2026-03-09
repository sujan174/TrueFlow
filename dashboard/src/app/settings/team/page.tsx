"use client";

import { useState } from "react";
import useSWR from "swr";
import {
    listTeams,
    createTeam,
    updateTeam,
    deleteTeam,
    listTeamMembers,
    addTeamMember,
    removeTeamMember,
    getTeamSpend,
    Team,
    TeamMember,
    CreateTeamRequest,
    swrFetcher,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import {
    Plus, Trash2, Users, ChevronRight, ChevronDown,
    DollarSign, UserMinus, Loader2, Pencil, Check, X
} from "lucide-react";
import { toast } from "sonner";
import { formatDistanceToNow } from "date-fns";
import { cn } from "@/lib/utils";

export default function TeamsPage() {
    const { data: teams = [], mutate, isLoading } = useSWR<Team[]>(
        "/teams", () => listTeams(), { refreshInterval: 15000 }
    );
    const [expandedTeam, setExpandedTeam] = useState<string | null>(null);
    const [createOpen, setCreateOpen] = useState(false);
    const [newName, setNewName] = useState("");
    const [newDescription, setNewDescription] = useState("");
    const [newBudget, setNewBudget] = useState("");
    const [newBudgetDuration, setNewBudgetDuration] = useState("monthly");
    const [newAllowedModels, setNewAllowedModels] = useState("");
    const [newTags, setNewTags] = useState("");
    const [creating, setCreating] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editName, setEditName] = useState("");

    const handleCreate = async () => {
        if (!newName.trim()) return;
        setCreating(true);
        try {
            const payload: CreateTeamRequest = { name: newName.trim() };
            if (newDescription.trim()) payload.description = newDescription.trim();
            if (newBudget && !isNaN(parseFloat(newBudget))) {
                payload.max_budget_usd = parseFloat(newBudget);
                payload.budget_duration = newBudgetDuration;
            }
            if (newAllowedModels.trim()) {
                payload.allowed_models = newAllowedModels.split(",").map(m => m.trim()).filter(Boolean);
            }
            if (newTags.trim()) {
                try { payload.tags = JSON.parse(newTags); } catch { toast.error("Tags must be valid JSON"); return; }
            }
            await createTeam(payload);
            mutate();
            setCreateOpen(false);
            setNewName(""); setNewDescription(""); setNewBudget(""); setNewAllowedModels(""); setNewTags("");
            toast.success(`Team "${payload.name}" created`);
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to create team");
        } finally {
            setCreating(false);
        }
    };

    const handleRename = async (id: string) => {
        if (!editName.trim()) return;
        try {
            await updateTeam(id, editName.trim());
            mutate();
            setEditingId(null);
            toast.success("Team renamed");
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to rename");
        }
    };

    const handleDelete = async (id: string, name: string) => {
        if (!confirm(`Delete team "${name}"? Members will be unassigned.`)) return;
        try {
            await deleteTeam(id);
            mutate();
            if (expandedTeam === id) setExpandedTeam(null);
            toast.success(`Team "${name}" deleted`);
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to delete team");
        }
    };

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
                    Organize API keys and tokens into teams for grouped spend tracking and access control.
                </p>
                <Dialog open={createOpen} onOpenChange={setCreateOpen}>
                    <DialogTrigger asChild>
                        <Button size="sm" className="gap-2 shrink-0 ml-4">
                            <Plus className="h-3.5 w-3.5" /> Create Team
                        </Button>
                    </DialogTrigger>
                    <DialogContent className="sm:max-w-[460px]">
                        <DialogHeader>
                            <DialogTitle>Create Team</DialogTitle>
                        </DialogHeader>
                        <div className="space-y-4 pt-2">
                            <div className="space-y-2">
                                <Label>Team Name <span className="text-destructive">*</span></Label>
                                <Input
                                    placeholder="e.g. Platform Engineering"
                                    value={newName}
                                    onChange={e => setNewName(e.target.value)}
                                    onKeyDown={e => e.key === "Enter" && handleCreate()}
                                />
                            </div>
                            <div className="space-y-2">
                                <Label>Description</Label>
                                <Input
                                    placeholder="What does this team work on?"
                                    value={newDescription}
                                    onChange={e => setNewDescription(e.target.value)}
                                />
                            </div>
                            <div className="grid grid-cols-2 gap-3">
                                <div className="space-y-2">
                                    <Label>Budget (USD)</Label>
                                    <Input
                                        type="number" min="0" step="1"
                                        placeholder="500"
                                        value={newBudget}
                                        onChange={e => setNewBudget(e.target.value)}
                                    />
                                </div>
                                <div className="space-y-2">
                                    <Label>Period</Label>
                                    <Select value={newBudgetDuration} onValueChange={setNewBudgetDuration}>
                                        <SelectTrigger><SelectValue /></SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="daily">Daily</SelectItem>
                                            <SelectItem value="monthly">Monthly</SelectItem>
                                            <SelectItem value="yearly">Yearly</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>
                            </div>
                            <div className="space-y-2">
                                <Label>Allowed Models (optional)</Label>
                                <Input
                                    placeholder="gpt-4o, claude-3-*, gemini-*"
                                    value={newAllowedModels}
                                    onChange={e => setNewAllowedModels(e.target.value)}
                                />
                                <p className="text-[10px] text-muted-foreground">Comma-separated. Leave blank to allow all.</p>
                            </div>
                            <div className="space-y-2">
                                <Label>Tags (optional)</Label>
                                <Input
                                    placeholder='{"cost_center": "CC-42"}'
                                    value={newTags}
                                    onChange={e => setNewTags(e.target.value)}
                                />
                            </div>
                            <Button onClick={handleCreate} disabled={creating || !newName.trim()} className="w-full">
                                {creating ? <><Loader2 className="h-4 w-4 mr-2 animate-spin" />Creating…</> : "Create Team"}
                            </Button>
                        </div>
                    </DialogContent>
                </Dialog>
            </div>

            {teams.length === 0 ? (
                <Card>
                    <CardContent className="py-16 text-center">
                        <Users className="h-10 w-10 mx-auto text-muted-foreground/20 mb-4" />
                        <h3 className="text-base font-medium">No teams yet</h3>
                        <p className="text-xs text-muted-foreground mt-1">
                            Create a team to organize members and track spend by group.
                        </p>
                    </CardContent>
                </Card>
            ) : (
                <div className="space-y-2">
                    {teams.map(team => (
                        <TeamCard
                            key={team.id}
                            team={team}
                            isExpanded={expandedTeam === team.id}
                            onToggle={() => setExpandedTeam(expandedTeam === team.id ? null : team.id)}
                            onDelete={handleDelete}
                            isEditing={editingId === team.id}
                            editName={editName}
                            onEditStart={() => { setEditingId(team.id); setEditName(team.name); }}
                            onEditChange={setEditName}
                            onEditSave={() => handleRename(team.id)}
                            onEditCancel={() => setEditingId(null)}
                        />
                    ))}
                </div>
            )}
        </div>
    );
}

function TeamCard({
    team, isExpanded, onToggle, onDelete,
    isEditing, editName, onEditStart, onEditChange, onEditSave, onEditCancel
}: {
    team: Team;
    isExpanded: boolean;
    onToggle: () => void;
    onDelete: (id: string, name: string) => void;
    isEditing: boolean;
    editName: string;
    onEditStart: () => void;
    onEditChange: (v: string) => void;
    onEditSave: () => void;
    onEditCancel: () => void;
}) {
    const { data: members = [], mutate: mutateMembers, isLoading: loadingMembers } = useSWR<TeamMember[]>(
        isExpanded ? `/teams/${team.id}/members` : null,
        () => listTeamMembers(team.id)
    );
    const { data: spend } = useSWR(
        isExpanded ? `/teams/${team.id}/spend` : null,
        () => getTeamSpend(team.id).catch(() => null)
    );

    const [addOpen, setAddOpen] = useState(false);
    const [newUserId, setNewUserId] = useState("");
    const [newRole, setNewRole] = useState("member");
    const [adding, setAdding] = useState(false);

    const handleAddMember = async () => {
        if (!newUserId.trim()) return;
        setAdding(true);
        try {
            await addTeamMember(team.id, newUserId.trim(), newRole);
            mutateMembers();
            setAddOpen(false);
            setNewUserId("");
            toast.success("Member added");
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to add member");
        } finally {
            setAdding(false);
        }
    };

    const handleRemoveMember = async (userId: string) => {
        try {
            await removeTeamMember(team.id, userId);
            mutateMembers();
            toast.success("Member removed");
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(err.message || "Failed to remove");
        }
    };

    return (
        <Card className="group">
            <div
                className="px-4 py-3 flex items-center gap-3 cursor-pointer hover:bg-card/80 transition-colors"
                onClick={onToggle}
            >
                {isExpanded
                    ? <ChevronDown className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                    : <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                }
                <div className="flex h-7 w-7 items-center justify-center rounded-md bg-violet-500/10 shrink-0">
                    <Users className="h-3.5 w-3.5 text-violet-500" />
                </div>

                <div className="flex-1 min-w-0">
                    {isEditing ? (
                        <div className="flex items-center gap-2" onClick={e => e.stopPropagation()}>
                            <Input
                                value={editName}
                                onChange={e => onEditChange(e.target.value)}
                                className="h-7 text-sm"
                                onKeyDown={e => { if (e.key === "Enter") onEditSave(); if (e.key === "Escape") onEditCancel(); }}
                                autoFocus
                            />
                            <Button size="icon" variant="ghost" className="h-7 w-7" onClick={onEditSave}><Check className="h-3.5 w-3.5" /></Button>
                            <Button size="icon" variant="ghost" className="h-7 w-7" onClick={onEditCancel}><X className="h-3.5 w-3.5" /></Button>
                        </div>
                    ) : (
                        <div className="flex items-center gap-2">
                            <span className="font-medium text-sm">{team.name}</span>
                            {team.description && (
                                <span className="text-[11px] text-muted-foreground truncate max-w-[200px]">{team.description}</span>
                            )}
                            {team.max_budget_usd && (
                                <Badge variant="outline" className="text-[9px] h-4 font-mono">
                                    ${parseFloat(team.max_budget_usd).toLocaleString()}/{team.budget_duration || "mo"}
                                </Badge>
                            )}
                            {team.allowed_models && team.allowed_models.length > 0 && (
                                <Badge variant="secondary" className="text-[9px] h-4">
                                    {team.allowed_models.length} model{team.allowed_models.length !== 1 ? "s" : ""} allowed
                                </Badge>
                            )}
                            <span className="text-[10px] text-muted-foreground/60">
                                Created {formatDistanceToNow(new Date(team.created_at), { addSuffix: true })}
                            </span>
                        </div>
                    )}
                </div>

                {spend && (
                    <div className="flex items-center gap-1 text-[11px] text-muted-foreground">
                        <DollarSign className="h-3 w-3" />
                        <span className="font-mono">${(spend.total_cost_usd ?? 0).toFixed(4)}</span>
                        <span className="text-muted-foreground/50">/ {spend.total_requests} reqs</span>
                    </div>
                )}

                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity" onClick={e => e.stopPropagation()}>
                    <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onEditStart}>
                        <Pencil className="h-3 w-3" />
                    </Button>
                    <Button variant="ghost" size="icon" className="h-7 w-7 text-destructive" onClick={() => onDelete(team.id, team.name)}>
                        <Trash2 className="h-3 w-3" />
                    </Button>
                </div>
            </div>

            {isExpanded && (
                <div className="px-4 pb-4 border-t border-border pt-3 space-y-3">
                    <div className="flex items-center justify-between">
                        <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">Members</span>
                        <Dialog open={addOpen} onOpenChange={setAddOpen}>
                            <DialogTrigger asChild>
                                <Button variant="outline" size="sm" className="h-7 gap-1 text-[11px]">
                                    <Plus className="h-3 w-3" /> Add Member
                                </Button>
                            </DialogTrigger>
                            <DialogContent className="sm:max-w-[380px]">
                                <DialogHeader>
                                    <DialogTitle>Add Team Member</DialogTitle>
                                </DialogHeader>
                                <div className="space-y-4 pt-2">
                                    <div className="space-y-2">
                                        <Label>User ID</Label>
                                        <Input
                                            placeholder="uuid of the user"
                                            value={newUserId}
                                            onChange={e => setNewUserId(e.target.value)}
                                        />
                                        <p className="text-[10px] text-muted-foreground">Enter the user&apos;s UUID from your identity provider or API keys table.</p>
                                    </div>
                                    <div className="space-y-2">
                                        <Label>Role</Label>
                                        <Select value={newRole} onValueChange={setNewRole}>
                                            <SelectTrigger><SelectValue /></SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="admin">Admin</SelectItem>
                                                <SelectItem value="member">Member</SelectItem>
                                                <SelectItem value="viewer">Viewer</SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </div>
                                    <Button onClick={handleAddMember} disabled={adding || !newUserId.trim()} className="w-full">
                                        {adding ? <><Loader2 className="h-4 w-4 mr-2 animate-spin" /> Adding…</> : "Add to Team"}
                                    </Button>
                                </div>
                            </DialogContent>
                        </Dialog>
                    </div>

                    {loadingMembers ? (
                        <div className="flex items-center justify-center py-6">
                            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
                        </div>
                    ) : members.length === 0 ? (
                        <p className="text-xs text-muted-foreground italic py-2">No members yet. Add the first one above.</p>
                    ) : (
                        <div className="space-y-1">
                            {members.map(m => (
                                <div key={m.user_id} className="flex items-center justify-between py-2 px-2 rounded-md hover:bg-muted/40 group/m">
                                    <div className="flex items-center gap-2">
                                        <div className="h-6 w-6 rounded-full bg-muted flex items-center justify-center text-[10px] font-mono">
                                            {m.user_id.slice(0, 2).toUpperCase()}
                                        </div>
                                        <span className="text-xs font-mono">{m.user_id}</span>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        <Badge variant="secondary" className="text-[9px] h-4 capitalize">{m.role}</Badge>
                                        <Button
                                            variant="ghost"
                                            size="icon"
                                            className="h-6 w-6 opacity-0 group-hover/m:opacity-100 text-destructive transition-opacity"
                                            onClick={() => handleRemoveMember(m.user_id)}
                                        >
                                            <UserMinus className="h-3 w-3" />
                                        </Button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            )}
        </Card>
    );
}
