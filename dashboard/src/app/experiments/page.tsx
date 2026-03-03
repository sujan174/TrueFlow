"use client";

import { useState } from "react";
import useSWR from "swr";
import { swrFetcher, ExperimentSummary } from "@/lib/api";
import {
    Activity,
    Layers,
    BarChart3,
    Clock,
    DollarSign,
    AlertTriangle,
    FlaskConical
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/empty-state";
import { PageSkeleton } from "@/components/page-skeleton";
import { cn } from "@/lib/utils";
import {
    BarChart,
    Bar,
    XAxis,
    YAxis,
    CartesianGrid,
    Tooltip,
    Legend,
    ResponsiveContainer,
    Cell
} from 'recharts';

// A refined teal/indigo palette for the variants
const VARIANT_COLORS = ['#cf3453', '#a9927d', '#d4a574', '#c47a50', '#e85d75'];

export default function ExperimentsPage() {
    // We pass a very large limit here because the backend aggregates this already
    // so the response is just 1 row per experiment + variant.
    const { data: experiments = [], isLoading } = useSWR<ExperimentSummary[]>("/analytics/experiments", swrFetcher, { refreshInterval: 10000 });

    if (isLoading) {
        return <PageSkeleton cards={2} rows={0} />;
    }

    // Group the flattened API response by experiment_name
    const groupedExperiments = experiments.reduce((acc, curr) => {
        if (!acc[curr.experiment_name]) {
            acc[curr.experiment_name] = [];
        }
        acc[curr.experiment_name].push(curr);
        return acc;
    }, {} as Record<string, ExperimentSummary[]>);

    const experimentNames = Object.keys(groupedExperiments);

    if (experimentNames.length === 0) {
        return (
            <div className="space-y-4">
                <div>
                    <h1 className="text-lg font-semibold tracking-tight">Experiments</h1>
                    <p className="text-xs text-muted-foreground mt-0.5">A/B test traffic splits and compare variant performance.</p>
                </div>
                <EmptyState
                    icon={FlaskConical}
                    title="No active experiments"
                    description="Create a Policy with the 'Split' action to begin routing traffic between different models or settings. Data will appear here automatically."
                    actionLabel="Create Policy"
                    onAction={() => window.location.href = '/policies'}
                />
            </div>
        );
    }

    return (
        <div className="space-y-4">
            {/* Header */}
            <div>
                <h1 className="text-lg font-semibold tracking-tight">
                    Experiments
                </h1>
                <p className="text-xs text-muted-foreground mt-0.5">
                    A/B test traffic splits and compare variant performance.
                </p>
            </div>

            <div className="space-y-12">
                {experimentNames.map((expName) => {
                    const variants = groupedExperiments[expName];
                    const totalRequests = variants.reduce((sum, v) => sum + v.total_requests, 0);

                    // Prepare chart data (Latency comparison)
                    const chartData = variants.map(v => ({
                        name: v.variant_name || "baseline",
                        latency: Math.round(v.avg_latency_ms),
                        cost: Number(v.total_cost_usd.toFixed(4)),
                        error_rate: Number(((v.error_count / v.total_requests) * 100).toFixed(1))
                    }));

                    return (
                        <div key={expName} className="space-y-6 animate-fade-in">
                            <div className="flex items-center gap-3 border-b pb-2">
                                <div className="p-2 bg-primary/10 rounded">
                                    <FlaskConical className="h-5 w-5 text-primary" />
                                </div>
                                <div>
                                    <h3 className="text-xl font-semibold">{expName}</h3>
                                    <p className="text-xs text-muted-foreground">{totalRequests.toLocaleString()} total requests logged</p>
                                </div>
                            </div>

                            <div className="grid lg:grid-cols-3 gap-3">
                                {/* Variant Cards */}
                                <div className="lg:col-span-2 grid sm:grid-cols-2 gap-4">
                                    {variants.map((v, i) => {
                                        const color = VARIANT_COLORS[i % VARIANT_COLORS.length];
                                        const errorRate = ((v.error_count / v.total_requests) * 100);
                                        return (
                                            <Card key={v.variant_name} className="glass-card overflow-hidden">
                                                <div className="h-1 w-full" style={{ backgroundColor: color }} />
                                                <CardHeader className="pb-2 flex flex-row items-center justify-between">
                                                    <CardTitle className="text-lg font-mono">{v.variant_name || "baseline"}</CardTitle>
                                                    <Badge variant="outline" className="font-mono">{v.total_requests} reqs</Badge>
                                                </CardHeader>
                                                <CardContent>
                                                    <div className="grid grid-cols-2 gap-y-4 gap-x-2 mt-2">
                                                        <div className="space-y-1">
                                                            <div className="flex items-center gap-1.5 text-muted-foreground">
                                                                <Clock className="h-3.5 w-3.5" />
                                                                <span className="text-xs">Avg Latency</span>
                                                            </div>
                                                            <p className="font-semibold text-lg">{Math.round(v.avg_latency_ms)}ms</p>
                                                        </div>
                                                        <div className="space-y-1">
                                                            <div className="flex items-center gap-1.5 text-muted-foreground">
                                                                <DollarSign className="h-3.5 w-3.5" />
                                                                <span className="text-xs">Est. Cost</span>
                                                            </div>
                                                            <p className="font-semibold text-lg">${v.total_cost_usd.toFixed(4)}</p>
                                                        </div>
                                                        <div className="space-y-1">
                                                            <div className="flex items-center gap-1.5 text-muted-foreground">
                                                                <Layers className="h-3.5 w-3.5" />
                                                                <span className="text-xs">Avg Tokens</span>
                                                            </div>
                                                            <p className="font-semibold text-lg">{Math.round(v.avg_tokens)}</p>
                                                        </div>
                                                        <div className="space-y-1">
                                                            <div className="flex items-center gap-1.5 text-muted-foreground">
                                                                <AlertTriangle className="h-3.5 w-3.5" />
                                                                <span className="text-xs">Error Rate</span>
                                                            </div>
                                                            <p className={cn("font-semibold text-lg", errorRate > 5 ? "text-rose-500" : "")}>
                                                                {errorRate.toFixed(1)}%
                                                            </p>
                                                        </div>
                                                    </div>
                                                </CardContent>
                                            </Card>
                                        );
                                    })}
                                </div>

                                {/* Comparison Chart */}
                                <Card className="glass-card">
                                    <CardHeader className="pb-2">
                                        <CardTitle className="text-sm font-medium flex items-center gap-2">
                                            <BarChart3 className="h-4 w-4" /> Latency vs Cost Comparison
                                        </CardTitle>
                                    </CardHeader>
                                    <CardContent>
                                        <div className="h-[250px] w-full mt-4">
                                            <ResponsiveContainer width="100%" height="100%">
                                                <BarChart data={chartData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                                                    <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                                                    <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 12, fill: '#888' }} />
                                                    <YAxis yAxisId="left" orientation="left" stroke="#888" axisLine={false} tickLine={false} tick={{ fontSize: 12 }} />
                                                    <YAxis yAxisId="right" orientation="right" stroke="#888" axisLine={false} tickLine={false} tick={{ fontSize: 12 }} />
                                                    <Tooltip
                                                        contentStyle={{ backgroundColor: 'rgba(0,0,0,0.8)', border: 'none', borderRadius: '8px', fontSize: '12px' }}
                                                        itemStyle={{ color: '#fff' }}
                                                    />
                                                    <Legend wrapperStyle={{ fontSize: '12px' }} />
                                                    <Bar yAxisId="left" dataKey="latency" name="Latency (ms)" radius={[4, 4, 0, 0]}>
                                                        {chartData.map((entry, index) => (
                                                            <Cell key={`cell-latency-${index}`} fill={VARIANT_COLORS[index % VARIANT_COLORS.length]} fillOpacity={0.9} />
                                                        ))}
                                                    </Bar>
                                                    <Bar yAxisId="right" dataKey="cost" name="Cost ($)" radius={[4, 4, 0, 0]}>
                                                        {chartData.map((entry, index) => (
                                                            <Cell key={`cell-cost-${index}`} fill={VARIANT_COLORS[index % VARIANT_COLORS.length]} fillOpacity={0.5} />
                                                        ))}
                                                    </Bar>
                                                </BarChart>
                                            </ResponsiveContainer>
                                        </div>
                                    </CardContent>
                                </Card>
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
