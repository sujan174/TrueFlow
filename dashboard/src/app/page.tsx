"use client";

import { useState, useEffect } from "react";
import useSWR from "swr";
import { swrFetcher, AuditLog, Token, ApprovalRequest, AnalyticsTimeseriesPoint, AnomalyResponse } from "@/lib/api";
import {
    Activity,
    ArrowUpRight,
    TrendingUp,
    TrendingDown,
    AlertTriangle,
    Loader2,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import Link from "next/link";
import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis, CartesianGrid } from "recharts";
import { CustomTooltip, CHART_AXIS_PROPS } from "@/components/ui/chart-utils";
import { CountUp } from "@/components/ui/count-up";

type Credential = { id: string };

export default function OverviewPage() {
    const { data: logs = [], isLoading: logsLoading } = useSWR<AuditLog[]>("/audit?limit=100", swrFetcher, { refreshInterval: 5000 });
    const { data: tokens = [], isLoading: tokensLoading } = useSWR<Token[]>("/tokens", swrFetcher);
    const { data: credentials = [], isLoading: credentialsLoading } = useSWR<Credential[]>("/credentials", swrFetcher);
    const { data: approvals = [], isLoading: approvalsLoading } = useSWR<ApprovalRequest[]>("/approvals", swrFetcher, { refreshInterval: 10000 });
    const { data: usage, isLoading: usageLoading } = useSWR<Record<string, number | string>>("/billing/usage", swrFetcher, { refreshInterval: 10000 });
    const { data: latencySeries = [], isLoading: latencyLoading } = useSWR<AnalyticsTimeseriesPoint[]>("/analytics/timeseries?range=168", swrFetcher, { refreshInterval: 10000 });
    const { data: anomalyData } = useSWR<AnomalyResponse>("/anomalies", swrFetcher, { refreshInterval: 15000 });

    const loading = logsLoading || tokensLoading || credentialsLoading || approvalsLoading || usageLoading || latencyLoading;

    // Computed metrics
    const totalRequests = usage ? Number(usage.total_requests || 0) : 0;
    const avgLatency = logs.length > 0
        ? Math.round(logs.reduce((sum, l) => sum + l.response_latency_ms, 0) / logs.length)
        : 0;
    const activeTokens = tokens.filter(t => t.is_active).length;
    const totalSpend = usage ? Number(usage.total_spend_usd || 0) : 0;
    const pendingApprovals = approvals.filter(a => a.status === "pending").length;
    const successRate = logs.length > 0
        ? Math.round((logs.filter(l => l.upstream_status && l.upstream_status < 400).length / logs.length) * 100)
        : 0;

    const recent5xxErrors = logs.filter(l => l.upstream_status && l.upstream_status >= 500).length;
    let alertMessage = null;
    if (logs.length > 0) {
        if (recent5xxErrors > 0) {
            alertMessage = `${recent5xxErrors} provider errors (5xx) detected in recent traffic`;
        } else if (successRate < 98 && logs.length > 10) {
            alertMessage = `Success rate dropped to ${successRate}%`;
        }
    }

    const formatDate = (dateStr: string | number) => {
        const date = new Date(dateStr);
        return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
    };

    const recentLogs = logs.slice(0, 8);
    const anomalousCount = anomalyData?.events?.filter(e => e.is_anomalous).length ?? 0;

    return (
        <div className="space-y-8 max-w-[1440px] mx-auto pb-12">
            {/* Page header */}
            <div className="flex items-center justify-between animate-fade-in">
                <div>
                    <h1 className="text-2xl font-semibold tracking-tight text-white">Dashboard</h1>
                    <p className="text-sm text-zinc-400 mt-1">Real-time gateway overview</p>
                </div>
                {!loading && (
                    <div className="flex items-center gap-2 text-[11px] text-zinc-500 font-mono tracking-widest uppercase bg-white/5 px-2.5 py-1 rounded-full border border-white/5 shadow-inner">
                        <div className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.5)]" />
                        Live
                    </div>
                )}
            </div>

            {/* ── KPI Strip ── */}
            <div className="grid gap-4 grid-cols-2 lg:grid-cols-4">
                <MetricCard
                    label="Requests"
                    value={totalRequests}
                    sub="this month"
                    loading={loading}
                    delay="stagger-1"
                />
                <MetricCard
                    label="Active Tokens"
                    value={activeTokens}
                    sub={`${tokens.length} total`}
                    loading={loading}
                    delay="stagger-2"
                />
                <MetricCard
                    label="Avg Latency"
                    value={avgLatency}
                    suffix="ms"
                    sub={avgLatency < 200 ? "excellent" : avgLatency < 500 ? "good" : "high"}
                    loading={loading}
                    trend={avgLatency > 0 ? (avgLatency < 300 ? "up" : "down") : undefined}
                    delay="stagger-3"
                />
                <MetricCard
                    label="Spend"
                    value={totalSpend}
                    prefix="$"
                    decimals={4}
                    sub="this month"
                    loading={loading}
                    delay="stagger-4"
                />
            </div>

            {/* ── Status Strip ── */}
            <div className="grid gap-4 grid-cols-1 md:grid-cols-3">
                {/* Anomalies — lead with problems */}
                <div className="bg-black border border-white/10 rounded-lg px-5 py-4 flex items-center gap-4 group animate-slide-up stagger-2">
                    <div className="flex-1">
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest font-medium mb-1">Anomalies</p>
                        <p className={cn(
                            "text-2xl font-mono tracking-tighter",
                            anomalousCount > 0 ? "text-rose-400" : "text-white"
                        )}>
                            <CountUp value={anomalousCount} duration={800} />
                        </p>
                    </div>
                    {anomalousCount > 0 && (
                        <div className="h-2 w-2 rounded-full bg-rose-500 animate-pulse shadow-[0_0_8px_rgba(244,63,94,0.5)]" />
                    )}
                </div>

                {/* Pending Approvals */}
                <Link href="/approvals" className="bg-black border border-white/10 rounded-lg px-5 py-4 flex items-center gap-4 hover:border-white/20 hover:bg-white/5 transition-all group animate-slide-up stagger-3">
                    <div className="flex-1">
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest font-medium mb-1">Pending Approvals</p>
                        <p className="text-2xl font-mono tracking-tighter text-white">
                            {loading ? "—" : <CountUp value={pendingApprovals} duration={800} />}
                        </p>
                    </div>
                    {pendingApprovals > 0 && (
                        <span className="text-[11px] text-amber-400 flex items-center gap-1 opacity-100 md:opacity-0 md:group-hover:opacity-100 transition-opacity uppercase tracking-widest">
                            Review <ArrowUpRight className="h-3 w-3" />
                        </span>
                    )}
                </Link>

                {/* Success Rate */}
                <div className="bg-black border border-white/10 rounded-lg px-5 py-4 flex items-center gap-4 animate-slide-up stagger-4">
                    <div className="flex-1">
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest font-medium mb-1">Success Rate</p>
                        <p className={cn(
                            "text-2xl font-mono tracking-tighter",
                            successRate >= 95 ? "text-emerald-400" : successRate >= 80 ? "text-amber-400" : "text-rose-400"
                        )}>
                            {loading ? "—" : <CountUp value={successRate} suffix="%" duration={1200} />}
                        </p>
                    </div>
                    <div className="h-1 flex-1 rounded-full bg-white/5 overflow-hidden shadow-inner">
                        <div
                            className={cn("h-full rounded-full transition-all duration-1000 ease-out",
                                successRate >= 95 ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]" : successRate >= 80 ? "bg-amber-500" : "bg-rose-500"
                            )}
                            style={{ width: `${successRate}%` }}
                        />
                    </div>
                </div>
            </div>

            {/* ── Alert Banner ── */}
            {!loading && alertMessage && (
                <div className="rounded-lg border border-rose-500/20 bg-rose-500/10 px-5 py-4 animate-fade-in flex items-center gap-4 text-sm backdrop-blur-md">
                    <AlertTriangle className="h-5 w-5 text-rose-400 shrink-0" />
                    <span className="text-rose-300 font-medium">{alertMessage}</span>
                </div>
            )}

            {/* ── Latency Chart (full width) ── */}
            <div className="bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-4">
                <div className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between">
                    <h3 className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">
                        Latency Trend
                    </h3>
                    <span className="text-[10px] text-zinc-500 font-mono tracking-widest uppercase">7 days</span>
                </div>
                <div className="p-5 min-h-[260px]">
                    {loading ? (
                        <div className="h-[220px] w-full flex items-center justify-center">
                            <Loader2 className="h-6 w-6 animate-spin text-zinc-600" />
                        </div>
                    ) : latencySeries.length > 0 ? (
                        <div className="h-[200px] md:h-[240px] w-full">
                            <ResponsiveContainer width="100%" height="100%">
                                <AreaChart data={latencySeries} margin={{ top: 8, right: 8, left: -24, bottom: 0 }}>
                                    <defs>
                                        <linearGradient id="colorLatency" x1="0" y1="0" x2="0" y2="1">
                                            <stop offset="5%" stopColor="#ffffff" stopOpacity={0.15} />
                                            <stop offset="95%" stopColor="#ffffff" stopOpacity={0} />
                                        </linearGradient>
                                    </defs>
                                    <CartesianGrid stroke="rgba(255,255,255,0.05)" strokeDasharray="4 4" vertical={false} />
                                    <XAxis
                                        dataKey="bucket"
                                        tickFormatter={formatDate}
                                        {...CHART_AXIS_PROPS}
                                        stroke="rgba(255,255,255,0.2)"
                                        tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                        minTickGap={40}
                                    />
                                    <YAxis
                                        domain={[0, 'auto']}
                                        tickFormatter={(val: number) => `${val}ms`}
                                        {...CHART_AXIS_PROPS}
                                        stroke="rgba(255,255,255,0.2)"
                                        tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                    />
                                    <Tooltip
                                        content={<CustomTooltip
                                            labelFormatter={formatDate}
                                            valueFormatter={(val: number | string) => `${val}ms`}
                                        />}
                                        cursor={{ stroke: 'rgba(255,255,255,0.15)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                    />
                                    <Area
                                        type="monotone"
                                        dataKey="avg_latency_ms"
                                        name="Latency"
                                        stroke="#ffffff"
                                        strokeWidth={1.5}
                                        fillOpacity={1}
                                        fill="url(#colorLatency)"
                                        isAnimationActive={false}
                                        activeDot={{ r: 4, strokeWidth: 0, fill: '#ffffff', filter: 'drop-shadow(0 0 4px rgba(255,255,255,0.5))' }}
                                    />
                                </AreaChart>
                            </ResponsiveContainer>
                        </div>
                    ) : (
                        <div className="h-[220px] w-full flex items-center justify-center text-[13px] text-zinc-500">
                            <div className="flex flex-col items-center gap-3">
                                <Activity className="h-6 w-6 opacity-20" />
                                <span>No latency data yet</span>
                            </div>
                        </div>
                    )}
                </div>
            </div>

            {/* ── Recent Activity ── */}
            <div className="bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-5">
                <div className="px-5 py-3 border-b border-white/10 flex flex-row items-center justify-between">
                    <h3 className="text-[11px] font-medium text-zinc-400 uppercase tracking-widest">
                        Recent Traces
                    </h3>
                    <Link href="/audit">
                        <Button variant="ghost" size="sm" className="h-7 text-[11px] uppercase tracking-widest text-zinc-400 hover:text-white gap-1.5">
                            View all <ArrowUpRight className="h-3 w-3" />
                        </Button>
                    </Link>
                </div>
                <div className="max-h-[360px] overflow-y-auto scrollbar-none">
                    {loading ? (
                        <div className="divide-y divide-white/5">
                            {Array.from({ length: 5 }).map((_, i) => (
                                <div key={i} className="flex items-center gap-4 px-5 py-4">
                                    <div className="h-1.5 w-1.5 rounded-full bg-white/5 shimmer" />
                                    <div className="flex-1 space-y-1">
                                        <div className="h-3 w-40 bg-white/5 rounded shimmer" />
                                    </div>
                                    <div className="h-3 w-12 bg-white/5 rounded shimmer" />
                                </div>
                            ))}
                        </div>
                    ) : recentLogs.length === 0 ? (
                        <div className="text-center py-12 text-zinc-500 flex flex-col items-center">
                            <Activity className="h-6 w-6 mb-3 opacity-20" />
                            <p className="text-[13px]">No activity yet. Send a request to see it here.</p>
                        </div>
                    ) : (
                        <div className="divide-y divide-white/5">
                            {recentLogs.map((log) => (
                                <div
                                    key={log.id}
                                    className="flex items-center gap-4 px-5 py-3.5 hover:bg-white/[0.02] transition-colors group"
                                >
                                    <StatusDot status={log.upstream_status} result={log.policy_result} />

                                    {/* Desktop: single-row grid */}
                                    <div className="hidden md:grid flex-1 min-w-0 grid-cols-4 gap-4 items-center">
                                        <div className="col-span-2 font-mono text-[13px] truncate text-white/80">
                                            <span className="font-semibold text-zinc-500 mr-2">{log.method}</span>
                                            {log.path}
                                        </div>
                                        <div className="col-span-1 text-[13px] text-zinc-500 truncate group-hover:text-zinc-300 transition-colors">
                                            {log.agent_name || "—"}
                                        </div>
                                        <div className="col-span-1 text-right font-mono text-[13px] text-zinc-400 tabular-nums">
                                            {log.response_latency_ms}ms
                                        </div>
                                    </div>

                                    <div className="hidden md:flex items-center gap-4 min-w-[120px] justify-end">
                                        {log.estimated_cost_usd && parseFloat(log.estimated_cost_usd) > 0 && (
                                            <span className="text-[11px] font-mono text-zinc-500">
                                                ${parseFloat(log.estimated_cost_usd).toFixed(5)}
                                            </span>
                                        )}
                                        <span className="text-[11px] text-zinc-600 w-14 text-right tabular-nums font-mono">
                                            {new Date(log.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                        </span>
                                    </div>

                                    {/* Mobile: two-line layout */}
                                    <div className="flex-1 min-w-0 md:hidden">
                                        <div className="flex items-center justify-between gap-2">
                                            <span className="font-mono text-[13px] truncate text-white/80">
                                                <span className="font-semibold text-zinc-500 mr-1.5">{log.method}</span>
                                                {log.path}
                                            </span>
                                            <span className="text-[11px] text-zinc-600 tabular-nums font-mono shrink-0">
                                                {new Date(log.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                            </span>
                                        </div>
                                        <div className="flex items-center justify-between gap-2 mt-1">
                                            <span className="text-[12px] text-zinc-500 truncate">
                                                {log.agent_name || "—"}
                                            </span>
                                            <div className="flex items-center gap-3 shrink-0">
                                                <span className="text-[12px] font-mono text-zinc-400 tabular-nums">
                                                    {log.response_latency_ms}ms
                                                </span>
                                                {log.estimated_cost_usd && parseFloat(log.estimated_cost_usd) > 0 && (
                                                    <span className="text-[11px] font-mono text-zinc-500">
                                                        ${parseFloat(log.estimated_cost_usd).toFixed(5)}
                                                    </span>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}

// ── Sub-components ──────────────────────────────

function MetricCard({
    label,
    value,
    sub,
    prefix,
    suffix,
    decimals = 0,
    loading,
    trend,
    delay = "",
}: {
    label: string;
    value: number;
    sub: string;
    prefix?: string;
    suffix?: string;
    decimals?: number;
    loading?: boolean;
    trend?: "up" | "down";
    delay?: string;
}) {
    return (
        <div className={cn("bg-black border border-white/10 rounded-lg p-5 animate-slide-up hover:border-white/20 transition-all group relative overflow-hidden", delay)}>
            {/* Extremely subtle corner gradient for depth */}
            <div className="absolute top-0 right-0 w-32 h-32 bg-white/[0.02] rounded-full blur-3xl -translate-y-1/2 translate-x-1/2 group-hover:bg-white/[0.04] transition-colors" />

            <p className="text-[11px] text-zinc-500 uppercase tracking-widest font-medium relative z-10">
                {label}
            </p>
            {loading ? (
                <div className="h-9 w-24 bg-white/5 rounded shimmer mt-2 relative z-10" />
            ) : (
                <div className="flex items-baseline gap-2 mt-2 relative z-10">
                    <div className="text-[32px] leading-none text-white tracking-tighter">
                        <CountUp
                            value={value}
                            duration={1200}
                            decimals={decimals}
                            prefix={prefix}
                            suffix={suffix}
                        />
                    </div>
                    {trend && (
                        <span className={cn("flex items-center gap-0.5 text-[10px] font-medium tracking-widest uppercase",
                            trend === "up" ? "text-emerald-400" : "text-rose-400"
                        )}>
                            {trend === "up" ? <TrendingUp className="h-3 w-3" /> : <TrendingDown className="h-3 w-3" />}
                        </span>
                    )}
                </div>
            )}
            <p className="text-[11px] text-zinc-600 mt-2 relative z-10 font-medium">{sub}</p>
        </div>
    );
}

function StatusDot({ status, result }: { status: number | null; result: string }) {
    if (result === "blocked") {
        return <div className="h-1.5 w-1.5 rounded-full bg-rose-500 shrink-0 shadow-[0_0_8px_rgba(244,63,94,0.4)]" />;
    }
    if (result === "shadow_violation") {
        return <div className="h-1.5 w-1.5 rounded-full bg-amber-500 shrink-0 shadow-[0_0_8px_rgba(245,158,11,0.4)]" />;
    }
    if (status && status >= 200 && status < 400) {
        return <div className="h-1.5 w-1.5 rounded-full bg-emerald-500 shrink-0 shadow-[0_0_8px_rgba(16,185,129,0.3)]" />;
    }
    return <div className="h-1.5 w-1.5 rounded-full bg-zinc-600 shrink-0" />;
}
