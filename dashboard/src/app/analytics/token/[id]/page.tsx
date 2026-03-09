"use client";

import { useState, useEffect, useCallback } from "react";
import { getTokenVolume, getTokenStatus, getTokenLatency, TokenVolume, TokenStatus, TokenLatency, listTokens, Token } from "@/lib/api";
import {
    RefreshCw,
    TrendingUp,
    Activity,
    Zap,
    ArrowUpRight,
    ArrowDownRight,
    ArrowLeft
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell, CartesianGrid } from "recharts";
import { CustomTooltip, CHART_AXIS_PROPS } from "@/components/ui/chart-utils";
import { useRouter } from "next/navigation";
import { PageSkeleton } from "@/components/page-skeleton";

export default function TokenAnalyticsPage({ params }: { params: { id: string } }) {
    const router = useRouter();
    const [volume, setVolume] = useState<TokenVolume[]>([]);
    const [statusData, setStatusData] = useState<TokenStatus[]>([]);
    const [latency, setLatency] = useState<TokenLatency | null>(null);
    const [token, setToken] = useState<Token | null>(null); // For name etc
    const [loading, setLoading] = useState(true);

    const fetchData = useCallback(async () => {
        try {
            setLoading(true);
            const [volData, statData, latData, tokens] = await Promise.all([
                getTokenVolume(params.id),
                getTokenStatus(params.id),
                getTokenLatency(params.id),
                listTokens(), // Inefficient but needed to get name if not passed
            ]);

            setVolume(volData.reverse()); // Assume server returns desc? older first for charts
            // Actually server returns desc usually? chart needs newer last.
            // If server returns latest first, reverse.

            setStatusData(statData);
            setLatency(latData);
            setToken(tokens.find(t => t.id === params.id) || null);
        } catch {
            toast.error("Failed to load analytics data");
        } finally {
            setLoading(false);
        }
    }, [params.id]);

    useEffect(() => {
        fetchData();
    }, [fetchData]);

    if (loading && !token) {
        return <PageSkeleton />;
    }

    const pieData = statusData.map(d => ({
        name: d.status === 0 ? "Unknown" : `${d.status}s`, // Group by status code? or ranges?
        // Server returns status code itself.
        // Let's bucket them
        value: d.count,
        color: d.status >= 200 && d.status < 300 ? "#818cf8" :
            d.status >= 300 && d.status < 400 ? "#d4a574" :
                d.status >= 400 && d.status < 500 ? "#c47a50" :
                    d.status >= 500 ? "#6366f1" : "#6b7280"
    }));
    // Aggregate by bucket?
    // Actually server implementation returns distinct status codes? Yes.
    // Let's aggregate for pie chart
    const statusBuckets = statusData.reduce((acc, curr) => {
        const key = curr.status === 0 ? "Unknown" : `${Math.floor(curr.status / 100)}xx`;
        acc[key] = (acc[key] || 0) + curr.count;
        return acc;
    }, {} as Record<string, number>);

    const finalPieData = Object.entries(statusBuckets).map(([name, value]) => ({
        name,
        value,
        color: name === "2xx" ? "#818cf8" :
            name === "3xx" ? "#d4a574" :
                name === "4xx" ? "#c47a50" :
                    name === "5xx" ? "#6366f1" : "#6b7280"
    }));

    return (
        <div className="p-4 space-y-6 max-w-[1600px] mx-auto">
            {/* Header */}
            <div className="flex items-center justify-between animate-fade-in">
                <div className="space-y-1">
                    <div className="flex items-center gap-2">
                        <Button variant="ghost" size="icon" onClick={() => router.back()}>
                            <ArrowLeft className="h-4 w-4" />
                        </Button>
                        <h2 className="text-lg font-semibold tracking-tight">Token Analytics</h2>
                    </div>
                    <p className="text-muted-foreground ml-10">
                        Performance metrics for <code className="bg-muted px-1.5 py-0.5 rounded text-xs">{params.id}</code>
                        {token && <span className="ml-2 font-medium text-foreground">({token.name})</span>}
                    </p>
                </div>
                <Button variant="outline" size="sm" onClick={fetchData} disabled={loading}>
                    <RefreshCw className={cn("h-4 w-4 mr-2", loading && "animate-spin")} />
                    Refresh
                </Button>
            </div>

            {/* KPI Cards */}
            <div className="grid gap-4 md:grid-cols-3 animate-fade-in duration-500">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Total Volume (24h)</CardTitle>
                        <Activity className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">{volume.reduce((acc, v) => acc + v.count, 0)}</div>
                        <p className="text-xs text-muted-foreground">Requests in last 24 hours</p>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Avg Latency (P50)</CardTitle>
                        <Zap className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">{Math.round(latency?.p50 || 0)}ms</div>
                        <p className="text-xs text-muted-foreground">Detailed: P90 {Math.round(latency?.p90 || 0)}ms, P99 {Math.round(latency?.p99 || 0)}ms</p>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Error Rate</CardTitle>
                        <TrendingUp className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">
                            {(() => {
                                const total = statusData.reduce((acc, s) => acc + s.count, 0);
                                const errors = statusData.filter(s => s.status >= 400).reduce((acc, s) => acc + s.count, 0);
                                return total > 0 ? ((errors / total) * 100).toFixed(1) + "%" : "0%";
                            })()}
                        </div>
                        <p className="text-xs text-muted-foreground">Responses with 4xx/5xx status</p>
                    </CardContent>
                </Card>
            </div>

            {/* Charts Row */}
            <div className="grid gap-4 md:grid-cols-2 animate-fade-in duration-700">
                {/* Volume Chart */}
                <Card>
                    <CardHeader>
                        <CardTitle>Request Volume (Hourly)</CardTitle>
                    </CardHeader>
                    <CardContent className="h-[300px]">
                        {volume && volume.length > 0 ? (
                            <ResponsiveContainer width="100%" height="100%">
                                <AreaChart data={volume}>
                                    <defs>
                                        <linearGradient id="volGradient" x1="0" y1="0" x2="0" y2="1">
                                            <stop offset="5%" stopColor="#6366f1" stopOpacity={0.3} />
                                            <stop offset="95%" stopColor="#6366f1" stopOpacity={0} />
                                        </linearGradient>
                                    </defs>
                                    <CartesianGrid stroke="var(--border, #1e2330)" strokeDasharray="3 3" vertical={false} />
                                    <XAxis
                                        dataKey="hour"
                                        {...CHART_AXIS_PROPS}
                                        tickFormatter={(val) => new Date(val).getHours() + 'h'}
                                    />
                                    <YAxis {...CHART_AXIS_PROPS} />
                                    <Tooltip
                                        content={<CustomTooltip
                                            labelFormatter={(label: string | number) => new Date(label).toLocaleString()}
                                        />}
                                        cursor={{ stroke: 'var(--border)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                    />
                                    <Area
                                        type="monotone"
                                        dataKey="count"
                                        name="Requests"
                                        stroke="#6366f1"
                                        strokeWidth={2}
                                        fill="url(#volGradient)"
                                        activeDot={{ r: 4, strokeWidth: 0, fill: '#6366f1' }}
                                    />
                                </AreaChart>
                            </ResponsiveContainer>
                        ) : (
                            <div className="h-full w-full flex items-center justify-center text-[13px] text-muted-foreground border border-dashed border-border/60 rounded-md bg-card/30">
                                No volume data available
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* Status Distribution */}
                <Card className="glass-card">
                    <CardHeader>
                        <CardTitle>Status Codes</CardTitle>
                    </CardHeader>
                    <CardContent className="h-[300px] flex flex-col md:flex-row items-center justify-center gap-6">
                        {finalPieData && finalPieData.length > 0 ? (
                            <>
                                <ResponsiveContainer width="100%" height="100%" className="flex-1 min-w-[50%]">
                                    <PieChart margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
                                        <Pie
                                            data={finalPieData}
                                            cx="50%"
                                            cy="50%"
                                            innerRadius={60}
                                            outerRadius={80}
                                            paddingAngle={4}
                                            dataKey="value"
                                            stroke="none"
                                        >
                                            {finalPieData.map((entry, index) => (
                                                <Cell key={`cell-${index}`} fill={entry.color} />
                                            ))}
                                        </Pie>
                                        <Tooltip content={<CustomTooltip contentStyle={{ backgroundColor: "var(--card, #13161e)", borderColor: "var(--border, #1e2330)", color: "var(--foreground, #e8eaf0)" }} />} />
                                    </PieChart>
                                </ResponsiveContainer>
                                <div className="space-y-4">
                                    {finalPieData.map((entry) => (
                                        <div key={entry.name} className="flex items-center gap-3">
                                            <div
                                                className="h-3 w-3 rounded-full"
                                                style={{ backgroundColor: entry.color }}
                                            />
                                            <span className="text-sm font-mono">{entry.name}</span>
                                            <span className="text-sm font-bold tabular-nums">{entry.value}</span>
                                        </div>
                                    ))}
                                </div>
                            </>
                        ) : (
                            <div className="h-full w-full flex items-center justify-center text-[13px] text-muted-foreground border border-dashed border-border/60 rounded-md bg-card/30">
                                No status data available
                            </div>
                        )}
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}
