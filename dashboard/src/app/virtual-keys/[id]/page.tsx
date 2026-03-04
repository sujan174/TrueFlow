"use client";

import { useEffect, useState, useCallback } from "react";
import { useParams, useRouter } from "next/navigation";
import { getToken, listAuditLogs, getTokenUsage, getSpendCaps, upsertSpendCap, deleteSpendCap, getCircuitBreaker, updateCircuitBreaker, Token, AuditLog, TokenUsageStats, SpendStatus, CircuitBreakerConfig } from "@/lib/api";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import {
    Key,
    Shield,
    Calendar,
    ArrowLeft,
    Activity,
    Clock,
    CheckCircle2,
    XCircle,
    AlertTriangle,
    Copy,
    Trash2,
    DollarSign,
    TrendingUp,
    X,
    CircuitBoard,
    ToggleLeft,
    ToggleRight,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { DataTable } from "@/components/data-table";
import { columns } from "@/app/audit/columns";
import { cn } from "@/lib/utils";
import { toast } from "sonner";

function StatCard({
    icon: Icon,
    label,
    value,
    sub,
    color,
}: {
    icon: React.ElementType
    label: string
    value: string
    sub?: string
    color: string
}) {
    return (
        <div className="rounded-md border border-border/60 bg-card/50 backdrop-blur-sm p-4 flex items-center gap-3 min-w-0">
            <div className={`rounded-md p-2 ${color}`}>
                <Icon className="h-4 w-4" />
            </div>
            <div className="min-w-0">
                <p className="text-xs text-muted-foreground">{label}</p>
                <p className="text-lg font-semibold truncate">{value}</p>
                {sub && <p className="text-[10px] text-muted-foreground">{sub}</p>}
            </div>
        </div>
    )
}

export default function TokenDetailPage() {
    const params = useParams();
    const router = useRouter();
    const id = params.id as string;

    const [token, setToken] = useState<Token | null>(null);
    const [logs, setLogs] = useState<AuditLog[]>([]);
    const [usage, setUsage] = useState<TokenUsageStats | null>(null);
    const [spend, setSpend] = useState<SpendStatus | null>(null);
    const [cb, setCb] = useState<CircuitBreakerConfig | null>(null);
    const [cbEdits, setCbEdits] = useState<Partial<CircuitBreakerConfig>>({});
    const [savingCb, setSavingCb] = useState(false);
    const [loading, setLoading] = useState(true);
    const [copied, setCopied] = useState(false);
    const [capInput, setCapInput] = useState<{ daily: string; monthly: string }>({ daily: "", monthly: "" });
    const [savingCap, setSavingCap] = useState<"daily" | "monthly" | null>(null);

    useEffect(() => {
        if (!id) return;

        const loadData = async () => {
            try {
                const [t, l, u, s, c] = await Promise.all([
                    getToken(id),
                    listAuditLogs(50, 0, { token_id: id }),
                    getTokenUsage(id),
                    getSpendCaps(id).catch(() => null),
                    getCircuitBreaker(id).catch(() => null),
                ]);
                setToken(t);
                setLogs(l);
                setUsage(u);
                setSpend(s);
                setCb(c);
                setCbEdits(c ?? {});
                if (s) {
                    setCapInput({
                        daily: s.daily_limit_usd != null ? String(s.daily_limit_usd) : "",
                        monthly: s.monthly_limit_usd != null ? String(s.monthly_limit_usd) : "",
                    });
                }
            } catch (e) {
                console.error(e);
                toast.error("Failed to load token details");
            } finally {
                setLoading(false);
            }
        };

        loadData();
    }, [id]);

    const handleCopy = () => {
        if (!token) return;
        navigator.clipboard.writeText(token.id);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const handleSaveCap = async (period: "daily" | "monthly") => {
        const val = period === "daily" ? capInput.daily : capInput.monthly;
        const num = parseFloat(val);
        if (isNaN(num) || num <= 0) { toast.error("Enter a valid positive amount"); return; }
        setSavingCap(period);
        try {
            await upsertSpendCap(id, period, num);
            const updated = await getSpendCaps(id);
            setSpend(updated);
            toast.success(`${period === "daily" ? "Daily" : "Monthly"} cap saved`);
        } catch { toast.error("Failed to save cap"); }
        finally { setSavingCap(null); }
    };

    const handleRemoveCap = async (period: "daily" | "monthly") => {
        try {
            await deleteSpendCap(id, period);
            const updated = await getSpendCaps(id);
            setSpend(updated);
            setCapInput((prev) => ({ ...prev, [period]: "" }));
            toast.success(`${period === "daily" ? "Daily" : "Monthly"} cap removed`);
        } catch { toast.error("Failed to remove cap"); }
    };

    const handleSaveCb = async () => {
        setSavingCb(true);
        try {
            const updated = await updateCircuitBreaker(id, cbEdits);
            setCb(updated);
            setCbEdits(updated);
            toast.success("Circuit breaker config saved");
        } catch (e: any) {
            toast.error(e.message || "Failed to update circuit breaker");
        } finally {
            setSavingCb(false);
        }
    };

    if (loading) {
        return (
            <div className="flex items-center justify-center min-h-[50vh]">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
            </div>
        );
    }

    if (!token) {
        return (
            <div className="flex flex-col items-center justify-center min-h-[50vh] gap-4">
                <AlertTriangle className="h-10 w-10 text-muted-foreground" />
                <h2 className="text-lg font-semibold">Token not found</h2>
                <Button onClick={() => router.back()}>Go Back</Button>
            </div>
        );
    }

    // Stats from logs
    const totalRequests = logs.length; // This is just the recent 50, but gives an idea
    const errorCount = logs.filter(l => (l.upstream_status || 0) >= 400).length;
    const avgLatency = logs.length > 0
        ? Math.round(logs.reduce((s, l) => s + l.response_latency_ms, 0) / logs.length)
        : 0;

    return (
        <div className="space-y-6 max-w-6xl mx-auto pb-20">
            {/* Nav */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <Button variant="ghost" size="sm" onClick={() => router.push('/tokens')} className="gap-2">
                        <ArrowLeft className="h-4 w-4" /> Back to Tokens
                    </Button>
                    <div className="h-4 w-px bg-border" />
                    <div className="flex items-center gap-2">
                        <Key className="h-4 w-4 text-muted-foreground" />
                        <span className="font-semibold text-lg">{token.name}</span>
                        <Badge variant={token.is_active ? "default" : "secondary"} className="ml-2">
                            {token.is_active ? "Active" : "Inactive"}
                        </Badge>
                    </div>
                </div>
                <Button variant="outline" size="sm" onClick={handleCopy} className="gap-2">
                    <Copy className="h-3.5 w-3.5" />
                    {copied ? "Copied ID" : "Copy ID"}
                </Button>
            </div>

            {/* Usage Chart */}
            {usage && (
                <Card className="glass-card">
                    <CardHeader className="pb-2">
                        <CardTitle className="text-sm font-medium">24h Traffic Volume</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="h-[200px] w-full">
                            <ResponsiveContainer width="100%" height="100%">
                                <AreaChart data={usage.hourly}>
                                    <defs>
                                        <linearGradient id="colorCount" x1="0" y1="0" x2="0" y2="1">
                                            <stop offset="5%" stopColor="#6366f1" stopOpacity={0.3} />
                                            <stop offset="95%" stopColor="#6366f1" stopOpacity={0} />
                                        </linearGradient>
                                    </defs>
                                    <XAxis dataKey="bucket" hide />
                                    <YAxis hide />
                                    <Tooltip
                                        contentStyle={{ backgroundColor: 'rgba(0,0,0,0.8)', border: 'none', borderRadius: '8px', fontSize: '12px' }}
                                        itemStyle={{ color: '#fff' }}
                                        labelStyle={{ color: '#aaa' }}
                                        formatter={(value: any) => [value, "Requests"]}
                                        labelFormatter={(label: any) => new Date(label).toLocaleTimeString()}
                                    />
                                    <Area type="monotone" dataKey="count" stroke="#6366f1" strokeWidth={2} fillOpacity={1} fill="url(#colorCount)" />
                                </AreaChart>
                            </ResponsiveContainer>
                        </div>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mt-6">
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Total Requests</p>
                                <p className="text-xl font-semibold tabular-nums">{usage.total_requests.toLocaleString()}</p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Success Rate</p>
                                <p className="text-xl font-semibold tabular-nums text-emerald-500">
                                    {usage.total_requests > 0 ? ((usage.success_count / usage.total_requests) * 100).toFixed(1) : 0}%
                                </p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Avg Latency</p>
                                <p className="text-xl font-semibold tabular-nums text-amber-500">{Math.round(usage.avg_latency_ms)}ms</p>
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">Est. Cost</p>
                                <p className="text-xl font-semibold tabular-nums text-violet-500">${usage.total_cost_usd.toFixed(4)}</p>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Info Cards & Quick Start */}
            <div className="grid md:grid-cols-3 gap-3">
                <Card className="glass-card md:col-span-2">
                    <CardHeader className="pb-3 flex flex-row items-center justify-between">
                        <CardTitle className="text-sm font-medium">Quick Start Guide</CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div>
                            <p className="text-xs text-muted-foreground mb-2">cURL Example</p>
                            <div className="w-full bg-muted/80 rounded-md p-3 text-left font-mono text-xs relative group overflow-x-auto">
                                <div className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity">
                                    <Button size="icon" variant="ghost" className="h-6 w-6" onClick={() => {
                                        navigator.clipboard.writeText(`curl -X POST http://localhost:8443/v1/chat/completions \\
  -H "Authorization: Bearer ${token.id}" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Hello TrueFlow!"}]
  }'`);
                                        toast.success("Copied cURL");
                                    }}><Copy className="h-3 w-3" /></Button>
                                </div>
                                <span className="text-violet-400">curl</span> -X POST http://localhost:8443/v1/chat/completions \<br />
                                &nbsp;&nbsp;-H <span className="text-emerald-400">"Authorization: Bearer {token.id}"</span> \<br />
                                &nbsp;&nbsp;-H <span className="text-emerald-400">"Content-Type: application/json"</span> \<br />
                                &nbsp;&nbsp;-d <span className="text-amber-400">'{`\n    "model": "gpt-4o-mini",\n    "messages": [{"role": "user", "content": "Hello TrueFlow!"}]\n  `}'</span>
                            </div>
                        </div>
                        <div>
                            <p className="text-xs text-muted-foreground mb-2">Python SDK</p>
                            <div className="w-full bg-muted/80 rounded-md p-3 text-left font-mono text-xs relative group overflow-x-auto">
                                <div className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity">
                                    <Button size="icon" variant="ghost" className="h-6 w-6" onClick={() => {
                                        navigator.clipboard.writeText(`from trueflow import TrueFlowClient\n\nclient = TrueFlowClient(api_key="${token.id}")\noai = client.openai()\nresponse = oai.chat.completions.create(\n    model="gpt-4o-mini",\n    messages=[{"role": "user", "content": "Hello TrueFlow!"}]\n)\nprint(response.choices[0].message.content)`);
                                        toast.success("Copied Python snippet");
                                    }}><Copy className="h-3 w-3" /></Button>
                                </div>
                                <span className="text-rose-400">from</span> trueflow <span className="text-rose-400">import</span> TrueFlowClient<br /><br />
                                client = TrueFlowClient(api_key=<span className="text-emerald-400">"{token.id}"</span>)<br />
                                oai = client.openai()<br />
                                response = oai.chat.completions.create(<br />
                                &nbsp;&nbsp;&nbsp;&nbsp;model=<span className="text-emerald-400">"gpt-4o-mini"</span>,<br />
                                &nbsp;&nbsp;&nbsp;&nbsp;messages=[{`{`}<span className="text-emerald-400">"role"</span>: <span className="text-emerald-400">"user"</span>, <span className="text-emerald-400">"content"</span>: <span className="text-emerald-400">"Hello TrueFlow!"</span>{`}`}]<br />
                                )<br />
                                <span className="text-blue-400">print</span>(response.choices[0].message.content)
                            </div>
                        </div>
                    </CardContent>
                </Card>

                <div className="grid gap-3">
                    <Card className="glass-card">
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm font-medium">Configuration</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4 text-sm">
                            <div>
                                <p className="text-xs text-muted-foreground mb-1">Credential ID</p>
                                <p className="font-mono text-xs break-all text-blue-400">{token.credential_id}</p>
                            </div>
                            <div>
                                <p className="text-xs text-muted-foreground mb-1">Upstream URL</p>
                                <p className="font-mono text-xs break-all">{token.upstream_url}</p>
                            </div>
                            <div>
                                <p className="text-xs text-muted-foreground mb-1">Created At</p>
                                <p className="font-mono text-xs">{new Date(token.created_at).toLocaleString()}</p>
                            </div>
                            {token.team_id && (
                                <div>
                                    <p className="text-xs text-muted-foreground mb-1">Team</p>
                                    <p className="font-mono text-xs text-violet-400">{token.team_id}</p>
                                </div>
                            )}
                            {token.allowed_models && token.allowed_models.length > 0 && (
                                <div>
                                    <p className="text-xs text-muted-foreground mb-1">Allowed Models</p>
                                    <div className="flex flex-wrap gap-1">
                                        {token.allowed_models.map((m) => (
                                            <Badge key={m} variant="secondary" className="font-mono text-[10px]">{m}</Badge>
                                        ))}
                                    </div>
                                </div>
                            )}
                            {token.tags && Object.keys(token.tags).length > 0 && (
                                <div>
                                    <p className="text-xs text-muted-foreground mb-1">Tags</p>
                                    <div className="flex flex-wrap gap-1">
                                        {Object.entries(token.tags).map(([k, v]) => (
                                            <Badge key={k} variant="outline" className="font-mono text-[10px]">{k}: {v}</Badge>
                                        ))}
                                    </div>
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    <Card className="glass-card">
                        <CardHeader className="pb-3">
                            <CardTitle className="text-sm font-medium">Policies</CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div className="flex flex-wrap gap-2">
                                {token.policy_ids.length > 0 ? token.policy_ids.map(pid => (
                                    <Badge key={pid} variant="outline" className="font-mono text-[10px] break-all">
                                        {pid}
                                    </Badge>
                                )) : (
                                    <span className="text-[10px] bg-muted px-2 py-1 rounded text-muted-foreground italic">No policies attached</span>
                                )}
                            </div>
                        </CardContent>
                    </Card>
                </div>
            </div>

            {/* Budget & Spend Caps */}
            <Card className="glass-card">
                <CardHeader className="pb-3 flex flex-row items-center justify-between">
                    <div className="flex items-center gap-2">
                        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-violet-500/10">
                            <DollarSign className="h-4 w-4 text-violet-500" />
                        </div>
                        <CardTitle className="text-sm font-medium">Budget &amp; Spend Caps</CardTitle>
                    </div>
                    {spend && (spend.daily_limit_usd != null || spend.monthly_limit_usd != null) && (
                        <Badge variant="outline" className="text-emerald-500 border-emerald-500/30 bg-emerald-500/10 text-[10px]">
                            <TrendingUp className="h-3 w-3 mr-1" /> Active
                        </Badge>
                    )}
                </CardHeader>
                <CardContent className="space-y-6">
                    {(["daily", "monthly"] as const).map((period) => {
                        const limit = period === "daily" ? spend?.daily_limit_usd : spend?.monthly_limit_usd;
                        const current = period === "daily" ? (spend?.current_daily_usd ?? 0) : (spend?.current_monthly_usd ?? 0);
                        const pct = limit != null && limit > 0 ? Math.min((current / limit) * 100, 100) : 0;
                        const barColor = pct >= 100 ? "bg-rose-500" : pct >= 80 ? "bg-amber-500" : "bg-emerald-500";
                        return (
                            <div key={period} className="space-y-2">
                                <div className="flex items-center justify-between">
                                    <span className="text-xs font-medium capitalize">{period} Cap</span>
                                    {limit != null && (
                                        <div className="flex items-center gap-2">
                                            <span className={`text-xs font-mono ${pct >= 100 ? "text-rose-500" : pct >= 80 ? "text-amber-500" : "text-emerald-500"
                                                }`}>
                                                ${current.toFixed(4)} / ${limit.toFixed(2)}
                                            </span>
                                            <button
                                                onClick={() => handleRemoveCap(period)}
                                                className="text-muted-foreground hover:text-rose-500 transition-colors"
                                                title="Remove cap"
                                            >
                                                <X className="h-3 w-3" />
                                            </button>
                                        </div>
                                    )}
                                </div>
                                {limit != null && (
                                    <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
                                        <div
                                            className={`h-full rounded-full transition-all duration-500 ${barColor}`}
                                            style={{ width: `${pct}%` }}
                                        />
                                    </div>
                                )}
                                <div className="flex items-center gap-2">
                                    <span className="text-[10px] text-muted-foreground w-14">Limit ($)</span>
                                    <input
                                        type="number"
                                        min="0"
                                        step="0.01"
                                        placeholder={limit != null ? String(limit) : "No cap"}
                                        value={period === "daily" ? capInput.daily : capInput.monthly}
                                        onChange={(e) => setCapInput((prev) => ({ ...prev, [period]: e.target.value }))}
                                        className="flex-1 rounded-md border border-border bg-background px-3 py-1 text-xs font-mono focus:outline-none focus:ring-2 focus:ring-primary/40"
                                    />
                                    <button
                                        onClick={() => handleSaveCap(period)}
                                        disabled={savingCap === period}
                                        className="rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
                                    >
                                        {savingCap === period ? "Saving…" : "Save"}
                                    </button>
                                </div>
                            </div>
                        );
                    })}
                </CardContent>
            </Card>

            {/* Circuit Breaker */}
            <Card className="glass-card">
                <CardHeader className="pb-3 flex flex-row items-center justify-between">
                    <div className="flex items-center gap-2">
                        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-orange-500/10">
                            <CircuitBoard className="h-4 w-4 text-orange-500" />
                        </div>
                        <CardTitle className="text-sm font-medium">Circuit Breaker</CardTitle>
                    </div>
                    {cb && (
                        <button
                            onClick={() => setCbEdits(prev => ({ ...prev, enabled: !prev.enabled }))}
                            className="flex items-center gap-1 text-xs"
                        >
                            {cbEdits.enabled
                                ? <><ToggleRight className="h-5 w-5 text-emerald-500" /><span className="text-emerald-500">Enabled</span></>
                                : <><ToggleLeft className="h-5 w-5 text-muted-foreground" /><span className="text-muted-foreground">Disabled</span></>}
                        </button>
                    )}
                </CardHeader>
                <CardContent className="space-y-4">
                    {!cb ? (
                        <p className="text-xs text-muted-foreground italic">Circuit breaker config not available for this token.</p>
                    ) : (
                        <>
                            <p className="text-[11px] text-muted-foreground">
                                Automatically opens the circuit after <strong>{cbEdits.failure_threshold ?? cb.failure_threshold}</strong> consecutive failures,
                                then retries after <strong>{cbEdits.recovery_cooldown_secs ?? cb.recovery_cooldown_secs}s</strong>.
                                State is reflected in <code className="text-[10px] bg-muted px-1 rounded">X-TrueFlow-CB-State</code> response header.
                            </p>
                            <div className="grid md:grid-cols-3 gap-4">
                                <div className="space-y-1">
                                    <label className="text-[10px] text-muted-foreground font-medium uppercase tracking-wider">Failure Threshold</label>
                                    <input
                                        type="number" min={1} max={100}
                                        value={cbEdits.failure_threshold ?? cb.failure_threshold}
                                        onChange={e => setCbEdits(p => ({ ...p, failure_threshold: parseInt(e.target.value) || 1 }))}
                                        className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-primary/40"
                                    />
                                    <p className="text-[10px] text-muted-foreground">Failures before circuit opens</p>
                                </div>
                                <div className="space-y-1">
                                    <label className="text-[10px] text-muted-foreground font-medium uppercase tracking-wider">Cooldown (seconds)</label>
                                    <input
                                        type="number" min={1}
                                        value={cbEdits.recovery_cooldown_secs ?? cb.recovery_cooldown_secs}
                                        onChange={e => setCbEdits(p => ({ ...p, recovery_cooldown_secs: parseInt(e.target.value) || 1 }))}
                                        className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-primary/40"
                                    />
                                    <p className="text-[10px] text-muted-foreground">Wait before half-open try</p>
                                </div>
                                <div className="space-y-1">
                                    <label className="text-[10px] text-muted-foreground font-medium uppercase tracking-wider">Half-Open Max Requests</label>
                                    <input
                                        type="number" min={1}
                                        value={cbEdits.half_open_max_requests ?? cb.half_open_max_requests}
                                        onChange={e => setCbEdits(p => ({ ...p, half_open_max_requests: parseInt(e.target.value) || 1 }))}
                                        className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-primary/40"
                                    />
                                    <p className="text-[10px] text-muted-foreground">Probe requests in half-open</p>
                                </div>
                            </div>
                            <div className="flex justify-end">
                                <button
                                    onClick={handleSaveCb}
                                    disabled={savingCb}
                                    className="rounded-md bg-primary px-4 py-2 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
                                >
                                    {savingCb ? "Saving…" : "Save Changes"}
                                </button>
                            </div>
                        </>
                    )}
                </CardContent>
            </Card>

            {/* Recent Activity */}
            <div className="space-y-4">
                <div className="flex items-center justify-between">
                    <h2 className="text-lg font-semibold tracking-tight">Recent Activity</h2>
                    <div className="flex gap-2">
                        <Badge variant="outline" className="gap-1">
                            <Activity className="h-3 w-3" />
                            {logs.length} requests
                        </Badge>
                        <Badge variant="outline" className="gap-1 text-amber-500">
                            <Clock className="h-3 w-3" />
                            ~{avgLatency}ms avg
                        </Badge>
                        {errorCount > 0 && (
                            <Badge variant="outline" className="gap-1 text-rose-500">
                                <AlertTriangle className="h-3 w-3" />
                                {errorCount} errors
                            </Badge>
                        )}
                    </div>
                </div>

                <div className="rounded-md border border-border/60 bg-card/50 backdrop-blur-sm overflow-hidden">
                    <DataTable
                        columns={columns}
                        data={logs}
                        onRowClick={(log) => router.push(`/audit/${log.id}`)}
                    />
                </div>
            </div>
        </div>
    );
}
