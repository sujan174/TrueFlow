"use client";

import { useState, useEffect, useCallback } from "react";
import {
    listWebhooks,
    createWebhook,
    deleteWebhook,
    testWebhook,
    Webhook,
} from "@/lib/api";
import {
    Webhook as WebhookIcon,
    Plus,
    Trash2,
    Send,
    RefreshCw,
    CheckCircle2,
    XCircle,
    Bell,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { PageSkeleton } from "@/components/page-skeleton";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

const ALL_EVENTS = [
    { value: "policy_violation", label: "Policy Violation", color: "text-rose-500" },
    { value: "rate_limit_exceeded", label: "Rate Limit Exceeded", color: "text-amber-500" },
    { value: "spend_cap_exceeded", label: "Spend Cap Exceeded", color: "text-violet-500" },
];

function EventBadge({ event }: { event: string }) {
    const def = ALL_EVENTS.find((e) => e.value === event);
    return (
        <span className={cn(
            "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold border",
            "border-border/60 bg-muted/40",
            def?.color ?? "text-muted-foreground"
        )}>
            {def?.label ?? event}
        </span>
    );
}

export default function WebhooksPage() {
    const [webhooks, setWebhooks] = useState<Webhook[]>([]);
    const [loading, setLoading] = useState(true);
    const [showAdd, setShowAdd] = useState(false);
    const [newUrl, setNewUrl] = useState("");
    const [selectedEvents, setSelectedEvents] = useState<string[]>([]);
    const [saving, setSaving] = useState(false);
    const [testingId, setTestingId] = useState<string | null>(null);

    const fetchWebhooks = useCallback(async () => {
        try {
            setLoading(true);
            setWebhooks(await listWebhooks());
        } catch {
            toast.error("Failed to load webhooks");
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => { fetchWebhooks(); }, [fetchWebhooks]);

    const handleCreate = async () => {
        if (!newUrl.trim()) { toast.error("URL is required"); return; }
        try {
            setSaving(true);
            const created = await createWebhook({
                url: newUrl.trim(),
                events: selectedEvents.length > 0 ? selectedEvents : undefined,
            });
            setWebhooks((prev) => [created, ...prev]);
            setNewUrl("");
            setSelectedEvents([]);
            setShowAdd(false);
            toast.success("Webhook created");
        } catch {
            toast.error("Failed to create webhook");
        } finally {
            setSaving(false);
        }
    };

    const handleDelete = async (id: string) => {
        try {
            await deleteWebhook(id);
            setWebhooks((prev) => prev.filter((w) => w.id !== id));
            toast.success("Webhook removed");
        } catch {
            toast.error("Failed to delete webhook");
        }
    };

    const handleTest = async (webhook: Webhook) => {
        setTestingId(webhook.id);
        try {
            const result = await testWebhook(webhook.url);
            if (result.success) {
                toast.success("Test event delivered successfully");
            } else {
                toast.error(`Delivery failed: ${result.message}`);
            }
        } catch {
            toast.error("Test delivery failed");
        } finally {
            setTestingId(null);
        }
    };

    const toggleEvent = (event: string) => {
        setSelectedEvents((prev) =>
            prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event]
        );
    };

    if (loading) return <PageSkeleton />;

    return (
        <div className="space-y-4">
            {/* Controls */}
            <div className="flex items-center justify-end animate-fade-in mb-2">
                <div className="flex items-center gap-2">
                    <Button variant="outline" size="sm" onClick={fetchWebhooks}>
                        <RefreshCw className="h-3.5 w-3.5 mr-1.5" />
                        Refresh
                    </Button>
                    <Button size="sm" onClick={() => setShowAdd((v) => !v)}>
                        <Plus className="h-4 w-4 mr-1.5" />
                        Add Webhook
                    </Button>
                </div>
            </div>

            {/* Add Webhook Form */}
            {showAdd && (
                <Card className="border-primary/30 bg-primary/5 animate-fade-in">
                    <CardHeader className="pb-3">
                        <CardTitle className="text-base">New Webhook Endpoint</CardTitle>
                        <CardDescription>
                            TrueFlow will POST a JSON payload to this URL when the selected events occur.
                        </CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div>
                            <label className="text-xs font-medium text-muted-foreground mb-2 block">
                                Endpoint URL
                            </label>
                            <input
                                type="url"
                                placeholder="https://your-server.com/webhook"
                                value={newUrl}
                                onChange={(e) => setNewUrl(e.target.value)}
                                className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-primary/40"
                            />
                        </div>
                        <div>
                            <label className="text-xs font-medium text-muted-foreground mb-2 block">
                                Event Filter <span className="text-muted-foreground/60">(leave empty to receive all events)</span>
                            </label>
                            <div className="flex flex-wrap gap-2">
                                {ALL_EVENTS.map((ev) => (
                                    <button
                                        key={ev.value}
                                        onClick={() => toggleEvent(ev.value)}
                                        className={cn(
                                            "rounded-full px-3 py-1 text-xs font-medium border transition-all",
                                            selectedEvents.includes(ev.value)
                                                ? "bg-primary text-primary-foreground border-primary"
                                                : "border-border/60 text-muted-foreground hover:border-primary/40"
                                        )}
                                    >
                                        {ev.label}
                                    </button>
                                ))}
                            </div>
                        </div>
                        <div className="flex gap-2 pt-1">
                            <Button size="sm" onClick={handleCreate} disabled={saving}>
                                {saving ? <RefreshCw className="h-3.5 w-3.5 mr-1.5 animate-spin" /> : <Plus className="h-3.5 w-3.5 mr-1.5" />}
                                Create Webhook
                            </Button>
                            <Button size="sm" variant="ghost" onClick={() => setShowAdd(false)}>
                                Cancel
                            </Button>
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Webhooks List */}
            {webhooks.length === 0 ? (
                <Card className="border-dashed">
                    <CardContent className="flex flex-col items-center justify-center py-16 gap-3">
                        <div className="flex h-12 w-12 items-center justify-center rounded-md bg-muted">
                            <WebhookIcon className="h-6 w-6 text-muted-foreground" />
                        </div>
                        <p className="text-sm font-medium">No webhooks configured</p>
                        <p className="text-xs text-muted-foreground text-center max-w-xs">
                            Add a webhook URL to receive real-time notifications for policy violations, spend cap breaches, and rate limit events.
                        </p>
                        <Button size="sm" className="mt-2" onClick={() => setShowAdd(true)}>
                            <Plus className="h-4 w-4 mr-1.5" />
                            Add your first webhook
                        </Button>
                    </CardContent>
                </Card>
            ) : (
                <div className="space-y-3 animate-fade-in">
                    {webhooks.map((webhook) => (
                        <Card key={webhook.id} className="glass-card">
                            <CardContent className="py-4 px-5">
                                <div className="flex items-start justify-between gap-4">
                                    <div className="flex-1 min-w-0 space-y-2">
                                        <div className="flex items-center gap-2">
                                            <div className={cn(
                                                "h-2 w-2 rounded-full flex-shrink-0",
                                                webhook.is_active ? "bg-emerald-500" : "bg-muted-foreground"
                                            )} />
                                            <p className="font-mono text-sm truncate">{webhook.url}</p>
                                        </div>
                                        <div className="flex flex-wrap items-center gap-2">
                                            {webhook.events.length === 0 ? (
                                                <span className="text-[10px] text-muted-foreground italic">All events</span>
                                            ) : (
                                                webhook.events.map((ev) => <EventBadge key={ev} event={ev} />)
                                            )}
                                        </div>
                                        <p className="text-[10px] text-muted-foreground">
                                            Added {new Date(webhook.created_at).toLocaleDateString()}
                                        </p>
                                    </div>
                                    <div className="flex items-center gap-2 flex-shrink-0">
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            onClick={() => handleTest(webhook)}
                                            disabled={testingId === webhook.id}
                                            className="gap-2 text-xs"
                                        >
                                            {testingId === webhook.id ? (
                                                <RefreshCw className="h-3 w-3 animate-spin" />
                                            ) : (
                                                <Send className="h-3 w-3" />
                                            )}
                                            Test
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => handleDelete(webhook.id)}
                                            className="text-muted-foreground hover:text-rose-500 hover:bg-rose-500/10"
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

            {/* Info Card */}
            <Card className="border-border/40 bg-muted/20">
                <CardContent className="py-4 px-5">
                    <div className="flex gap-3">
                        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-blue-500/10 flex-shrink-0 mt-0.5">
                            <WebhookIcon className="h-4 w-4 text-blue-500" />
                        </div>
                        <div className="space-y-1">
                            <p className="text-sm font-medium">Webhook Payload Format</p>
                            <p className="text-xs text-muted-foreground">
                                TrueFlow sends a <code className="font-mono bg-muted px-1 rounded">POST</code> request with{" "}
                                <code className="font-mono bg-muted px-1 rounded">Content-Type: application/json</code>.
                                Deliveries are fire-and-forget — failures are logged but do not block requests.
                            </p>
                            <pre className="mt-2 text-[10px] font-mono bg-muted/60 rounded-md p-3 overflow-x-auto text-muted-foreground">
                                {`{
  "event_type": "spend_cap_exceeded",
  "timestamp": "2026-02-18T22:00:00Z",
  "token_id": "...",
  "token_name": "my-agent",
  "project_id": "...",
  "details": { "reason": "daily spend cap of $10.00 exceeded" }
}`}
                            </pre>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
