"use client";

import { useState, useEffect, useCallback } from "react";
import { useParams, useRouter } from "next/navigation";
import {
    getSession, getSessionEntity, updateSessionStatus, setSessionSpendCap,
    SessionSummary, SessionEntity
} from "@/lib/api";
import {
    ArrowLeft, DollarSign, Zap, Clock, Activity, Cpu, ChevronRight,
    Bot, Layers, PauseCircle, PlayCircle, CheckCircle, Edit2, Shield, RefreshCw,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
    Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { format, formatDistanceToNow } from "date-fns";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell, CartesianGrid } from "recharts";
import { cn } from "@/lib/utils";
import { CHART_AXIS_PROPS } from "@/components/ui/chart-utils";
import { toast } from "sonner";

const MODEL_COLORS = [
    "#cf3453", "#a9927d", "#d4a574", "#c47a50", "#e85d75",
    "#06b6d4", "#ec4899", "#84cc16",
];

function getModelColor(model: string | null, modelMap: Map<string, string>): string {
    if (!model) return "#94a3b8";
    if (modelMap.has(model)) return modelMap.get(model)!;
    const idx = modelMap.size % MODEL_COLORS.length;
    modelMap.set(model, MODEL_COLORS[idx]);
    return MODEL_COLORS[idx];
}

function StatCard({ icon: Icon, label, value, sub, color }: {
    icon: React.ElementType; label: string; value: string; sub?: string; color: string;
}) {
    return (
        <Card className="border-border/60 bg-card/50">
            <CardContent className="p-5 flex items-center gap-4">
                <div className={cn("p-2.5 rounded-md", color)}>
                    <Icon className="h-5 w-5" />
                </div>
                <div>
                    <p className="text-xs text-muted-foreground">{label}</p>
                    <p className="text-lg font-bold mt-0.5">{value}</p>
                    {sub && <p className="text-xs text-muted-foreground mt-0.5">{sub}</p>}
                </div>
            </CardContent>
        </Card>
    );
}

type SessionStatus = "active" | "paused" | "completed";

function StatusBadge({ status }: { status: SessionStatus | undefined }) {
    if (!status) return null;
    const cfg = {
        active: { variant: "success" as const, dot: true, label: "Active" },
        paused: { variant: "warning" as const, dot: true, label: "Paused" },
        completed: { variant: "secondary" as const, dot: false, label: "Completed" },
    };
    const c = cfg[status];
    return <Badge variant={c.variant} dot={c.dot} className="capitalize">{c.label}</Badge>;
}

