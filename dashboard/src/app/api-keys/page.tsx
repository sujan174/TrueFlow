"use client";

import { useState, useEffect, useCallback } from "react";
import {
    listApiKeys,
    createApiKey,
    revokeApiKey,
    ApiKey,
    CreateApiKeyRequest,
    CreateApiKeyResponse
} from "@/lib/api";
import {
    Plus, RefreshCw, Key, Trash2, Loader2, Copy
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/empty-state";
import { PageSkeleton } from "@/components/page-skeleton";
import { useRouter } from "next/navigation";
import { Card } from "@/components/ui/card";
import { DataTable } from "@/components/data-table";
import { columns } from "./columns";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

export default function ApiKeysPage() {
    const router = useRouter();
    const [keys, setKeys] = useState<ApiKey[]>([]);
    const [loading, setLoading] = useState(true);
    const [createOpen, setCreateOpen] = useState(false);
    const [revokeKeyData, setRevokeKeyData] = useState<ApiKey | null>(null);
    const [createdKey, setCreatedKey] = useState<CreateApiKeyResponse | null>(null);

    const fetchKeys = useCallback(async () => {
        try {
            setLoading(true);
            const data = await listApiKeys();
            // Sort active first, then by date
            const sorted = data.sort((a, b) => {
                if (a.is_active && !b.is_active) return -1;
                if (!a.is_active && b.is_active) return 1;
                return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
            });
            setKeys(sorted);
        } catch {
            toast.error("Failed to load API keys");
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        fetchKeys();
    }, [fetchKeys]);

    const handleRevoke = async () => {
        if (!revokeKeyData) return;
        try {
            await revokeApiKey(revokeKeyData.id);
            toast.success("API Key revoked successfully");
            setRevokeKeyData(null);
            fetchKeys();
        } catch {
            toast.error("Failed to revoke API key");
        }
    };

    return (
        <div className="space-y-4">
            {/* Controls */}
            <div className="flex items-center justify-end animate-fade-in mb-2">
                <div className="flex items-center gap-2">
                    <Button variant="outline" size="sm" onClick={fetchKeys} disabled={loading}>
                        <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", loading && "animate-spin")} />
                        Refresh
                    </Button>
                    <Dialog open={createOpen} onOpenChange={(open) => {
                        if (!open) setCreatedKey(null); // Reset on close
                        setCreateOpen(open);
                    }}>
                        <DialogTrigger asChild>
                            <Button size="sm">
                                <Plus className="h-4 w-4 mr-1.5" />
                                Create Key
                            </Button>
                        </DialogTrigger>
                        <DialogContent>
                            <CreateKeyForm
                                onSuccess={(key) => {
                                    setCreatedKey(key);
                                    fetchKeys();
                                }}
                                createdKey={createdKey}
                                onClose={() => setCreateOpen(false)}
                            />
                        </DialogContent>
                    </Dialog>
                </div>
            </div>

            {loading ? (
                <PageSkeleton />
            ) : keys.length === 0 ? (
                <EmptyState
                    icon={Key}
                    title="No API Keys found"
                    description="Create an API key to access the Management API programmatically."
                    actionLabel="Create your first key"
                    onAction={() => setCreateOpen(true)}
                    className="bg-black border-white/10"
                />
            ) : (
                <div className="grid gap-3 animate-fade-in duration-500">
                    <div className="bg-black border border-white/10 rounded-lg overflow-hidden">
                        <DataTable
                            columns={columns}
                            data={keys}
                            searchKey="name"
                            meta={{ onRevoke: setRevokeKeyData }}
                        />
                    </div>
                </div>
            )}

            {/* Revoke Dialog */}
            <Dialog open={!!revokeKeyData} onOpenChange={(open) => !open && setRevokeKeyData(null)}>
                <DialogContent className="bg-zinc-950 border-rose-500/20 text-white">
                    <DialogHeader>
                        <DialogTitle className="text-rose-500 font-medium">Revoke API Key</DialogTitle>
                        <DialogDescription className="pt-1 text-zinc-400 text-[13px]">
                            Are you sure you want to revoke <span className="font-mono font-medium text-white">{revokeKeyData?.name}</span>?
                            This action cannot be undone and any applications using this key will immediately lose access.
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="ghost" onClick={() => setRevokeKeyData(null)} className="text-zinc-400 hover:text-white hover:bg-white/5">Cancel</Button>
                        <Button variant="destructive" onClick={handleRevoke}>
                            Revoke Key
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}

function CreateKeyForm({ onSuccess, createdKey, onClose }: { onSuccess: (k: CreateApiKeyResponse) => void, createdKey: CreateApiKeyResponse | null, onClose: () => void }) {
    const [name, setName] = useState("");
    const [role, setRole] = useState("member");
    const [scopes, setScopes] = useState<string[]>([]);
    const [isSubmitting, setIsSubmitting] = useState(false);

    const availableScopes = [
        "tokens:read", "tokens:write",
        "policies:read", "policies:write",
        "credentials:read", "credentials:write",
        "approvals:read", "approvals:write",
        "audit:read",
        // "keys:manage" // Only admins can grant this, assume UI hides it or handles it based on current user role?
        // simple UI for now
    ];

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            setIsSubmitting(true);
            const res = await createApiKey({
                name,
                role,
                scopes: role === "admin" ? [] : scopes, // admins get all scopes implicitly
            });
            onSuccess(res);
            toast.success("API Key created");
        } catch (err) {
            toast.error("Failed to create key");
        } finally {
            setIsSubmitting(false);
        }
    };

    if (createdKey) {
        return (
            <div className="space-y-4">
                <DialogHeader>
                    <DialogTitle className="text-white">API Key Created</DialogTitle>
                    <DialogDescription className="text-zinc-500">
                        Please copy your API key now. It will not be shown again.
                    </DialogDescription>
                </DialogHeader>
                <div className="p-4 bg-black border border-white/10 rounded-md break-all font-mono text-sm relative group text-zinc-300">
                    {createdKey.key}
                    <Button
                        variant="ghost"
                        size="icon"
                        className="absolute top-2 right-2 h-6 w-6 opacity-0 group-hover:opacity-100 transition-opacity hover:bg-white/10 text-zinc-400 hover:text-white"
                        onClick={() => {
                            navigator.clipboard.writeText(createdKey.key);
                            toast.success("Copied to clipboard");
                        }}
                    >
                        <Copy className="h-3 w-3" />
                    </Button>
                </div>
                <DialogFooter className="border-t border-white/10 pt-4">
                    <Button onClick={onClose} className="bg-white text-black hover:bg-zinc-200">Done</Button>
                </DialogFooter>
            </div>
        );
    }

    return (
        <form onSubmit={handleSubmit} className="space-y-4">
            <DialogHeader>
                <DialogTitle className="text-white">Create API Key</DialogTitle>
                <DialogDescription className="text-zinc-500">Create a scoped API key for management access.</DialogDescription>
            </DialogHeader>
            <div className="space-y-4 py-2">
                <div className="space-y-2">
                    <Label htmlFor="name" className="text-zinc-400 text-xs uppercase tracking-widest">Name</Label>
                    <Input id="name" placeholder="e.g. CI/CD Pipeline" value={name} onChange={e => setName(e.target.value)} className="bg-black border-white/10 text-white placeholder:text-zinc-600 focus-visible:ring-white/20" required />
                </div>
                <div className="space-y-2">
                    <Label htmlFor="role" className="text-zinc-400 text-xs uppercase tracking-widest">Role</Label>
                    <select
                        id="role"
                        value={role}
                        onChange={(e) => setRole(e.target.value)}
                        className="flex h-10 w-full items-center justify-between rounded-md border border-white/10 bg-black px-3 py-2 text-sm ring-offset-background placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-white/20 text-white disabled:cursor-not-allowed disabled:opacity-50 [&>span]:line-clamp-1"
                    >
                        <option value="readonly">Read Only</option>
                        <option value="member">Member</option>
                        <option value="admin">Admin</option>
                    </select>
                    <p className="text-[10px] text-zinc-500">
                        {role === "admin" ? "Admins have full access." :
                            role === "readonly" ? "Read-only access to all resources." :
                                "Can manage resources but not keys/users."}
                    </p>
                </div>
                {role !== "admin" && role !== "readonly" && (
                    <div className="space-y-2">
                        <Label className="text-zinc-400 text-xs uppercase tracking-widest">Scopes</Label>
                        <div className="grid grid-cols-2 gap-2 text-sm bg-black border border-white/10 p-3 rounded-md">
                            {availableScopes.map(scope => (
                                <div key={scope} className="flex items-center space-x-2">
                                    <input
                                        type="checkbox"
                                        id={scope}
                                        className="h-4 w-4 rounded border-white/20 bg-black text-white focus:ring-white/20"
                                        checked={scopes.includes(scope)}
                                        onChange={(e) => {
                                            if (e.target.checked) setScopes([...scopes, scope]);
                                            else setScopes(scopes.filter(s => s !== scope));
                                        }}
                                    />
                                    <label htmlFor={scope} className="text-[11px] text-zinc-400 leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70">
                                        {scope}
                                    </label>
                                </div>
                            ))}
                        </div>
                    </div>
                )}
            </div>
            <DialogFooter className="border-t border-white/10 pt-4">
                <Button type="button" variant="ghost" className="text-zinc-400 hover:text-white hover:bg-white/5" onClick={onClose}>Cancel</Button>
                <Button type="submit" disabled={isSubmitting || !name} className="bg-white text-black hover:bg-zinc-200">
                    {isSubmitting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                    Create Key
                </Button>
            </DialogFooter>
        </form>
    );
}
