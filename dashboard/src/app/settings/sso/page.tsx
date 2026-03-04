"use client";

import { useState, useCallback } from "react";
import useSWR, { mutate } from "swr";
import {
    listOidcProviders, createOidcProvider, updateOidcProvider, deleteOidcProvider,
    OidcProvider, CreateOidcProviderRequest, swrFetcher,
} from "@/lib/api";
import {
    Shield, Plus, Trash2, RefreshCw, KeyRound, Globe, ToggleRight, ToggleLeft,
    Edit2, CheckCircle2, XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
    Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogClose,
} from "@/components/ui/dialog";
import { toast } from "sonner";
import { formatDistanceToNow } from "date-fns";
import { cn } from "@/lib/utils";

const EMPTY: OidcProvider[] = [];

function ProviderFormDialog({
    mode, initial, onDone,
}: {
    mode: "create" | "edit";
    initial?: OidcProvider;
    onDone: () => void;
}) {
    const [open, setOpen] = useState(mode === "create");
    const [issuerUrl, setIssuerUrl] = useState(initial?.issuer_url ?? "");
    const [clientId, setClientId] = useState(initial?.client_id ?? "");
    const [audience, setAudience] = useState(initial?.audience ?? "");
    const [claimsJson, setClaimsJson] = useState(
        initial?.claim_mappings ? JSON.stringify(initial.claim_mappings, null, 2) : ""
    );
    const [saving, setSaving] = useState(false);

    async function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!issuerUrl.trim() || !clientId.trim()) {
            toast.error("Issuer URL and Client ID are required");
            return;
        }
        let claim_mappings: Record<string, string> | undefined;
        if (claimsJson.trim()) {
            try { claim_mappings = JSON.parse(claimsJson); }
            catch { toast.error("Claim Mappings must be valid JSON"); return; }
        }
        const data: CreateOidcProviderRequest = {
            issuer_url: issuerUrl.trim(),
            client_id: clientId.trim(),
            audience: audience.trim() || undefined,
            claim_mappings,
        };
        setSaving(true);
        try {
            if (mode === "edit" && initial) {
                await updateOidcProvider(initial.id, data);
                toast.success("Provider updated");
            } else {
                await createOidcProvider(data);
                toast.success("Provider created");
            }
            setOpen(false);
            onDone();
        } catch {
            toast.error(mode === "edit" ? "Failed to update provider" : "Failed to create provider");
        } finally {
            setSaving(false);
        }
    }

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            {mode === "create" && (
                <Button size="sm" onClick={() => setOpen(true)}>
                    <Plus className="h-3.5 w-3.5 mr-1.5" /> Add Provider
                </Button>
            )}
            {mode === "edit" && (
                <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => setOpen(true)}>
                    <Edit2 className="h-3.5 w-3.5" />
                </Button>
            )}
            <DialogContent className="sm:max-w-[520px]">
                <DialogHeader>
                    <DialogTitle>{mode === "edit" ? "Edit OIDC Provider" : "Add OIDC Provider"}</DialogTitle>
                </DialogHeader>
                <form onSubmit={handleSubmit}>
                    <div className="space-y-4 py-3">
                        <div className="space-y-1.5">
                            <Label className="text-xs">Issuer URL <span className="text-destructive">*</span></Label>
                            <Input
                                value={issuerUrl}
                                onChange={e => setIssuerUrl(e.target.value)}
                                placeholder="https://accounts.google.com"
                                required
                            />
                            <p className="text-[11px] text-muted-foreground">
                                Must match the <code className="font-mono">iss</code> claim in inbound JWTs
                            </p>
                        </div>
                        <div className="grid grid-cols-2 gap-3">
                            <div className="space-y-1.5">
                                <Label className="text-xs">Client ID <span className="text-destructive">*</span></Label>
                                <Input
                                    value={clientId}
                                    onChange={e => setClientId(e.target.value)}
                                    placeholder="my-service-client"
                                    required
                                />
                            </div>
                            <div className="space-y-1.5">
                                <Label className="text-xs">Audience (optional)</Label>
                                <Input
                                    value={audience}
                                    onChange={e => setAudience(e.target.value)}
                                    placeholder="https://api.myapp.com"
                                />
                            </div>
                        </div>
                        <div className="space-y-1.5">
                            <Label className="text-xs">Claim Mappings (JSON, optional)</Label>
                            <textarea
                                className="flex min-h-[80px] w-full rounded-md border border-input bg-muted/30 px-3 py-2 text-xs font-mono focus:outline-none focus:ring-1 focus:ring-ring"
                                value={claimsJson}
                                onChange={e => setClaimsJson(e.target.value)}
                                placeholder={'{\n  "email": "user_email",\n  "roles.admin": "admin"\n}'}
                            />
                            <p className="text-[11px] text-muted-foreground">
                                Map JWT claims to TrueFlow roles. Keys are claim paths, values are local role names.
                            </p>
                        </div>
                    </div>
                    <DialogFooter>
                        <DialogClose asChild>
                            <Button type="button" variant="outline" size="sm">Cancel</Button>
                        </DialogClose>
                        <Button type="submit" size="sm" disabled={saving}>
                            {saving ? "Saving…" : mode === "edit" ? "Update" : "Create"}
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}