function SpendCapDialog({
    sessionId, currentCap, onSaved,
}: { sessionId: string; currentCap: string | null; onSaved: () => void }) {
    const [open, setOpen] = useState(false);
    const [value, setValue] = useState(currentCap ? parseFloat(currentCap).toString() : "");
    const [saving, setSaving] = useState(false);

    async function handleSave() {
        setSaving(true);
        try {
            const cap = value.trim() === "" ? null : parseFloat(value);
            await setSessionSpendCap(sessionId, cap);
            toast.success(cap === null ? "Spend cap removed" : `Spend cap set to $${cap.toFixed(2)}`);
            setOpen(false);
            onSaved();
        } catch {
            toast.error("Failed to update spend cap");
        } finally {
            setSaving(false);
        }
    }

    return (
        <>
            <Button variant="outline" size="sm" onClick={() => setOpen(true)} className="gap-1.5">
                <Edit2 className="h-3.5 w-3.5" />
                {currentCap ? `Cap: $${parseFloat(currentCap).toFixed(2)}` : "Set Spend Cap"}
            </Button>
            <Dialog open={open} onOpenChange={setOpen}>
                <DialogContent className="sm:max-w-[340px]">
                    <DialogHeader>
                        <DialogTitle>Session Spend Cap</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-3 py-2">
                        <p className="text-sm text-muted-foreground">
                            Set a maximum spend limit (USD) for this session. Leave blank to remove the cap.
                        </p>
                        <div className="space-y-1.5">
                            <Label className="text-xs">Limit (USD)</Label>
                            <Input
                                type="number"
                                min="0"
                                step="0.01"
                                placeholder="e.g. 5.00"
                                value={value}
                                onChange={e => setValue(e.target.value)}
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <DialogClose asChild>
                            <Button variant="outline" size="sm">Cancel</Button>
                        </DialogClose>
                        <Button size="sm" onClick={handleSave} disabled={saving}>
                            {saving ? "Saving…" : "Save"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}

export default function SessionDetailPage() {
    const params = useParams();
    const router = useRouter();
    const id = decodeURIComponent(params.id as string);

    const [session, setSession] = useState<SessionSummary | null>(null);
    const [entity, setEntity] = useState<SessionEntity | null>(null);
    const [loading, setLoading] = useState(true);
    const [actionLoading, setActionLoading] = useState(false);

    const loadData = useCallback(async () => {
        if (!id) return;
        try {
            const [sum, ent] = await Promise.allSettled([
                getSession(id),
                getSessionEntity(id),
            ]);
            if (sum.status === "fulfilled") setSession(sum.value);
            if (ent.status === "fulfilled") setEntity(ent.value);
        } catch (e) {
            console.error(e);
        } finally {
            setLoading(false);
        }
    }, [id]);

    useEffect(() => { loadData(); }, [loadData]);

    const handleStatusChange = async (newStatus: SessionStatus) => {
        setActionLoading(true);
        try {
            await updateSessionStatus(id, newStatus);
            toast.success(`Session ${newStatus}`);
            await loadData();
        } catch {
            toast.error(`Failed to ${newStatus} session`);
        } finally {
            setActionLoading(false);
        }
    };

    if (loading) {
        return (
            <div className="flex items-center justify-center min-h-[50vh]">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
            </div>
        );
    }

    if (!session) {
        return (
            <div className="flex flex-col items-center justify-center min-h-[50vh] gap-4">
                <Layers className="h-10 w-10 text-muted-foreground" />
                <h2 className="text-lg font-semibold">Session not found</h2>
                <Button onClick={() => router.back()}>Go Back</Button>
            </div>
        );
    }

    const totalCost = parseFloat(session.total_cost_usd ?? "0");
    const totalTokens = session.total_prompt_tokens + session.total_completion_tokens;
    const durationSec = session.total_latency_ms / 1000;
    const status = entity?.status;

    // Build model color map
    const modelColorMap = new Map<string, string>();
    (session.models_used ?? []).forEach((m, i) => modelColorMap.set(m, MODEL_COLORS[i % MODEL_COLORS.length]));

    // Build Gantt Chart data
    const firstReqTime = new Date(session.first_request_at).getTime();
    const sortedRequests = [...session.requests].sort(
        (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
    );
    const chartData = sortedRequests.map((r, i) => ({
        name: `#${i + 1}`,
        offsetMs: Math.max(0, new Date(r.created_at).getTime() - firstReqTime),
        durationMs: r.response_latency_ms ?? 0,
        model: r.model ?? "unknown",
        id: r.id,
    }));

    return (
        <div className="max-w-5xl mx-auto space-y-6 pb-20">
            {/* Nav */}
            <div className="flex items-center gap-4">
                <Button variant="ghost" size="sm" onClick={() => router.back()} className="gap-2">
                    <ArrowLeft className="h-4 w-4" /> Back to Sessions
                </Button>
                <div className="h-4 w-px bg-border" />
                <div className="flex items-center gap-2 min-w-0">
                    <Layers className="h-4 w-4 text-muted-foreground shrink-0" />
                    <span className="font-mono text-[13px] text-muted-foreground break-all">{session.session_id}</span>
                </div>
            </div>

            {/* Session Lifecycle Bar */}
            <div className="rounded-md border border-border/60 bg-card/50 px-5 py-4 flex flex-wrap items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                    <StatusBadge status={status} />
                    {entity && (
                        <div className="flex items-center gap-4 text-xs text-muted-foreground">
                            <span>
                                <span className="font-medium text-foreground">${parseFloat(entity.total_cost_usd).toFixed(6)}</span>
                                {" "}spent
                            </span>
                            {entity.spend_cap_usd && (
                                <span>
                                    of{" "}
                                    <span className="font-medium text-foreground">${parseFloat(entity.spend_cap_usd).toFixed(2)}</span>
                                    {" "}cap
                                </span>
                            )}
                        </div>
                    )}
                </div>
                <div className="flex items-center gap-2">
                    {entity && (
                        <SpendCapDialog
                            sessionId={id}
                            currentCap={entity.spend_cap_usd}
                            onSaved={loadData}
                        />
                    )}
                    <Button
                        variant="outline" size="sm"
                        onClick={() => loadData()}
                        disabled={actionLoading}
                    >
                        <RefreshCw className="h-3.5 w-3.5" />
                    </Button>
                    {status === "active" && (
                        <Button
                            variant="outline" size="sm"
                            onClick={() => handleStatusChange("paused")}
                            disabled={actionLoading}
                            className="gap-1.5 text-amber-500 border-amber-500/30 hover:bg-amber-500/10"
                        >
                            <PauseCircle className="h-3.5 w-3.5" /> Pause
                        </Button>
                    )}
                    {status === "paused" && (
                        <Button
                            variant="outline" size="sm"
                            onClick={() => handleStatusChange("active")}
                            disabled={actionLoading}
                            className="gap-1.5 text-emerald-500 border-emerald-500/30 hover:bg-emerald-500/10"
                        >
                            <PlayCircle className="h-3.5 w-3.5" /> Resume
                        </Button>
                    )}
                    {status && status !== "completed" && (
                        <Button
                            variant="outline" size="sm"
                            onClick={() => handleStatusChange("completed")}
                            disabled={actionLoading}
                            className="gap-1.5 text-muted-foreground"
                        >
                            <CheckCircle className="h-3.5 w-3.5" /> Mark Done
                        </Button>
                    )}
                    {entity?.token_id && (
                        <span className="text-xs text-muted-foreground font-mono hidden lg:inline">
                            <Shield className="h-3 w-3 inline mr-0.5 opacity-50" />
                            {entity.token_id.slice(0, 8)}…
                        </span>
                    )}
                </div>
            </div>

            {/* Time Range */}
            <div className="rounded-md border border-border/60 bg-card/50 px-6 py-4 flex flex-wrap items-center gap-4 text-sm">
                <div>
                    <span className="text-muted-foreground">Started: </span>
                    <span className="font-medium">{format(new Date(session.first_request_at), "MMM d, yyyy HH:mm:ss")}</span>
                </div>
                <div className="h-4 w-px bg-border hidden md:block" />
                <div>
                    <span className="text-muted-foreground">Last active: </span>
                    <span className="font-medium">{format(new Date(session.last_request_at), "HH:mm:ss")}</span>
                </div>
                <div className="h-4 w-px bg-border hidden md:block" />
                <div>
                    <span className="text-muted-foreground">{formatDistanceToNow(new Date(session.last_request_at), { addSuffix: true })}</span>
                </div>
                {(session.models_used ?? []).length > 0 && (
                    <div className="flex items-center gap-1 ml-auto">
                        {(session.models_used ?? []).map((m, i) => (
                            <Badge
                                key={m}
                                variant="secondary"
                                className="text-xs font-mono"
                                style={{ borderColor: MODEL_COLORS[i % MODEL_COLORS.length], color: MODEL_COLORS[i % MODEL_COLORS.length] }}
                            >
                                {m.includes("/") ? m.split("/").pop() : m}
                            </Badge>
                        ))}
                    </div>
                )}
            </div>

            {/* Summary Cards */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <StatCard icon={DollarSign} label="Total Cost" value={`$${totalCost.toFixed(6)}`} color="bg-emerald-500/10 text-emerald-500" />
                <StatCard icon={Zap} label="Total Requests" value={session.total_requests.toString()} color="bg-primary/10 text-primary" />
                <StatCard icon={Cpu} label="Total Tokens" value={totalTokens.toLocaleString()} sub={`↑${session.total_prompt_tokens.toLocaleString()} ↓${session.total_completion_tokens.toLocaleString()}`} color="bg-violet-500/10 text-violet-500" />
                <StatCard icon={Clock} label="Wall-clock" value={`${durationSec.toFixed(1)}s`} color="bg-amber-500/10 text-amber-500" />
            </div>

            {/* Gantt Chart */}
            {chartData.length > 0 && (
                <Card className="border-border/60 bg-card/50">
                    <CardHeader className="pb-3">
                        <CardTitle className="text-base">Agent Session Timeline</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <ResponsiveContainer width="100%" height={Math.max(180, chartData.length * 40 + 60)}>
                            <BarChart layout="vertical" data={chartData} margin={{ top: 0, right: 20, bottom: 0, left: -20 }}>
                                <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                                <XAxis type="number" tickFormatter={(v) => `${v}ms`} {...CHART_AXIS_PROPS} />
                                <YAxis type="category" dataKey="name" {...CHART_AXIS_PROPS} width={50} />
                                <Tooltip
                                    cursor={{ fill: 'var(--border)', opacity: 0.1 }}
                                    content={({ active, payload }) => {
                                        if (!active || !payload?.length) return null;
                                        const d = payload[0].payload;
                                        return (
                                            <div className="rounded-md border border-border/50 bg-background/95 p-3 text-sm shadow-xl backdrop-blur-sm z-[100]">
                                                <p className="font-mono text-xs text-muted-foreground uppercase tracking-wider mb-2">{d.name} ({d.model})</p>
                                                <p className="font-medium text-emerald-400">{d.durationMs}ms duration</p>
                                                <p className="text-xs text-muted-foreground mt-1">Started at +{d.offsetMs}ms</p>
                                            </div>
                                        );
                                    }}
                                />
                                <Bar dataKey="offsetMs" stackId="timeline" fill="transparent" />
                                <Bar dataKey="durationMs" stackId="timeline" radius={[4, 4, 4, 4]}>
                                    {chartData.map((d) => (
                                        <Cell key={d.id} fill={getModelColor(d.model, modelColorMap)} />
                                    ))}
                                </Bar>
                            </BarChart>
                        </ResponsiveContainer>
                        <div className="flex flex-wrap gap-3 mt-3">
                            {Array.from(modelColorMap.entries()).map(([model, color]) => (
                                <div key={model} className="flex items-center gap-1.5 text-xs text-muted-foreground">
                                    <div className="h-2.5 w-2.5 rounded-sm" style={{ background: color }} />
                                    <span className="font-mono">{model.includes("/") ? model.split("/").pop() : model}</span>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Request Timeline Table */}
            <Card className="border-border/60 bg-card/50">
                <CardHeader className="pb-3">
                    <CardTitle className="text-base">Request Timeline</CardTitle>
                </CardHeader>
                <CardContent className="p-0">
                    <div className="overflow-x-auto">
                        <table className="w-full text-sm">
                            <thead>
                                <tr className="border-b border-border/60 bg-muted/20">
                                    <th className="text-left px-4 py-2 font-medium text-muted-foreground">#</th>
                                    <th className="text-left px-4 py-2 font-medium text-muted-foreground">Model</th>
                                    <th className="text-right px-4 py-2 font-medium text-muted-foreground">Cost</th>
                                    <th className="text-right px-4 py-2 font-medium text-muted-foreground">Latency</th>
                                    <th className="text-right px-4 py-2 font-medium text-muted-foreground">Tokens</th>
                                    <th className="text-right px-4 py-2 font-medium text-muted-foreground hidden md:table-cell">Tools</th>
                                    <th className="text-center px-4 py-2 font-medium text-muted-foreground hidden md:table-cell">Cache</th>
                                    <th className="text-left px-4 py-2 font-medium text-muted-foreground hidden lg:table-cell">Properties</th>
                                    <th className="px-4 py-2 w-8" />
                                </tr>
                            </thead>
                            <tbody>
                                {session.requests.length === 0 ? (
                                    <tr>
                                        <td colSpan={9} className="px-4 py-10 text-center text-muted-foreground text-sm">
                                            <Bot className="h-8 w-8 mx-auto mb-2 opacity-30" />
                                            No request data available
                                        </td>
                                    </tr>
                                ) : session.requests.map((r, i) => {
                                    const reqCost = parseFloat(r.estimated_cost_usd ?? "0");
                                    const tokens = (r.prompt_tokens ?? 0) + (r.completion_tokens ?? 0);
                                    const latencyMs = r.response_latency_ms ?? 0;
                                    const color = getModelColor(r.model, modelColorMap);
                                    const props = r.custom_properties;

                                    return (
                                        <tr
                                            key={r.id}
                                            onClick={() => router.push(`/audit/${r.id}`)}
                                            className="border-b border-border/40 hover:bg-muted/20 cursor-pointer transition-colors group"
                                        >
                                            <td className="px-4 py-2.5 text-muted-foreground tabular-nums">{i + 1}</td>
                                            <td className="px-4 py-2.5">
                                                {r.model ? (
                                                    <div className="flex items-center gap-1.5">
                                                        <div className="h-2 w-2 rounded-sm flex-shrink-0" style={{ background: color }} />
                                                        <span className="font-mono text-xs">
                                                            {r.model.includes("/") ? r.model.split("/").pop() : r.model}
                                                        </span>
                                                    </div>
                                                ) : <span className="text-muted-foreground">—</span>}
                                            </td>
                                            <td className="px-4 py-2.5 text-right font-mono tabular-nums text-emerald-500 text-xs">${reqCost.toFixed(6)}</td>
                                            <td className="px-4 py-2.5 text-right tabular-nums text-muted-foreground text-xs">
                                                {latencyMs >= 1000 ? `${(latencyMs / 1000).toFixed(1)}s` : `${latencyMs}ms`}
                                            </td>
                                            <td className="px-4 py-2.5 text-right tabular-nums text-muted-foreground text-xs">
                                                {tokens > 0 ? tokens.toLocaleString() : "—"}
                                            </td>
                                            <td className="px-4 py-2.5 text-right tabular-nums text-muted-foreground text-xs hidden md:table-cell">
                                                {r.tool_call_count ?? "—"}
                                            </td>
                                            <td className="px-4 py-2.5 text-center hidden md:table-cell">
                                                {r.cache_hit != null && (
                                                    <Badge variant={r.cache_hit ? "default" : "outline"} className="text-xs px-1.5 py-0">
                                                        {r.cache_hit ? "HIT" : "MISS"}
                                                    </Badge>
                                                )}
                                            </td>
                                            <td className="px-4 py-2.5 hidden lg:table-cell">
                                                {props && Object.keys(props).length > 0 ? (
                                                    <div className="flex flex-wrap gap-1">
                                                        {Object.entries(props).slice(0, 3).map(([k, v]) => (
                                                            <span key={k} className="inline-flex items-center gap-1 text-xs bg-muted/60 px-1.5 py-0.5 rounded font-mono">
                                                                <span className="text-muted-foreground">{k}:</span>
                                                                <span>{String(v)}</span>
                                                            </span>
                                                        ))}
                                                        {Object.keys(props).length > 3 && (
                                                            <span className="text-xs text-muted-foreground">+{Object.keys(props).length - 3}</span>
                                                        )}
                                                    </div>
                                                ) : <span className="text-muted-foreground text-xs">—</span>}
                                            </td>
                                            <td className="px-4 py-2.5">
                                                <ChevronRight className="h-4 w-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
