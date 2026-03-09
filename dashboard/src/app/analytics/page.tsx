"use client";

import { useState, useMemo } from "react";
import useSWR from "swr";
import { swrFetcher, AnalyticsSummary, AnalyticsTimeseriesPoint, getSpendBreakdown, SpendBreakdown } from "@/lib/api";
import {
    BarChart,
    Bar,
    XAxis,
    YAxis,
    CartesianGrid,
    Tooltip,
    ResponsiveContainer,
    AreaChart,
    Area,
    LineChart,
    Line,
    Legend,
    PieChart,
    Pie,
    Cell,
    LabelList,
} from "recharts";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Activity, DollarSign, Clock, Zap, MessageSquare, FileText } from "lucide-react";
import { cn } from "@/lib/utils";
import { CustomTooltip, CHART_AXIS_PROPS } from "@/components/ui/chart-utils";

export default function AnalyticsPage() {
    const [range, setRange] = useState("24"); // hours

    // Fetch Summary
    const { data: summary, isLoading: loadingSummary } = useSWR<AnalyticsSummary>(
        `/analytics/summary?range=${range}`,
        swrFetcher
    );

    // Fetch Timeseries
    const { data: timeseries, isLoading: loadingTimeseries } = useSWR<AnalyticsTimeseriesPoint[]>(
        `/analytics/timeseries?range=${range}`,
        swrFetcher
    );

    // Fetch Status Distribution
    const { data: statusData, isLoading: loadingStatus } = useSWR<Record<string, number | string>[]>(
        `/analytics/status?range=${range}`,
        swrFetcher
    );

    // Fetch Spend Breakdown
    const { data: breakdown, isLoading: loadingBreakdown } = useSWR<SpendBreakdown>(
        `/analytics/spend/breakdown?range=${range}`,
        () => getSpendBreakdown(range)
    );

    const statusChartData = useMemo(() => {
        if (!statusData) return [];
        const groups: Record<string, { count: number; color: string; label: string }> = {
            "2xx": { count: 0, color: "#818cf8", label: "Success (2xx)" },
            "400/404": { count: 0, color: "#d4a574", label: "User Error (400/404)" },
            "429": { count: 0, color: "#c47a50", label: "Rate Limited (429)" },
            "5xx": { count: 0, color: "#6366f1", label: "Provider Down (5xx)" },
            "Other": { count: 0, color: "#6b7280", label: "Other" },
        };

        statusData.forEach((s) => {
            const code = Number(s.status_class || s.status_code || 0);
            const count = Number(s.count || 0);
            if (code >= 200 && code < 300) groups["2xx"].count += count;
            else if (code === 400 || code === 404) groups["400/404"].count += count;
            else if (code === 429) groups["429"].count += count;
            else if (code >= 500 && code < 600) groups["5xx"].count += count;
            else groups["Other"].count += count;
        });

        return Object.entries(groups)
            .filter(([, data]) => data.count > 0)
            .map(([, data]) => ({ name: data.label, value: data.count, fill: data.color }));
    }, [statusData]);

    // Formatters
    const formatCost = (val: number) => `$${val.toFixed(4)}`;
    const formatLatency = (val: number) => `${Math.round(val)}ms`;
    const formatNumber = (val: number) => new Intl.NumberFormat('en-US', { notation: "compact", maximumFractionDigits: 1 }).format(val);
    const formatDate = (dateStr: string | number) => {
        const date = new Date(dateStr);
        return range === "24"
            ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
            : date.toLocaleDateString([], { month: 'short', day: 'numeric', hour: 'numeric' });
    };

    return (
        <div className="space-y-4">
            {/* Header */}
            <div className="flex items-center justify-between gap-4">
                <div>
                    <h1 className="text-lg font-semibold tracking-tight">Analytics</h1>
                    <p className="text-xs text-muted-foreground mt-0.5">
                        Global traffic and performance metrics.
                    </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                    <Select value={range} onValueChange={setRange}>
                        <SelectTrigger className="w-[160px] h-8 text-xs">
                            <SelectValue placeholder="Select range" />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value="1">Last Hour</SelectItem>
                            <SelectItem value="24">Last 24 Hours</SelectItem>
                            <SelectItem value="168">Last 7 Days</SelectItem>
                            <SelectItem value="720">Last 30 Days</SelectItem>
                        </SelectContent>
                    </Select>
                </div>
            </div>

            {/* KPI Cards */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <KPICard
                    title="Total Requests"
                    value={summary ? formatNumber(summary.total_requests) : undefined}
                    loading={loadingSummary}
                    icon={Activity}
                    intent="neutral"
                />
                <KPICard
                    title="Error Rate"
                    value={summary ? `${((summary.error_count / (summary.total_requests || 1)) * 100).toFixed(2)}%` : undefined}
                    loading={loadingSummary}
                    icon={Zap}
                    intent={summary && (summary.error_count / (summary.total_requests || 1)) > 0.01 ? "danger" : "success"}
                    subtext={summary ? `${summary.error_count} errors` : undefined}
                />
                <KPICard
                    title="Avg Latency"
                    value={summary ? formatLatency(summary.avg_latency) : undefined}
                    loading={loadingSummary}
                    icon={Clock}
                    intent={summary && summary.avg_latency > 500 ? "warning" : "neutral"}
                />
                <KPICard
                    title="Total Cost"
                    value={summary ? formatCost(summary.total_cost) : undefined}
                    loading={loadingSummary}
                    icon={DollarSign}
                    intent="neutral"
                    subtext={summary ? `${formatNumber(summary.total_tokens)} tokens` : undefined}
                />
            </div>

            {/* Token Breakdown KPI Row */}
            <div className="grid gap-4 md:grid-cols-3">
                <KPICard
                    title="Prompt Tokens"
                    value={summary ? formatNumber(summary.total_input_tokens ?? 0) : undefined}
                    loading={loadingSummary}
                    icon={MessageSquare}
                    intent="neutral"
                    subtext="Tokens sent in requests"
                />
                <KPICard
                    title="Completion Tokens"
                    value={summary ? formatNumber(summary.total_output_tokens ?? 0) : undefined}
                    loading={loadingSummary}
                    icon={FileText}
                    intent="neutral"
                    subtext="Tokens generated by models"
                />
                <KPICard
                    title="Token Efficiency"
                    value={summary && (summary.total_input_tokens ?? 0) > 0
                        ? `${((summary.total_output_tokens ?? 0) / (summary.total_input_tokens ?? 1)).toFixed(2)}×`
                        : "—"}
                    loading={loadingSummary}
                    icon={Zap}
                    intent="neutral"
                    subtext="Output / Input ratio"
                />
            </div>

            {/* Charts */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-7">
                {/* Main Volume Chart */}
                <Card className="col-span-4 bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-4">
                    <CardHeader className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between space-y-0">
                        <div className="space-y-1">
                            <CardTitle className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">Request Volume</CardTitle>
                            <CardDescription className="text-[10px] text-zinc-600">
                                Total requests and errors over time.
                            </CardDescription>
                        </div>
                    </CardHeader>
                    <CardContent className="p-5">
                        <div className="h-[350px] w-full">
                            {loadingTimeseries ? (
                                <div className="h-full w-full flex items-center justify-center">
                                    <div className="h-6 w-6 animate-spin text-zinc-600 rounded-full border-2 border-transparent border-t-zinc-600" />
                                </div>
                            ) : timeseries && timeseries.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                                    <AreaChart data={timeseries} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                                        <defs>
                                            <linearGradient id="colorRequests" x1="0" y1="0" x2="0" y2="1">
                                                <stop offset="5%" stopColor="#ffffff" stopOpacity={0.15} />
                                                <stop offset="95%" stopColor="#ffffff" stopOpacity={0} />
                                            </linearGradient>
                                            <linearGradient id="colorErrors" x1="0" y1="0" x2="0" y2="1">
                                                <stop offset="5%" stopColor="#f43f5e" stopOpacity={0.15} />
                                                <stop offset="95%" stopColor="#f43f5e" stopOpacity={0} />
                                            </linearGradient>
                                        </defs>

                                        <CartesianGrid stroke="rgba(255,255,255,0.05)" strokeDasharray="4 4" vertical={false} />
                                        <XAxis
                                            dataKey="bucket"
                                            tickFormatter={formatDate}
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            minTickGap={30}
                                        />
                                        <YAxis
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            tickFormatter={(value) => `${value}`}
                                        />
                                        <Tooltip
                                            content={<CustomTooltip labelFormatter={formatDate} />}
                                            cursor={{ stroke: 'rgba(255,255,255,0.15)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                        />
                                        <Area
                                            type="monotone"
                                            dataKey="request_count"
                                            stroke="#ffffff"
                                            strokeWidth={1.5}
                                            fillOpacity={1}
                                            fill="url(#colorRequests)"
                                            name="Requests"
                                            isAnimationActive={false}
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#ffffff', filter: 'drop-shadow(0 0 4px rgba(255,255,255,0.5))' }}
                                        />
                                        <Area
                                            type="monotone"
                                            dataKey="error_count"
                                            stroke="#f43f5e"
                                            strokeWidth={1.5}
                                            fillOpacity={1}
                                            fill="url(#colorErrors)"
                                            name="Errors"
                                            isAnimationActive={false}
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#f43f5e', filter: 'drop-shadow(0 0 4px rgba(244,63,94,0.5))' }}
                                        />
                                    </AreaChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex flex-col items-center justify-center text-[13px] text-zinc-500">
                                    <Activity className="h-6 w-6 opacity-20 mb-3" />
                                    No data available for this range
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>

                {/* Latency & Cost */}
                <Card className="col-span-3 bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-5">
                    <CardHeader className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between space-y-0">
                        <div className="space-y-1">
                            <CardTitle className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">Latency & Cost</CardTitle>
                            <CardDescription className="text-[10px] text-zinc-600">
                                Performance vs. Spend correlation.
                            </CardDescription>
                        </div>
                    </CardHeader>
                    <CardContent className="p-5">
                        <div className="h-[350px] w-full">
                            {loadingTimeseries ? (
                                <div className="h-full w-full flex items-center justify-center">
                                    <div className="h-6 w-6 animate-spin text-zinc-600 rounded-full border-2 border-transparent border-t-zinc-600" />
                                </div>
                            ) : timeseries && timeseries.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%">
                                    <LineChart data={timeseries} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                                        <CartesianGrid stroke="rgba(255,255,255,0.05)" strokeDasharray="3 3" vertical={false} />
                                        <XAxis
                                            dataKey="bucket"
                                            tickFormatter={formatDate}
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            minTickGap={30}
                                        />
                                        <YAxis
                                            yAxisId="left"
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            tickFormatter={(val: number) => `${val}ms`}
                                        />
                                        <YAxis
                                            yAxisId="right"
                                            orientation="right"
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            tickFormatter={(val: number | string) => typeof val === 'number' ? `$${val > 0 && val < 0.01 ? val.toFixed(4) : val.toFixed(2)}` : val}
                                        />
                                        <Tooltip
                                            content={<CustomTooltip
                                                labelFormatter={formatDate as (label: string) => string}
                                                valueFormatter={(val: number | string) => typeof val === 'number' && val > 0 && val < 1 ? `$${val.toFixed(4)}` : String(val)}
                                            />}
                                            cursor={{ stroke: 'rgba(255,255,255,0.15)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                        />
                                        <Line
                                            yAxisId="left"
                                            type="monotone"
                                            dataKey="lat"
                                            stroke="#10b981"
                                            strokeWidth={1.5}
                                            dot={false}
                                            name="Latency (ms)"
                                            isAnimationActive={false}
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#10b981', filter: 'drop-shadow(0 0 4px rgba(16,185,129,0.5))' }}
                                        />
                                        <Line
                                            yAxisId="right"
                                            type="monotone"
                                            dataKey="cost"
                                            stroke="#fbbf24"
                                            strokeWidth={1.5}
                                            dot={false}
                                            name="Cost ($)"
                                            isAnimationActive={false}
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#fbbf24', filter: 'drop-shadow(0 0 4px rgba(251,191,36,0.5))' }}
                                        />
                                    </LineChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex flex-col items-center justify-center text-[13px] text-zinc-500">
                                    <Activity className="h-6 w-6 opacity-20 mb-3" />
                                    No data available for this range
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Bottom Row Charts */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-7">
                {/* Granular Status Code Doughnut */}
                <Card className="col-span-3 bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-5">
                    <CardHeader className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between space-y-0">
                        <div className="space-y-1">
                            <CardTitle className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">Status Code Breakdown</CardTitle>
                            <CardDescription className="text-[10px] text-zinc-600">
                                Distribution of routing outcomes and upstream errors.
                            </CardDescription>
                        </div>
                    </CardHeader>
                    <CardContent className="flex justify-center items-center p-5">
                        <div className="h-[280px] w-full">
                            {loadingStatus ? (
                                <div className="h-full w-full flex items-center justify-center">
                                    <div className="h-6 w-6 animate-spin text-zinc-600 rounded-full border-2 border-transparent border-t-zinc-600" />
                                </div>
                            ) : statusChartData.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%">
                                    <PieChart margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
                                        <Pie
                                            data={statusChartData}
                                            innerRadius={70}
                                            outerRadius={100}
                                            paddingAngle={3}
                                            dataKey="value"
                                            stroke="none"
                                        >
                                            {statusChartData.map((entry, index) => (
                                                <Cell key={`cell-${index}`} fill={entry.fill} />
                                            ))}
                                        </Pie>
                                        <Tooltip content={<CustomTooltip />} />
                                        <Legend verticalAlign="bottom" height={36} wrapperStyle={{ fontSize: '11px', color: 'rgba(255,255,255,0.6)' }} />
                                    </PieChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex flex-col items-center justify-center text-[13px] text-zinc-500">
                                    <Activity className="h-6 w-6 opacity-20 mb-3" />
                                    No status data
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>

                {/* Token Breakdown Chart */}
                <Card className="col-span-4 bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-6">
                    <CardHeader className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between space-y-0">
                        <div className="space-y-1">
                            <CardTitle className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">Token Breakdown</CardTitle>
                            <CardDescription className="text-[10px] text-zinc-600">
                                Prompt vs. completion tokens over time. Higher completion ratio means more output-heavy workloads.
                            </CardDescription>
                        </div>
                    </CardHeader>
                    <CardContent className="p-5">
                        <div className="h-[280px] w-full">
                            {loadingTimeseries ? (
                                <div className="h-full w-full flex items-center justify-center">
                                    <div className="h-6 w-6 animate-spin text-zinc-600 rounded-full border-2 border-transparent border-t-zinc-600" />
                                </div>
                            ) : timeseries && timeseries.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%">
                                    <BarChart data={timeseries} barCategoryGap="30%" margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                                        <CartesianGrid stroke="rgba(255,255,255,0.05)" strokeDasharray="3 3" vertical={false} />
                                        <XAxis
                                            dataKey="bucket"
                                            tickFormatter={formatDate}
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            minTickGap={30}
                                        />
                                        <YAxis
                                            {...CHART_AXIS_PROPS}
                                            stroke="rgba(255,255,255,0.2)"
                                            tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}}
                                            tickFormatter={formatNumber}
                                        />
                                        <Tooltip
                                            content={<CustomTooltip
                                                labelFormatter={formatDate}
                                                valueFormatter={(val: number | string) => typeof val === 'number' ? formatNumber(val) : formatNumber(Number(val ?? 0))}
                                            />}
                                            cursor={{ fill: 'rgba(255,255,255,0.05)' }}
                                        />
                                        <Legend
                                            wrapperStyle={{ fontSize: 11, paddingTop: 8, color: 'rgba(255,255,255,0.6)' }}
                                            formatter={(v) => v === 'input_tokens' ? 'Prompt Tokens' : 'Completion Tokens'}
                                        />
                                        <Bar dataKey="input_tokens" name="input_tokens" stackId="a" fill="#3b82f6" radius={[0, 0, 0, 0]} />
                                        <Bar dataKey="output_tokens" name="output_tokens" stackId="a" fill="#8b5cf6" radius={[4, 4, 0, 0]} />
                                    </BarChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex flex-col items-center justify-center text-[13px] text-zinc-500">
                                    <Activity className="h-6 w-6 opacity-20 mb-3" />
                                    No data available for this range
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Spend Breakdown */}
            <div className="grid gap-4 md:grid-cols-2">
                {/* By Model */}
                <Card className="bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-6">
                    <CardHeader className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between space-y-0">
                        <div className="space-y-1">
                            <CardTitle className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">Cost by Model</CardTitle>
                            <CardDescription className="text-[10px] text-zinc-600">Spending breakdown across all LLM models.</CardDescription>
                        </div>
                    </CardHeader>
                    <CardContent className="p-5">
                        {loadingBreakdown ? (
                            <div className="h-[220px] w-full flex items-center justify-center">
                                <div className="h-6 w-6 animate-spin text-zinc-600 rounded-full border-2 border-transparent border-t-zinc-600" />
                            </div>
                        ) : breakdown && breakdown.by_model && breakdown.by_model.length > 0 ? (
                            <ResponsiveContainer width="100%" height={220} minWidth={0}>
                                <BarChart
                                    data={breakdown.by_model.slice(0, 8).map(d => ({ ...d, costFmt: `$${d.cost_usd.toFixed(4)}` }))}
                                    layout="vertical"
                                    margin={{ top: 0, right: 60, left: 0, bottom: 0 }}
                                >
                                    <XAxis type="number" {...CHART_AXIS_PROPS} stroke="rgba(255,255,255,0.2)" tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}} tickFormatter={v => `$${v.toFixed(2)}`} />
                                    <YAxis type="category" dataKey="label" {...CHART_AXIS_PROPS} stroke="rgba(255,255,255,0.2)" tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 10}} width={90} />
                                    <Tooltip
                                        content={<CustomTooltip valueFormatter={(v: number | string) => typeof v === 'number' ? `$${v.toFixed(4)}` : v} />}
                                        cursor={{ fill: 'rgba(255,255,255,0.05)' }}
                                    />
                                    <Bar dataKey="cost_usd" name="Cost ($)" fill="#3b82f6" radius={[0, 4, 4, 0]}>
                                        <LabelList dataKey="costFmt" position="right" style={{ fontSize: 10, fill: 'rgba(255,255,255,0.5)' }} />
                                    </Bar>
                                </BarChart>
                            </ResponsiveContainer>
                        ) : (
                            <div className="h-[220px] flex flex-col items-center justify-center text-[13px] text-zinc-500">
                                <DollarSign className="h-6 w-6 opacity-20 mb-3" />
                                No spend data for this period
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* By Token */}
                <Card className="bg-black border border-white/10 rounded-lg overflow-hidden animate-slide-up stagger-7">
                    <CardHeader className="px-5 py-4 border-b border-white/10 flex flex-row items-center justify-between space-y-0">
                        <div className="space-y-1">
                            <CardTitle className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest">Cost by Virtual Key</CardTitle>
                            <CardDescription className="text-[10px] text-zinc-600">Top-spending virtual keys in the selected period.</CardDescription>
                        </div>
                    </CardHeader>
                    <CardContent className="p-5">
                        {loadingBreakdown ? (
                            <div className="h-[220px] w-full flex items-center justify-center">
                                <div className="h-6 w-6 animate-spin text-zinc-600 rounded-full border-2 border-transparent border-t-zinc-600" />
                            </div>
                        ) : breakdown && breakdown.by_token && breakdown.by_token.length > 0 ? (
                            <ResponsiveContainer width="100%" height={220} minWidth={0}>
                                <BarChart
                                    data={breakdown.by_token.slice(0, 8).map(d => ({ ...d, costFmt: `$${d.cost_usd.toFixed(4)}` }))}
                                    layout="vertical"
                                    margin={{ top: 0, right: 60, left: 0, bottom: 0 }}
                                >
                                    <XAxis type="number" {...CHART_AXIS_PROPS} stroke="rgba(255,255,255,0.2)" tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 11}} tickFormatter={v => `$${v.toFixed(2)}`} />
                                    <YAxis type="category" dataKey="label" {...CHART_AXIS_PROPS} stroke="rgba(255,255,255,0.2)" tick={{fill: 'rgba(255,255,255,0.4)', fontSize: 10}} width={90} />
                                    <Tooltip
                                        content={<CustomTooltip valueFormatter={(v: number | string) => typeof v === 'number' ? `$${v.toFixed(4)}` : v} />}
                                        cursor={{ fill: 'rgba(255,255,255,0.05)' }}
                                    />
                                    <Bar dataKey="cost_usd" name="Cost ($)" fill="#8b5cf6" radius={[0, 4, 4, 0]}>
                                        <LabelList dataKey="costFmt" position="right" style={{ fontSize: 10, fill: 'rgba(255,255,255,0.5)' }} />
                                    </Bar>
                                </BarChart>
                            </ResponsiveContainer>
                        ) : (
                            <div className="h-[220px] flex flex-col items-center justify-center text-[13px] text-zinc-500">
                                <DollarSign className="h-6 w-6 opacity-20 mb-3" />
                                No spend data for this period
                            </div>
                        )}
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}

function KPICard({
    title,
    value,
    loading,
    icon: Icon,
    intent = "neutral",
    subtext
}: {
    title: string;
    value?: string | number;
    loading?: boolean;
    icon: React.ElementType;
    intent?: "neutral" | "success" | "warning" | "danger";
    subtext?: string;
}) {
    return (
        <Card className="bg-black border border-white/10 rounded-lg p-5 hover:border-white/20 transition-all group relative overflow-hidden">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 p-0 pb-2">
                <CardTitle className="text-[11px] text-zinc-500 uppercase tracking-widest font-medium relative z-10">
                    {title}
                </CardTitle>
                <Icon className={cn(
                    "h-4 w-4 relative z-10",
                    intent === "neutral" && "text-zinc-500",
                    intent === "success" && "text-emerald-500",
                    intent === "warning" && "text-amber-500",
                    intent === "danger" && "text-rose-500",
                )} />
            </CardHeader>
            <CardContent className="p-0">
                {loading ? (
                    <div className="h-9 w-24 bg-white/5 rounded shimmer mt-2 relative z-10" />
                ) : (
                    <div className="space-y-1 mt-2 relative z-10">
                        <div className="text-[32px] leading-none text-white tracking-tighter">{value ?? "—"}</div>
                        {subtext && (
                            <p className="text-[11px] text-zinc-600 font-medium">
                                {subtext}
                            </p>
                        )}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