export default function SSOPage() {
    const { data, isLoading, mutate: refresh } = useSWR<OidcProvider[]>(
        "/oidc/providers", swrFetcher
    );
    const providers = data ?? EMPTY;
    const [deleting, setDeleting] = useState<string | null>(null);
    const [toggling, setToggling] = useState<string | null>(null);

    const handleDelete = useCallback(async (id: string) => {
        if (!confirm("Delete this OIDC provider? Agents using this SSO config will fall back to API key auth.")) return;
        setDeleting(id);
        try {
            await deleteOidcProvider(id);
            toast.success("Provider deleted");
            refresh();
        } catch {
            toast.error("Failed to delete provider");
        } finally {
            setDeleting(null);
        }
    }, [refresh]);

    const handleToggle = useCallback(async (p: OidcProvider) => {
        setToggling(p.id);
        try {
            await updateOidcProvider(p.id, { is_active: !p.is_active });
            toast.success(p.is_active ? "Provider disabled" : "Provider enabled");
            refresh();
        } catch {
            toast.error("Failed to update provider");
        } finally {
            setToggling(null);
        }
    }, [refresh]);

    return (
        <div className="p-4 space-y-6 max-w-[1200px] mx-auto">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <div className="flex items-center gap-2 mb-1">
                        <Shield className="h-5 w-5 text-primary" />
                        <h1 className="text-xl font-semibold">SSO / OIDC Providers</h1>
                    </div>
                    <p className="text-[13px] text-muted-foreground">
                        Configure identity providers for JWT-based authentication in agent requests
                    </p>
                </div>
                <div className="flex items-center gap-2">
                    <Button variant="outline" size="sm" onClick={() => refresh()} disabled={isLoading}>
                        <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", isLoading && "animate-spin")} />
                        Refresh
                    </Button>
                    <ProviderFormDialog mode="create" onDone={() => refresh()} />
                </div>
            </div>

            {/* How it works banner */}
            <div className="rounded-md border border-blue-500/20 bg-blue-500/5 px-4 py-3 text-sm text-blue-300/80 flex items-start gap-3">
                <KeyRound className="h-4 w-4 mt-0.5 shrink-0 text-blue-400" />
                <div>
                    <p className="font-medium text-blue-300 mb-1">JWT / OIDC Authentication</p>
                    <p className="text-[12px] leading-relaxed">
                        Agents can pass a <code className="font-mono bg-blue-500/10 px-1 rounded">Bearer &lt;JWT&gt;</code> token in their{" "}
                        <code className="font-mono bg-blue-500/10 px-1 rounded">Authorization</code> header.
                        The gateway validates the JWT against the matching provider (by <code className="font-mono bg-blue-500/10 px-1 rounded">iss</code> claim),
                        maps claims to TrueFlow roles, and falls back to API key auth if no provider matches.
                    </p>
                </div>
            </div>

            {/* Provider List */}
            {isLoading ? (
                <div className="space-y-3">
                    {[1, 2].map(i => (
                        <div key={i} className="h-24 rounded-md border border-border/60 bg-card/50 animate-pulse" />
                    ))}
                </div>
            ) : providers.length === 0 ? (
                <Card className="border-border/60 bg-card/50">
                    <CardContent className="flex flex-col items-center justify-center py-16 gap-4 text-muted-foreground">
                        <Globe className="h-10 w-10 opacity-30" />
                        <p className="text-sm">No OIDC providers configured</p>
                        <ProviderFormDialog mode="create" onDone={() => refresh()} />
                    </CardContent>
                </Card>
            ) : (
                <div className="space-y-3">
                    {providers.map(p => (
                        <Card key={p.id} className={cn(
                            "border-border/60 bg-card/50 transition-opacity",
                            !p.is_active && "opacity-60"
                        )}>
                            <CardContent className="p-4">
                                <div className="flex items-start justify-between gap-4">
                                    <div className="flex-1 min-w-0 space-y-2">
                                        {/* Top row */}
                                        <div className="flex items-center gap-2 flex-wrap">
                                            <Badge variant={p.is_active ? "success" : "secondary"} dot={p.is_active} className="text-[10px] capitalize">
                                                {p.is_active ? "Active" : "Disabled"}
                                            </Badge>
                                            <span className="font-mono text-sm text-primary truncate">{p.issuer_url}</span>
                                        </div>
                                        {/* Meta grid */}
                                        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
                                            <div>
                                                <p className="text-muted-foreground uppercase tracking-wider text-[10px] mb-1">Client ID</p>
                                                <p className="font-mono truncate">{p.client_id}</p>
                                            </div>
                                            <div>
                                                <p className="text-muted-foreground uppercase tracking-wider text-[10px] mb-1">Audience</p>
                                                <p className="font-mono truncate">{p.audience ?? "—"}</p>
                                            </div>
                                            <div>
                                                <p className="text-muted-foreground uppercase tracking-wider text-[10px] mb-1">Claim Mappings</p>
                                                <p>{p.claim_mappings ? Object.keys(p.claim_mappings).length + " rule(s)" : "None"}</p>
                                            </div>
                                            <div>
                                                <p className="text-muted-foreground uppercase tracking-wider text-[10px] mb-1">Added</p>
                                                <p>{formatDistanceToNow(new Date(p.created_at), { addSuffix: true })}</p>
                                            </div>
                                        </div>
                                        {/* Claim mappings preview */}
                                        {p.claim_mappings && Object.keys(p.claim_mappings).length > 0 && (
                                            <div className="flex flex-wrap gap-2">
                                                {Object.entries(p.claim_mappings).map(([k, v]) => (
                                                    <span key={k} className="inline-flex items-center gap-1 text-[10px] font-mono bg-muted/50 px-1.5 py-0.5 rounded border border-border/40">
                                                        <span className="text-muted-foreground">{k}</span>
                                                        <span className="text-muted-foreground">→</span>
                                                        <span className="text-primary">{v}</span>
                                                    </span>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                    {/* Actions */}
                                    <div className="flex items-center gap-1 shrink-0">
                                        <Button
                                            variant="ghost"
                                            size="icon"
                                            className="h-7 w-7"
                                            onClick={() => handleToggle(p)}
                                            disabled={toggling === p.id}
                                            title={p.is_active ? "Disable provider" : "Enable provider"}
                                        >
                                            {p.is_active
                                                ? <ToggleRight className="h-4 w-4 text-emerald-500" />
                                                : <ToggleLeft className="h-4 w-4 text-muted-foreground" />}
                                        </Button>
                                        <ProviderFormDialog mode="edit" initial={p} onDone={() => refresh()} />
                                        <Button
                                            variant="ghost"
                                            size="icon"
                                            className="h-7 w-7 hover:text-destructive"
                                            onClick={() => handleDelete(p.id)}
                                            disabled={deleting === p.id}
                                        >
                                            <Trash2 className="h-3.5 w-3.5" />
                                        </Button>
                                    </div>
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}

            {/* ID suffix */}
            <p className="text-[11px] text-muted-foreground text-right font-mono">
                {providers.length} provider{providers.length !== 1 ? "s" : ""}
            </p>
        </div>
    );
}
