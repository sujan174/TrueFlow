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
    const { data: statusData, isLoading: loadingStatus } = useSWR<any[]>(
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
            "2xx": { count: 0, color: "#a9927d", label: "Success (2xx)" },
            "400/404": { count: 0, color: "#d4a574", label: "User Error (400/404)" },
            "429": { count: 0, color: "#c47a50", label: "Rate Limited (429)" },
            "5xx": { count: 0, color: "#cf3453", label: "Provider Down (5xx)" },
            "Other": { count: 0, color: "#5e503f", label: "Other" },
        };

        statusData.forEach((s) => {
            const code = s.status_class || s.status_code || 0;
            if (code >= 200 && code < 300) groups["2xx"].count += s.count;
            else if (code === 400 || code === 404) groups["400/404"].count += s.count;
            else if (code === 429) groups["429"].count += s.count;
            else if (code >= 500 && code < 600) groups["5xx"].count += s.count;
            else groups["Other"].count += s.count;
        });

        return Object.entries(groups)
            .filter(([, data]) => data.count > 0)
            .map(([, data]) => ({ name: data.label, value: data.count, fill: data.color }));
    }, [statusData]);

    // Formatters
    const formatCost = (val: number) => `$${val.toFixed(4)}`;
    const formatLatency = (val: number) => `${Math.round(val)}ms`;
    const formatNumber = (val: number) => new Intl.NumberFormat('en-US', { notation: "compact", maximumFractionDigits: 1 }).format(val);
    const formatDate = (dateStr: any) => {
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
                <Card className="col-span-4 glass-card">
                    <CardHeader>
                        <CardTitle>Request Volume</CardTitle>
                        <CardDescription>
                            Total requests and errors over time.
                        </CardDescription>
                    </CardHeader>
                    <CardContent className="pl-2">
                        <div className="h-[350px] w-full">
                            {loadingTimeseries ? (
                                <Skeleton className="h-full w-full" />
                            ) : timeseries && timeseries.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                                    <AreaChart data={timeseries} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                                        <defs>
                                            <linearGradient id="colorRequests" x1="0" y1="0" x2="0" y2="1">
                                                <stop offset="5%" stopColor="#cf3453" stopOpacity={0.3} />
                                                <stop offset="95%" stopColor="#cf3453" stopOpacity={0} />
                                            </linearGradient>
                                            <linearGradient id="colorErrors" x1="0" y1="0" x2="0" y2="1">
                                                <stop offset="5%" stopColor="#e85d75" stopOpacity={0.3} />
                                                <stop offset="95%" stopColor="#e85d75" stopOpacity={0} />
                                            </linearGradient>
                                        </defs>

                                        <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                                        <XAxis
                                            dataKey="bucket"
                                            tickFormatter={formatDate}
                                            {...CHART_AXIS_PROPS}
                                            minTickGap={30}
                                        />
                                        <YAxis
                                            {...CHART_AXIS_PROPS}
                                            tickFormatter={(value) => `${value}`}
                                        />
                                        <Tooltip
                                            content={<CustomTooltip labelFormatter={formatDate} />}
                                            cursor={{ stroke: 'var(--border)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                        />
                                        <Area
                                            type="monotone"
                                            dataKey="request_count"
                                            stroke="#cf3453"
                                            strokeWidth={2}
                                            fillOpacity={1}
                                            fill="url(#colorRequests)"
                                            name="Requests"
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#cf3453' }}
                                        />
                                        <Area
                                            type="monotone"
                                            dataKey="error_count"
                                            stroke="#e85d75"
                                            strokeWidth={2}
                                            fillOpacity={1}
                                            fill="url(#colorErrors)"
                                            name="Errors"
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#e85d75' }}
                                        />
                                    </AreaChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex items-center justify-center text-[13px] text-muted-foreground border border-dashed border-border/60 rounded-md bg-card/30">
                                    No data available for this range
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>

                {/* Latency & Cost */}
                <Card className="col-span-3 glass-card">
                    <CardHeader>
                        <CardTitle>Latency & Cost</CardTitle>
                        <CardDescription>
                            Performance vs. Spend correlation.
                        </CardDescription>
                    </CardHeader>
                    <CardContent className="pl-2">
                        <div className="h-[350px] w-full">
                            {loadingTimeseries ? (
                                <Skeleton className="h-full w-full" />
                            ) : timeseries && timeseries.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%">
                                    <LineChart data={timeseries} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                                        <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                                        <XAxis
                                            dataKey="bucket"
                                            tickFormatter={formatDate}
                                            {...CHART_AXIS_PROPS}
                                            minTickGap={30}
                                        />
                                        <YAxis
                                            yAxisId="left"
                                            {...CHART_AXIS_PROPS}
                                            tickFormatter={(val: any) => `${val}ms`}
                                        />
                                        <YAxis
                                            yAxisId="right"
                                            orientation="right"
                                            {...CHART_AXIS_PROPS}
                                            tickFormatter={(val: any) => typeof val === 'number' ? `$${val > 0 && val < 0.01 ? val.toFixed(4) : val.toFixed(2)}` : val}
                                        />
                                        <Tooltip
                                            content={<CustomTooltip
                                                labelFormatter={formatDate}
                                                valueFormatter={(val: any) => typeof val === 'number' && val > 0 && val < 1 ? `$${val.toFixed(4)}` : val}
                                            />}
                                            cursor={{ stroke: 'var(--border)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                        />
                                        <Line
                                            yAxisId="left"
                                            type="monotone"
                                            dataKey="lat"
                                            stroke="#cf3453"
                                            strokeWidth={2}
                                            dot={false}
                                            name="Latency (ms)"
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#cf3453' }}
                                        />
                                        <Line
                                            yAxisId="right"
                                            type="monotone"
                                            dataKey="cost"
                                            stroke="#e85d75"
                                            strokeWidth={2}
                                            dot={false}
                                            name="Cost ($)"
                                            activeDot={{ r: 4, strokeWidth: 0, fill: '#e85d75' }}
                                        />
                                    </LineChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex items-center justify-center text-[13px] text-muted-foreground border border-dashed border-border/60 rounded-md bg-card/30">
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
                <Card className="col-span-3 glass-card">
                    <CardHeader>
                        <CardTitle>Status Code Breakdown</CardTitle>
                        <CardDescription>
                            Distribution of routing outcomes and upstream errors.
                        </CardDescription>
                    </CardHeader>
                    <CardContent className="flex justify-center items-center">
                        <div className="h-[280px] w-full">
                            {loadingStatus ? (
                                <Skeleton className="h-full w-full" />
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
                                        <Tooltip content={<CustomTooltip contentStyle={{ backgroundColor: "#161210", borderColor: "#2d2520", color: "#eee9e5" }} />} />
                                        <Legend verticalAlign="bottom" height={36} wrapperStyle={{ fontSize: '12px' }} />
                                    </PieChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex items-center justify-center text-[13px] text-muted-foreground border border-dashed border-border/60 rounded-md bg-card/30">
                                    No status data
                                </div>
                            )}
                        </div>
                    </CardContent>
                </Card>

                {/* Token Breakdown Chart */}
                <Card className="col-span-4 glass-card">
                    <CardHeader>
                        <CardTitle>Token Breakdown</CardTitle>
                        <CardDescription>
                            Prompt vs. completion tokens over time. Higher completion ratio means more output-heavy workloads.
                        </CardDescription>
                    </CardHeader>
                    <CardContent className="pl-2">
                        <div className="h-[280px] w-full">
                            {loadingTimeseries ? (
                                <Skeleton className="h-full w-full" />
                            ) : timeseries && timeseries.length > 0 ? (
                                <ResponsiveContainer width="100%" height="100%">
                                    <BarChart data={timeseries} barCategoryGap="30%" margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                                        <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                                        <XAxis
                                            dataKey="bucket"
                                            tickFormatter={formatDate}
                                            {...CHART_AXIS_PROPS}
                                            minTickGap={30}
                                        />
                                        <YAxis
                                            {...CHART_AXIS_PROPS}
                                            tickFormatter={formatNumber}
                                        />
                                        <Tooltip
                                            content={<CustomTooltip
                                                labelFormatter={formatDate}
                                                valueFormatter={(val: any) => formatNumber(val ?? 0)}
                                            />}
                                            cursor={{ fill: 'var(--border)', opacity: 0.1 }}
                                        />
                                        <Legend
                                            wrapperStyle={{ fontSize: 12, paddingTop: 8 }}
                                            formatter={(v) => v === 'input_tokens' ? 'Prompt Tokens' : 'Completion Tokens'}
                                        />
                                        <Bar dataKey="input_tokens" name="input_tokens" stackId="a" fill="#cf3453" radius={[0, 0, 0, 0]} />
                                        <Bar dataKey="output_tokens" name="output_tokens" stackId="a" fill="#a9927d" radius={[4, 4, 0, 0]} />
                                    </BarChart>
                                </ResponsiveContainer>
                            ) : (
                                <div className="h-full w-full flex items-center justify-center text-[13px] text-muted-foreground border border-dashed border-border/60 rounded-md bg-card/30">
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
                <Card className="glass-card">
                    <CardHeader>
                        <CardTitle>Cost by Model</CardTitle>
                        <CardDescription>Spending breakdown across all LLM models.</CardDescription>
                    </CardHeader>
                    <CardContent>
                        {loadingBreakdown ? (
                            <Skeleton className="h-[220px] w-full" />
                        ) : breakdown && breakdown.by_model && breakdown.by_model.length > 0 ? (
                            <ResponsiveContainer width="100%" height={220} minWidth={0}>
                                <BarChart
                                    data={breakdown.by_model.slice(0, 8).map(d => ({ ...d, costFmt: `$${d.cost_usd.toFixed(4)}` }))}
                                    layout="vertical"
                                    margin={{ top: 0, right: 60, left: 0, bottom: 0 }}
                                >
                                    <XAxis type="number" {...CHART_AXIS_PROPS} tickFormatter={v => `$${v.toFixed(2)}`} />
                                    <YAxis type="category" dataKey="label" {...CHART_AXIS_PROPS} width={90} tick={{ fontSize: 10 }} />
                                    <Tooltip
                                        content={<CustomTooltip valueFormatter={(v: any) => typeof v === 'number' ? `$${v.toFixed(4)}` : v} />}
                                        cursor={{ fill: 'var(--border)', opacity: 0.1 }}
                                    />
                                    <Bar dataKey="cost_usd" name="Cost ($)" fill="#cf3453" radius={[0, 4, 4, 0]}>
                                        <LabelList dataKey="costFmt" position="right" style={{ fontSize: 10, fill: 'var(--muted-foreground)' }} />
                                    </Bar>
                                </BarChart>
                            </ResponsiveContainer>
                        ) : (
                            <div className="h-[220px] flex items-center justify-center text-xs text-muted-foreground border border-dashed rounded-md">
                                No spend data for this period
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* By Token */}
                <Card className="glass-card">
                    <CardHeader>
                        <CardTitle>Cost by Virtual Key</CardTitle>
                        <CardDescription>Top-spending virtual keys in the selected period.</CardDescription>
                    </CardHeader>
                    <CardContent>
                        {loadingBreakdown ? (
                            <Skeleton className="h-[220px] w-full" />
                        ) : breakdown && breakdown.by_token && breakdown.by_token.length > 0 ? (
                            <ResponsiveContainer width="100%" height={220} minWidth={0}>
                                <BarChart
                                    data={breakdown.by_token.slice(0, 8).map(d => ({ ...d, costFmt: `$${d.cost_usd.toFixed(4)}` }))}
                                    layout="vertical"
                                    margin={{ top: 0, right: 60, left: 0, bottom: 0 }}
                                >
                                    <XAxis type="number" {...CHART_AXIS_PROPS} tickFormatter={v => `$${v.toFixed(2)}`} />
                                    <YAxis type="category" dataKey="label" {...CHART_AXIS_PROPS} width={90} tick={{ fontSize: 10 }} />
                                    <Tooltip
                                        content={<CustomTooltip valueFormatter={(v: any) => typeof v === 'number' ? `$${v.toFixed(4)}` : v} />}
                                        cursor={{ fill: 'var(--border)', opacity: 0.1 }}
                                    />
                                    <Bar dataKey="cost_usd" name="Cost ($)" fill="#a9927d" radius={[0, 4, 4, 0]}>
                                        <LabelList dataKey="costFmt" position="right" style={{ fontSize: 10, fill: 'var(--muted-foreground)' }} />
                                    </Bar>
                                </BarChart>
                            </ResponsiveContainer>
                        ) : (
                            <div className="h-[220px] flex items-center justify-center text-xs text-muted-foreground border border-dashed rounded-md">
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
    icon: any;
    intent?: "neutral" | "success" | "warning" | "danger";
    subtext?: string;
}) {
    return (
        <Card className="glass-card">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">
                    {title}
                </CardTitle>
                <Icon className={cn(
                    "h-4 w-4",
                    intent === "neutral" && "text-muted-foreground",
                    intent === "success" && "text-emerald-500",
                    intent === "warning" && "text-amber-500",
                    intent === "danger" && "text-rose-500",
                )} />
            </CardHeader>
            <CardContent>
                {loading ? (
                    <Skeleton className="h-7 w-20" />
                ) : (
                    <div className="space-y-1">
                        <div className="text-xl font-semibold font-mono">{value ?? "—"}</div>
                        {subtext && (
                            <p className="text-xs text-muted-foreground">
                                {subtext}
                            </p>
                        )}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
