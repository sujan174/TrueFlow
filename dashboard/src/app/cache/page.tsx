"use client";

import { useState, useCallback } from "react";
import useSWR from "swr";
import { getCacheStats, flushCache, CacheStats } from "@/lib/api";
import {
    Database,
    RefreshCw,
    Trash2,
    Zap,
    Clock,
    HardDrive,
    CheckCircle,
    XCircle,
    Info,
    AlertTriangle,
    BarChart2,
    Lock,
    Activity,
    Layers,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

// ── Helpers ────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatTTL(secs: number): string {
    if (secs < 0) return "Persistent";
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.round(secs / 60)}m`;
    return `${Math.round(secs / 3600)}h`;
}

// ── KPI Card ───────────────────────────────────────────────────

function KPICard({
    icon: Icon,
    label,
    value,
    sub,
    color,
    loading,
}: {
    icon: React.ComponentType<{ className?: string }>;
    label: string;
    value: string;
    sub?: string;
    color: string;
    loading?: boolean;
}) {
    return (
        <Card className="glass-card hover-lift">
            <CardContent className="p-4 flex items-center gap-4">
                <div className={cn("p-2.5 rounded-md flex-shrink-0", color)}>
                    <Icon className="h-5 w-5" />
                </div>
                <div className="min-w-0 flex-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-1">
                        {label}
                    </p>
                    {loading ? (
                        <div className="h-7 w-24 bg-muted/50 rounded shimmer my-0.5" />
                    ) : (
                        <p className="text-xl font-bold tabular-nums tracking-tight">{value}</p>
                    )}
                    {sub && <p className="text-[10px] text-muted-foreground truncate">{sub}</p>}
                </div>
            </CardContent>
        </Card>
    );
}

// ── Namespace Bar ──────────────────────────────────────────────

function NamespaceBar({
    label,
    count,
    total,
    color,
    icon: Icon,
    description,
}: {
    label: string;
    count: number;
    total: number;
    color: string;
    icon: React.ComponentType<{ className?: string }>;
    description: string;
}) {
    const pct = total > 0 ? Math.round((count / total) * 100) : 0;
    return (
        <div className="space-y-1.5">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <Icon className={cn("h-3.5 w-3.5", color)} />
                    <span className="text-sm font-medium">{label}</span>
                    <span className="text-xs text-muted-foreground">{description}</span>
                </div>
                <div className="flex items-center gap-2">
                    <span className={cn("text-sm font-mono font-bold", color)}>{count}</span>
                    <span className="text-xs text-muted-foreground">({pct}%)</span>
                </div>
            </div>
            <div className="w-full rounded-full h-1.5 bg-muted/50 overflow-hidden">
                <div
                    className="h-full rounded-full bg-current transition-all"
                    style={{ width: `${pct}%` }}
                />
            </div>
        </div>
    );
}

// ── Main Page ──────────────────────────────────────────────────

export default function CacheManagementPage() {
    const [flushing, setFlushing] = useState(false);

    const { data: stats, isLoading, mutate } = useSWR<CacheStats>(
        "/system/cache-stats",
        () => getCacheStats(),
        { refreshInterval: 15_000 }
    );

    const handleFlush = useCallback(async () => {
        if (!confirm("Flush all cached LLM responses? This will increase latency temporarily while caches warm up again.")) return;
        setFlushing(true);
        try {
            const result = await flushCache();
            toast.success(`Cache flushed — ${result.keys_deleted} entries removed`);
            await mutate();
        } catch (_e) {
            toast.error("Failed to flush cache");
        } finally {
            setFlushing(false);
        }
    }, [mutate]);

    const totalNs = stats
        ? stats.namespace_counts.llm_cache +
        stats.namespace_counts.spend_tracking +
        stats.namespace_counts.rate_limits
        : 0;

    const _utilizationPct =
        stats && stats.max_entry_bytes > 0
            ? Math.min(
                100,
                Math.round((stats.estimated_size_bytes / (stats.cache_key_count * stats.max_entry_bytes || 1)) * 100)
            )
            : 0;

    return (
        <div className="space-y-6 pb-10 animate-fade-in">
            {/* Controls */}
            <div className="flex items-center justify-end mb-2 gap-2">
                <Button
                    variant="outline"
                    size="sm"
                    onClick={() => mutate()}
                    disabled={isLoading}
                >
                    <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", isLoading && "animate-spin")} />
                    Refresh
                </Button>
                <Button
                    variant="destructive"
                    size="sm"
                    onClick={handleFlush}
                    disabled={flushing || isLoading}
                >
                    <Trash2 className="h-3.5 w-3.5 mr-1.5" />
                    {flushing ? "Flushing…" : "Flush Cache"}
                </Button>
            </div>

            {/* KPI Strip */}
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 animate-slide-up">
                <KPICard
                    icon={Database}
                    label="Cached Entries"
                    value={stats?.cache_key_count?.toLocaleString() ?? "—"}
                    sub="LLM response keys in Redis"
                    color="bg-blue-500/10 text-blue-500"
                    loading={isLoading}
                />
                <KPICard
                    icon={HardDrive}
                    label="Est. Memory Used"
                    value={stats ? formatBytes(stats.estimated_size_bytes) : "—"}
                    sub={`Max ${formatBytes(stats?.max_entry_bytes ?? 0)} / entry`}
                    color="bg-violet-500/10 text-violet-500"
                    loading={isLoading}
                />
                <KPICard
                    icon={Clock}
                    label="Default TTL"
                    value={stats ? formatTTL(stats.default_ttl_secs) : "—"}
                    sub="Per cached response"
                    color="bg-emerald-500/10 text-emerald-500"
                    loading={isLoading}
                />
                <KPICard
                    icon={Zap}
                    label="Avg Entry Size"
                    value={
                        stats && stats.cache_key_count > 0
                            ? formatBytes(Math.round(stats.estimated_size_bytes / stats.cache_key_count))
                            : "—"
                    }
                    sub="Bytes per cached response"
                    color="bg-amber-500/10 text-amber-500"
                    loading={isLoading}
                />
            </div>

            {/* Middle row: Namespace breakdown + Strategy */}
            <div className="grid gap-4 lg:grid-cols-2 animate-slide-up stagger-2">

                {/* Redis Namespace Breakdown */}
                <Card className="glass-card">
                    <CardHeader className="pb-3">
                        <div className="flex items-center gap-2">
                            <div className="flex h-8 w-8 items-center justify-center rounded-md bg-blue-500/10">
                                <Layers className="h-4 w-4 text-blue-500" />
                            </div>
                            <div>
                                <CardTitle className="text-sm">Redis Namespace Breakdown</CardTitle>
                                <CardDescription className="text-xs">Keys by namespace (live scan)</CardDescription>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        {isLoading ? (
                            <div className="space-y-3">
                                {[1, 2, 3].map(i => <div key={i} className="h-8 bg-muted/40 rounded shimmer" />)}
                            </div>
                        ) : (
                            <>
                                <NamespaceBar
                                    label="llm_cache"
                                    count={stats?.namespace_counts.llm_cache ?? 0}
                                    total={totalNs}
                                    color="text-blue-500"
                                    icon={Database}
                                    description="LLM response cache"
                                />
                                <NamespaceBar
                                    label="spend"
                                    count={stats?.namespace_counts.spend_tracking ?? 0}
                                    total={totalNs}
                                    color="text-amber-500"
                                    icon={BarChart2}
                                    description="Budget / spend tracking"
                                />
                                <NamespaceBar
                                    label="rl"
                                    count={stats?.namespace_counts.rate_limits ?? 0}
                                    total={totalNs}
                                    color="text-rose-500"
                                    icon={Activity}
                                    description="Rate limit counters"
                                />
                                <div className="pt-2 border-t border-border/40 flex items-center justify-between text-xs text-muted-foreground">
                                    <span>Total Redis keys (sampled)</span>
                                    <span className="font-mono font-semibold text-foreground">{totalNs}</span>
                                </div>
                                <div className="rounded-md border border-amber-500/20 bg-amber-500/5 p-3 flex gap-2 text-xs text-amber-700 dark:text-amber-400">
                                    <AlertTriangle className="h-3.5 w-3.5 flex-shrink-0 mt-0.5" />
                                    <span>
                                        <strong>Safe flush:</strong> Only <code className="font-mono bg-amber-500/10 px-0.5 rounded">llm_cache:*</code> keys are deleted when flushing.
                                        Spend tracking and rate-limit counters are preserved.
                                    </span>
                                </div>
                            </>
                        )}
                    </CardContent>
                </Card>

                {/* Cache Strategy */}
                <Card className="glass-card">
                    <CardHeader className="pb-3">
                        <div className="flex items-center gap-2">
                            <div className="flex h-8 w-8 items-center justify-center rounded-md bg-emerald-500/10">
                                <Info className="h-4 w-4 text-emerald-500" />
                            </div>
                            <div>
                                <CardTitle className="text-sm">Cache Strategy</CardTitle>
                                <CardDescription className="text-xs">How TrueFlow caches LLM responses</CardDescription>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        {/* Cached Fields */}
                        <div>
                            <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-2 flex items-center gap-2">
                                <CheckCircle className="h-3 w-3 text-emerald-500" /> Key Fields (cache key hash)
                            </p>
                            <div className="flex flex-wrap gap-2">
                                {(stats?.cached_fields ?? ["model", "messages", "temperature", "max_tokens", "tools", "tool_choice"]).map(f => (
                                    <Badge key={f} variant="secondary" className="font-mono text-[10px]">
                                        {f}
                                    </Badge>
                                ))}
                            </div>
                        </div>

                        {/* Skip Conditions */}
                        <div>
                            <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-2 flex items-center gap-2">
                                <XCircle className="h-3 w-3 text-rose-500" /> Skip Conditions (no caching)
                            </p>
                            <div className="space-y-1.5">
                                {(stats?.skip_conditions ?? []).map(c => (
                                    <div key={c} className="flex items-center gap-2 text-xs text-muted-foreground">
                                        <div className="h-1.5 w-1.5 rounded-full bg-rose-500/60 flex-shrink-0" />
                                        <code className="font-mono text-[11px]">{c}</code>
                                    </div>
                                ))}
                            </div>
                        </div>

                        {/* How it works */}
                        <div className="rounded-md border border-border/40 bg-muted/20 p-3 text-xs text-muted-foreground space-y-1">
                            <p><strong className="text-foreground">Key generation:</strong> SHA-256 hash of <code className="font-mono text-[11px]">token_id + canonical_request_fields</code></p>
                            <p><strong className="text-foreground">Storage:</strong> Redis (tiered: in-process → Redis)</p>
                            <p><strong className="text-foreground">Bypass header:</strong> <code className="font-mono text-[11px]">x-trueflow-no-cache: true</code></p>
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Sample Entries Table */}
            <Card className="glass-card animate-slide-up stagger-3">
                <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <div className="flex h-8 w-8 items-center justify-center rounded-md bg-violet-500/10">
                                <Lock className="h-4 w-4 text-violet-500" />
                            </div>
                            <div>
                                <CardTitle className="text-sm">Cached Entry Sample</CardTitle>
                                <CardDescription className="text-xs">
                                    Up to 20 live cache keys with TTL and size info
                                </CardDescription>
                            </div>
                        </div>
                        <Badge variant="outline" className="text-xs font-mono">
                            {stats?.cache_key_count ?? 0} total
                        </Badge>
                    </div>
                </CardHeader>
                <CardContent>
                    {isLoading ? (
                        <div className="space-y-2">
                            {Array.from({ length: 5 }).map((_, i) => (
                                <div key={i} className="h-10 bg-muted/40 rounded shimmer" />
                            ))}
                        </div>
                    ) : !stats || stats.sample_entries.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-12 gap-3 text-center">
                            <div className="flex h-12 w-12 items-center justify-center rounded-md bg-muted">
                                <Database className="h-6 w-6 text-muted-foreground" />
                            </div>
                            <p className="text-sm font-medium">No cached entries</p>
                            <p className="text-xs text-muted-foreground max-w-xs">
                                The cache is empty. Send some deterministic requests (temperature ≤ 0.1, non-streaming) to populate it.
                            </p>
                        </div>
                    ) : (
                        <div className="overflow-x-auto">
                            <table className="w-full text-xs">
                                <thead>
                                    <tr className="border-b border-border/50">
                                        <th className="text-left font-medium text-muted-foreground uppercase tracking-wider pb-2 pr-4">Cache Key</th>
                                        <th className="text-right font-medium text-muted-foreground uppercase tracking-wider pb-2 pr-4">Size</th>
                                        <th className="text-right font-medium text-muted-foreground uppercase tracking-wider pb-2">TTL</th>
                                    </tr>
                                </thead>
                                <tbody className="divide-y divide-border/30">
                                    {stats.sample_entries.map((entry, i) => (
                                        <tr key={i} className="hover:bg-muted/20 transition-colors group">
                                            <td className="py-3 pr-4">
                                                <code className="font-mono text-[11px] text-muted-foreground group-hover:text-foreground transition-colors">
                                                    {entry.key}
                                                </code>
                                            </td>
                                            <td className="py-3 pr-4 text-right">
                                                <span className="font-mono text-violet-400">
                                                    {formatBytes(entry.size_bytes)}
                                                </span>
                                            </td>
                                            <td className="py-3 text-right">
                                                <Badge
                                                    variant="outline"
                                                    className={cn(
                                                        "text-[10px] font-mono",
                                                        entry.ttl_secs > 0 && entry.ttl_secs < 60
                                                            ? "border-amber-500/40 text-amber-500"
                                                            : entry.ttl_secs < 0
                                                                ? "border-rose-500/40 text-rose-500"
                                                                : "border-emerald-500/40 text-emerald-500"
                                                    )}
                                                >
                                                    {formatTTL(entry.ttl_secs)}
                                                </Badge>
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </CardContent>
            </Card>

            {/* How Caching Helps */}
            <div className="grid gap-4 sm:grid-cols-3 animate-slide-up stagger-4">
                {[
                    {
                        icon: Zap,
                        color: "text-amber-500 bg-amber-500/10",
                        title: "Cost Reduction",
                        desc: "Identical prompts hit the cache instead of the LLM provider, eliminating redundant API costs."
                    },
                    {
                        icon: Clock,
                        color: "text-blue-500 bg-blue-500/10",
                        title: "Latency Improvement",
                        desc: "Cache hits return in <1ms vs. 500–5000ms for live LLM calls. Agents feel dramatically faster."
                    },
                    {
                        icon: CheckCircle,
                        color: "text-emerald-500 bg-emerald-500/10",
                        title: "Deterministic Outputs",
                        desc: "Same inputs always return the same output, making AI agent behavior predictable and auditable."
                    }
                ].map(({ icon: Icon, color, title, desc }) => (
                    <Card key={title} className="glass-card">
                        <CardContent className="p-4 flex gap-3">
                            <div className={cn("flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md", color)}>
                                <Icon className="h-4 w-4" />
                            </div>
                            <div>
                                <p className="text-sm font-semibold mb-1">{title}</p>
                                <p className="text-xs text-muted-foreground leading-relaxed">{desc}</p>
                            </div>
                        </CardContent>
                    </Card>
                ))}
            </div>
        </div>
    );
}
