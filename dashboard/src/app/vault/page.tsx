"use client";

import { useState } from "react";
import useSWR, { mutate } from "swr";
import { listCredentials, createCredential, rotateCredential, Credential, swrFetcher } from "@/lib/api";
import { Fingerprint, Plus, Lock, RefreshCw, Server, RotateCw, Copy, Check, AlertTriangle, Key } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { PageSkeleton } from "@/components/page-skeleton";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";

const EMPTY_CREDENTIALS: Credential[] = [];

export default function CredentialsPage() {
    const { data: credentialsData, isLoading, mutate: mutateCredentials } = useSWR<Credential[]>("/credentials", swrFetcher);
    const credentials = credentialsData || EMPTY_CREDENTIALS;

    const [showModal, setShowModal] = useState(false);

    // Rotation state
    const [rotatingId, setRotatingId] = useState<string | null>(null);
    const [newSecret, setNewSecret] = useState<string | null>(null);
    const [rotateLoading, setRotateLoading] = useState(false);
    const [copied, setCopied] = useState(false);

    const handleRotate = async () => {
        if (!rotatingId) return;
        setRotateLoading(true);
        try {
            const res = await rotateCredential(rotatingId);
            setNewSecret(res.secret);
            toast.success("Credential rotated successfully");
            mutateCredentials();
        } catch (e) {
            toast.error("Failed to rotate credential");
            setRotatingId(null); // Close on error
        } finally {
            setRotateLoading(false);
        }
    };

    const handleCopy = () => {
        if (newSecret) {
            navigator.clipboard.writeText(newSecret);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    const activeCount = credentials.filter((c) => c.is_active).length;

    return (
        <div className="space-y-4">
            {/* Controls */}
            <div className="flex items-center justify-end animate-fade-in mb-2">
                <div className="flex items-center gap-2">
                    <Button variant="outline" size="sm" onClick={() => mutateCredentials()} disabled={isLoading}>
                        <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", isLoading && "animate-spin")} />
                        Refresh
                    </Button>
                    <Button size="sm" onClick={() => setShowModal(true)}>
                        <Plus className="mr-1.5 h-3.5 w-3.5" /> Add Credential
                    </Button>
                </div>
            </div>

            {/* KPI Cards */}
            <div className="grid gap-4 md:grid-cols-3 animate-slide-up">
                <StatCard
                    icon={Lock}
                    label="Total Credentials"
                    value={credentials.length}
                    color="blue"
                    loading={isLoading}
                />
                <StatCard
                    icon={Fingerprint}
                    label="Active"
                    value={activeCount}
                    color="emerald"
                    loading={isLoading}
                />
                <StatCard
                    icon={Server}
                    label="Providers"
                    value={new Set(credentials.map((c) => c.provider)).size}
                    color="violet"
                    loading={isLoading}
                />
            </div>

            {/* Table */}
            <div className="animate-slide-up stagger-2">
                {isLoading && credentials.length === 0 ? (
                    <PageSkeleton cards={0} rows={5} />
                ) : credentials.length === 0 ? (
                    <Card className="glass-card p-12">
                        <EmptyState
                            icon={Lock}
                            title="No credentials found"
                            description="Add your first API key to the secure vault. We encrypt it with AES-256-GCM."
                            actionLabel="Add Credential"
                            onAction={() => setShowModal(true)}
                        />
                    </Card>
                ) : (
                    <Card className="glass-card overflow-hidden">
                        <Table>
                            <TableHeader>
                                <TableRow className="bg-muted/20 hover:bg-muted/20">
                                    <TableHead className="uppercase text-xs tracking-wider">Name</TableHead>
                                    <TableHead className="uppercase text-xs tracking-wider">Provider</TableHead>
                                    <TableHead className="uppercase text-xs tracking-wider">Version</TableHead>
                                    <TableHead className="uppercase text-xs tracking-wider">Status</TableHead>
                                    <TableHead className="uppercase text-xs tracking-wider">Created</TableHead>
                                    <TableHead className="w-[100px]"></TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {credentials.map((cred) => (
                                    <TableRow key={cred.id} className="hover:bg-muted/30 transition-colors">
                                        <TableCell className="font-medium text-foreground flex items-center gap-2">
                                            <Key className="h-4 w-4 text-muted-foreground" />
                                            {cred.name}
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant="outline" className="font-mono text-xs bg-muted/50">
                                                {cred.provider}
                                            </Badge>
                                        </TableCell>
                                        <TableCell className="text-muted-foreground text-xs font-mono">v{cred.version}</TableCell>
                                        <TableCell>
                                            <Badge variant={cred.is_active ? "success" : "secondary"} dot>
                                                {cred.is_active ? "Active" : "Revoked"}
                                            </Badge>
                                        </TableCell>
                                        <TableCell className="text-muted-foreground text-xs font-mono">
                                            {new Date(cred.created_at).toLocaleDateString()}
                                        </TableCell>
                                        <TableCell>
                                            <Button
                                                variant="ghost"
                                                size="icon"
                                                title="Rotate Credential"
                                                className="hover:text-primary"
                                                onClick={() => setRotatingId(cred.id)}
                                            >
                                                <RotateCw className="h-4 w-4" />
                                            </Button>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </Card>
                )}
            </div>

            {/* Create Modal */}
            {showModal && (
                <CreateCredentialModal
                    onClose={() => setShowModal(false)}
                    onSuccess={() => {
                        setShowModal(false);
                        mutateCredentials();
                    }}
                />
            )}

            {/* Rotation Modal */}
            <Dialog open={!!rotatingId} onOpenChange={(open) => {
                if (!open) {
                    setRotatingId(null);
                    setNewSecret(null);
                }
            }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2">
                            <RotateCw className="h-5 w-5 text-blue-500" />
                            Rotate Credential
                        </DialogTitle>
                        <DialogDescription>
                            Are you sure you want to rotate this credential? This will generate a new version.
                            Existing tokens will continue to work until you update them or revoke the old version.
                        </DialogDescription>
                    </DialogHeader>

                    {newSecret ? (
                        <div className="space-y-4 py-4">
                            <div className="rounded-md bg-emerald-500/10 p-4 border border-emerald-500/20">
                                <div className="flex items-center gap-2 text-emerald-500 font-medium mb-2">
                                    <Check className="h-4 w-4" /> Rotation Successful
                                </div>
                                <p className="text-[13px] text-muted-foreground mb-4">
                                    Here is your new secret. Copy it now, you won&apos;t see it again.
                                </p>
                                <div className="relative">
                                    <pre className="p-3 bg-background rounded-md border border-border font-mono text-sm break-all pr-10">
                                        {newSecret}
                                    </pre>
                                    <Button
                                        size="icon"
                                        variant="ghost"
                                        className="absolute right-1 top-1 h-7 w-7"
                                        onClick={handleCopy}
                                    >
                                        {copied ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}
                                    </Button>
                                </div>
                            </div>
                            <DialogFooter>
                                <Button onClick={() => {
                                    setRotatingId(null);
                                    setNewSecret(null);
                                }}>Done</Button>
                            </DialogFooter>
                        </div>
                    ) : (
                        <DialogFooter>
                            <Button variant="outline" onClick={() => setRotatingId(null)}>Cancel</Button>
                            <Button onClick={handleRotate} disabled={rotateLoading}>
                                {rotateLoading && <RotateCw className="mr-2 h-4 w-4 animate-spin" />}
                                Rotate Key
                            </Button>
                        </DialogFooter>
                    )}
                </DialogContent>
            </Dialog>
        </div>
    );
}

function StatCard({ icon: Icon, label, value, color, loading }: {
    icon: React.ComponentType<{ className?: string }>;
    label: string;
    value: number;
    color: "blue" | "emerald" | "violet";
    loading?: boolean;
}) {
    const bgColors = {
        blue: "bg-blue-500/10 text-blue-500",
        emerald: "bg-emerald-500/10 text-emerald-500",
        violet: "bg-violet-500/10 text-violet-500",
    };
    return (
        <Card className="glass-card hover-lift">
            <CardContent className="p-4 flex items-center gap-4">
                <div className={cn("p-3 rounded-md transition-colors", bgColors[color])}>
                    <Icon className="h-6 w-6" />
                </div>
                <div>
                    <p className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-1">{label}</p>
                    {loading ? (
                        <div className="h-8 w-16 bg-muted/50 rounded shimmer my-0.5" />
                    ) : (
                        <p className="text-2xl font-semibold font-bold tabular-nums tracking-tight">{value}</p>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}

function CreateCredentialModal({ onClose, onSuccess }: { onClose: () => void; onSuccess: () => void }) {
    const [name, setName] = useState("");
    const [provider, setProvider] = useState("openai");
    const [secret, setSecret] = useState("");
    const [loading, setLoading] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        try {
            await createCredential({ name, provider, secret });
            toast.success("Credential created securely");
            onSuccess();
        } catch (e) {
            toast.error("Failed to create credential");
        } finally {
            setLoading(false);
        }
    };

    return (
        <Dialog open onOpenChange={() => onClose()}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Add Credential</DialogTitle>
                    <DialogDescription>
                        Securely store an API key. It will be encrypted at rest.
                    </DialogDescription>
                </DialogHeader>
                <form onSubmit={handleSubmit} className="space-y-4">
                    <div className="space-y-2">
                        <Label htmlFor="name">Name</Label>
                        <Input
                            id="name"
                            placeholder="e.g. OpenAI Prod"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            required
                        />
                    </div>
                    <div className="space-y-2">
                        <Label htmlFor="provider">Provider</Label>
                        <select
                            id="provider"
                            value={provider}
                            onChange={(e) => setProvider(e.target.value)}
                            required
                            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                        >
                            <option value="openai">OpenAI</option>
                            <option value="anthropic">Anthropic</option>
                            <option value="gemini">Google Gemini</option>
                            <option value="azure-openai">Azure OpenAI</option>
                            <option value="bedrock">AWS Bedrock</option>
                            <option value="groq">Groq</option>
                            <option value="mistral">Mistral AI</option>
                            <option value="together-ai">Together AI</option>
                            <option value="cohere">Cohere</option>
                            <option value="ollama">Ollama (self-hosted)</option>
                            <option value="custom">Custom / Other</option>
                        </select>
                    </div>
                    <div className="space-y-2">
                        <Label htmlFor="secret">API Key / Secret</Label>
                        <div className="relative">
                            <Input
                                id="secret"
                                type="password"
                                placeholder="sk-..."
                                value={secret}
                                onChange={(e) => setSecret(e.target.value)}
                                required
                                className="pr-10"
                            />
                            <Lock className="absolute right-3 top-2.5 h-4 w-4 text-muted-foreground" />
                        </div>
                        <p className="text-[10px] text-muted-foreground">
                            <Lock className="inline h-3 w-3 mr-1" />
                            Encrypted with AES-256-GCM
                        </p>
                    </div>
                    <DialogFooter>
                        <Button type="button" variant="outline" onClick={onClose}>
                            Cancel
                        </Button>
                        <Button type="submit" disabled={loading}>
                            {loading && <RefreshCw className="mr-2 h-4 w-4 animate-spin" />}
                            Encrypt & Save
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}
