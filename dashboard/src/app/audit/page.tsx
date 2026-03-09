"use client"

import { useEffect, useState, useCallback } from "react"
import { useRouter } from "next/navigation"
import useSWR, { mutate } from "swr"
import { swrFetcher, listTokens, streamAuditLogs, AuditLog, Token } from "@/lib/api"
import { cn } from "@/lib/utils"
import { DataTable } from "@/components/data-table"
import { columns } from "./columns"
import {
    Activity,
    Clock,
    DollarSign,
    Cpu,
    Search,
    Filter,
    X,
    Loader2,
    Play,
    Pause,
    RefreshCw
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { EmptyState } from "@/components/empty-state"
import { PageSkeleton } from "@/components/page-skeleton"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"

// ── Summary Card ────────────────────────────────

function StatCard({
    icon: Icon,
    label,
    value,
    sub,
    color,
    loading
}: {
    icon: React.ElementType
    label: string
    value: string
    sub?: string
    color: string
    loading?: boolean
}) {
    return (
        <Card className="bg-black border-white/10 hover:border-white/20 transition-colors">
            <CardContent className="p-4 flex items-center gap-4">
                <div className={cn("flex h-10 w-10 items-center justify-center rounded-lg border transition-colors", color)}>
                    <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                    <p className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest mb-1">{label}</p>
                    {loading ? (
                        <div className="h-7 w-24 bg-white/5 rounded my-0.5 animate-pulse" />
                    ) : (
                        <p className="text-xl font-semibold tabular-nums tracking-tight text-white">{value}</p>
                    )}
                    {sub && <p className="text-[10px] text-zinc-500 truncate">{sub}</p>}
                </div>
            </CardContent>
        </Card>
    )
}

// ── Page ──────────────────────────────────────────

const EMPTY_LOGS: AuditLog[] = [];
const EMPTY_TOKENS: Token[] = [];

export default function AuditPage() {
    const router = useRouter()
    const [isLive, setIsLive] = useState(false)
    const [selectedToken, setSelectedToken] = useState<string>("all")
    const [liveLogs, setLiveLogs] = useState<AuditLog[]>([])

    // Data Fetching
    const { data: tokensData } = useSWR<Token[]>("/tokens", swrFetcher)
    const tokens = tokensData || EMPTY_TOKENS;

    // Construct Query Key
    const queryKey = selectedToken && selectedToken !== "all"
        ? `/audit?limit=500&token_id=${selectedToken}`
        : `/audit?limit=500`;

    const { data: historicalLogsData, isLoading, mutate: refreshLogs } = useSWR<AuditLog[]>(
        !isLive ? queryKey : null, // Pause SWR when live
        swrFetcher,
        {
            keepPreviousData: true,
            revalidateOnFocus: false
        }
    )
    const historicalLogs = historicalLogsData || EMPTY_LOGS;

    // Combined Logs
    const logs = isLive ? liveLogs : historicalLogs;

    // Live Stream Effect
    useEffect(() => {
        let active = true;
        if (!isLive) return;

        // Reset to historical data when starting live mode to avoid empty flash
        if (active) {
            setTimeout(() => {
                if (active) setLiveLogs([...historicalLogs].slice(0, 50));
            }, 0);
        }

        const cleanup = streamAuditLogs((log) => {
            // Check filter
            if (selectedToken !== "all" && log.token_id !== selectedToken) return;

            if (active) {
                setLiveLogs(prev => {
                    const newLogs = [log, ...prev];
                    return newLogs.slice(0, 500); // Keep buffer capped
                });
            }
        });
        return () => {
            active = false;
            cleanup();
        };
    }, [isLive, selectedToken, historicalLogs]);

    const handleRowClick = useCallback((log: AuditLog) => {
        router.push(`/audit/${log.id}`)
    }, [router])

    // Summary stats
    const nonHitlLogs = logs.filter(l => !["approved", "rejected", "timeout"].includes(l.policy_result))
    const avgLatency = nonHitlLogs.length > 0
        ? Math.round(nonHitlLogs.reduce((s, l) => s + l.response_latency_ms, 0) / nonHitlLogs.length)
        : 0
    const totalCost = logs.reduce((s, l) => s + (l.estimated_cost_usd ? parseFloat(l.estimated_cost_usd) : 0), 0)
    const totalTokens = logs.reduce((s, l) => s + (l.prompt_tokens ?? 0) + (l.completion_tokens ?? 0), 0)

    const handleFilterChange = (val: string) => {
        setSelectedToken(val)
        // If live, we need to clear live buffer if distinct, or just let filter logic handle it
        if (isLive) setLiveLogs([]);
    }

    return (
        <div className="space-y-4">
            {/* Controls */}
            <div className="flex items-center justify-between animate-fade-in mb-2">
                <div>
                    <h1 className="text-lg font-semibold tracking-tight text-white">Audit Logs</h1>
                    <p className="text-xs text-zinc-500 mt-0.5">Real-time gateway traffic analysis and debugging.</p>
                </div>
                {/* Controls */}
                <div className="flex items-center gap-3">
                    {isLive && (
                        <Badge variant="outline" className="animate-pulse text-emerald-500 border-emerald-500/50 bg-emerald-500/10 mr-2">
                            LIVE
                        </Badge>
                    )}
                    {/* Live Toggle */}
                    <Button
                        variant={isLive ? "destructive" : "outline"}
                        size="sm"
                        onClick={() => setIsLive(!isLive)}
                        className={cn("gap-2 min-w-[100px] transition-all", !isLive && "bg-black border-white/10 text-zinc-400 hover:text-white hover:bg-white/5")}
                    >
                        {isLive ? <Pause className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
                        {isLive ? "Pause" : "Live View"}
                    </Button>

                    <div className="h-6 w-px bg-white/10" />

                    {/* Filter */}
                    <div className="flex items-center gap-2">
                        <div className="relative w-[240px]">
                            <Select
                                value={selectedToken}
                                onValueChange={(val) => handleFilterChange(val)}
                            >
                                <SelectTrigger className="pl-9 bg-black border-white/10 text-zinc-300">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent className="bg-zinc-950 border-white/10 text-zinc-300">
                                    <SelectItem value="all" className="focus:bg-white/5 focus:text-white">All Tokens</SelectItem>
                                    {tokens.map(t => (
                                        <SelectItem key={t.id} value={t.id} className="focus:bg-white/5 focus:text-white">{t.name}</SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                            <Filter className="absolute left-3 top-2.5 h-3.5 w-3.5 text-zinc-500 pointer-events-none" />
                        </div>
                        {selectedToken !== "all" && (
                            <Button
                                variant="ghost"
                                size="icon"
                                className="h-9 w-9 text-zinc-500 hover:text-white hover:bg-white/5"
                                onClick={() => handleFilterChange("all")}
                                title="Clear filter"
                            >
                                <X className="h-4 w-4" />
                            </Button>
                        )}
                    </div>

                    <div className="h-6 w-px bg-white/10" />

                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => refreshLogs()}
                        disabled={isLoading || isLive}
                        className="gap-2 bg-black border-white/10 text-zinc-400 hover:text-white hover:bg-white/5"
                    >
                        <RefreshCw className={cn("h-3.5 w-3.5", isLoading && "animate-spin")} />
                        Refresh
                    </Button>
                </div>
            </div>

            {/* KPIs */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4 animate-slide-up">
                <StatCard
                    icon={Activity}
                    label="Visible Requests"
                    value={logs.length.toLocaleString()}
                    color="bg-blue-500/10 text-blue-400 border-blue-500/20 group-hover:bg-blue-500/20"
                    loading={isLoading && !isLive}
                />
                <StatCard
                    icon={Clock}
                    label="Avg Latency"
                    value={`${avgLatency}ms`}
                    color="bg-emerald-500/10 text-emerald-400 border-emerald-500/20 group-hover:bg-emerald-500/20"
                    loading={isLoading && !isLive}
                />
                <StatCard
                    icon={DollarSign}
                    label="Total Cost"
                    value={`$${totalCost.toFixed(4)}`}
                    color="bg-amber-500/10 text-amber-400 border-amber-500/20 group-hover:bg-amber-500/20"
                    loading={isLoading && !isLive}
                />
                <StatCard
                    icon={Cpu}
                    label="Tokens Processed"
                    value={totalTokens.toLocaleString()}
                    color="bg-violet-500/10 text-violet-400 border-violet-500/20 group-hover:bg-violet-500/20"
                    loading={isLoading && !isLive}
                />
            </div>

            {/* Table */}
            <div className="animate-slide-up stagger-2">
                {isLoading && !isLive && logs.length === 0 ? (
                    <PageSkeleton cards={0} rows={10} />
                ) : logs.length === 0 ? (
                    <EmptyState
                        icon={Search}
                        title="No traces found"
                        description={selectedToken !== "all" ? "No logs match the current filter." : "Send your first request to the gateway to see it here."}
                        actionLabel={selectedToken !== "all" ? "Clear Filter" : undefined}
                        onAction={selectedToken !== "all" ? () => handleFilterChange("all") : undefined}
                        className="bg-black/50"
                    />
                ) : (
                    <div className="rounded-md border border-white/10 bg-black overflow-hidden shadow-sm">
                        <DataTable
                            columns={columns}
                            data={logs}
                            onRowClick={handleRowClick}
                            searchKey="path"
                            searchPlaceholder="Filter by path..."
                        />
                    </div>
                )}
            </div>
        </div>
    )
}
