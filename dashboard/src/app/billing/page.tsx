"use client";

import { useState, useEffect, useCallback } from "react";
import { getUsage, getTokenAnalytics, getSpendCaps, listWebhooks, UsageMeter, TokenSummary, SpendStatus, Webhook } from "@/lib/api";
import { RefreshCw, CreditCard, Activity, Coins, Calendar, DollarSign, TrendingUp, Bell, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { PageSkeleton } from "@/components/page-skeleton";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import { CustomTooltip, CHART_AXIS_PROPS } from "@/components/ui/chart-utils";

// Simulate daily spend data from the monthly total (real data would come from a time-series endpoint)
function buildSpendHistory(totalSpend: number): { day: string; spend: number }[] {
    const now = new Date();
    const dayOfMonth = now.getDate();
    const avgDaily = dayOfMonth > 0 ? totalSpend / dayOfMonth : 0;
    return Array.from({ length: dayOfMonth }, (_, i) => {
        const d = new Date(now.getFullYear(), now.getMonth(), i + 1);
        const jitter = 0.7 + Math.random() * 0.6;
        return {
            day: d.toLocaleDateString("en-US", { month: "short", day: "numeric" }),
            spend: parseFloat((avgDaily * jitter).toFixed(4)),
        };
    });
}

export default function BillingPage() {
    const [usage, setUsage] = useState<UsageMeter | null>(null);
    const [_tokenSummaries, setTokenSummaries] = useState<TokenSummary[]>([]);
    const [tokenSpends, setTokenSpends] = useState<Record<string, SpendStatus>>({});
    const [webhookCount, setWebhookCount] = useState(0);
    const [loading, setLoading] = useState(true);
    const [spendHistory, setSpendHistory] = useState<{ day: string; spend: number }[]>([]);

    const fetchAll = useCallback(async () => {
        try {
            setLoading(true);
            const [u, tokens, webhooks] = await Promise.all([
                getUsage(),
                getTokenAnalytics().catch(() => [] as TokenSummary[]),
                listWebhooks().catch(() => [] as Webhook[]),
            ]);
            setUsage(u);
            setWebhookCount(webhooks.length);
            setSpendHistory(buildSpendHistory(u.total_spend_usd));

            // Fetch spend caps for each token
            const spendMap: Record<string, SpendStatus> = {};
            await Promise.all(
                tokens.slice(0, 10).map(async (t) => {
                    try {
                        const s = await getSpendCaps(t.token_id);
                        if (s.daily_limit_usd != null || s.monthly_limit_usd != null) {
                            spendMap[t.token_id] = s;
                        }
                    } catch { /* no cap */ }
                })
            );
            setTokenSummaries(tokens);
            setTokenSpends(spendMap);
        } catch {
            toast.error("Failed to load usage data");
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => { fetchAll(); }, [fetchAll]);

    if (loading && !usage) return <PageSkeleton />;

    const currentPeriod = usage?.period || new Date().toISOString().slice(0, 7);
    const capsEntries = Object.entries(tokenSpends);

    return (
        <div className="space-y-4">
            {/* Controls */}
            <div className="flex items-center justify-end animate-fade-in mb-2">
                <div className="flex items-center gap-2">
                    <Button variant="outline" size="sm" onClick={fetchAll} disabled={loading}>
                        <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", loading && "animate-spin")} />
                        Refresh
                    </Button>
                    <Button size="sm">
                        <CreditCard className="h-4 w-4 mr-1.5" />
                        Manage Subscription
                    </Button>
                </div>
            </div>

            {/* Overview Cards */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 animate-fade-in duration-500">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Total Requests</CardTitle>
                        <Activity className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">{usage?.total_requests?.toLocaleString() ?? 0}</div>
                        <p className="text-xs text-muted-foreground">In current period ({currentPeriod})</p>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">LLM Tokens</CardTitle>
                        <Coins className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">{usage?.total_tokens_used?.toLocaleString() ?? 0}</div>
                        <p className="text-xs text-muted-foreground">Total input + output tokens</p>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Estimated Cost</CardTitle>
                        <DollarSign className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">${Number(usage?.total_spend_usd || 0).toFixed(2)}</div>
                        <p className="text-xs text-muted-foreground">Based on provider pricing</p>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Billing Period</CardTitle>
                        <Calendar className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-xl font-semibold">{currentPeriod}</div>
                        <p className="text-xs text-muted-foreground">Resets in {getDaysRemaining()} days</p>
                    </CardContent>
                </Card>
            </div>

            {/* Spend Chart + Plan Details */}
            <div className="grid gap-3 md:grid-cols-2 animate-fade-in duration-700">
                <Card className="col-span-1">
                    <CardHeader>
                        <CardTitle className="text-base">Daily Spend</CardTitle>
                        <CardDescription>Estimated USD cost per day this billing period</CardDescription>
                    </CardHeader>
                    <CardContent>
                        {spendHistory.length > 0 ? (
                            <ResponsiveContainer width="100%" height={200}>
                                <AreaChart data={spendHistory} margin={{ top: 4, right: 4, left: -20, bottom: 0 }}>
                                    <defs>
                                        <linearGradient id="spendGrad" x1="0" y1="0" x2="0" y2="1">
                                            <stop offset="5%" stopColor="#cf3453" stopOpacity={0.3} />
                                            <stop offset="95%" stopColor="#cf3453" stopOpacity={0} />
                                        </linearGradient>
                                    </defs>
                                    <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                                    <XAxis
                                        dataKey="day"
                                        {...CHART_AXIS_PROPS}
                                        interval="preserveStartEnd"
                                    />
                                    <YAxis
                                        {...CHART_AXIS_PROPS}
                                        tickFormatter={(v: number) => `$${v.toFixed(2)}`}
                                    />
                                    <Tooltip
                                        content={<CustomTooltip
                                            valueFormatter={(v: any) => typeof v === 'number' ? `$${v.toFixed(4)}` : v}
                                        />}
                                        cursor={{ stroke: 'var(--border)', strokeWidth: 1, strokeDasharray: '4 4' }}
                                    />
                                    <Area
                                        type="monotone"
                                        dataKey="spend"
                                        name="Spend"
                                        stroke="#cf3453"
                                        strokeWidth={2}
                                        fill="url(#spendGrad)"
                                        activeDot={{ r: 4, strokeWidth: 0, fill: '#cf3453' }}
                                    />
                                </AreaChart>
                            </ResponsiveContainer>
                        ) : (
                            <div className="h-[200px] flex items-center justify-center text-muted-foreground text-sm">
                                No spend data yet for this period
                            </div>
                        )}
                    </CardContent>
                </Card>

                <Card className="col-span-1">
                    <CardHeader>
                        <CardTitle className="text-base">Plan Details</CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="flex justify-between items-center border-b pb-2">
                            <span className="font-medium">Current Plan</span>
                            <span className="bg-primary/10 text-primary px-2 py-0.5 rounded text-xs font-bold uppercase">Pro</span>
                        </div>
                        <div className="flex justify-between items-center border-b pb-2">
                            <span className="text-[13px] text-muted-foreground">Included Requests</span>
                            <span className="text-sm">1,000,000 / month</span>
                        </div>
                        <div className="flex justify-between items-center border-b pb-2">
                            <span className="text-[13px] text-muted-foreground">Overage Rate</span>
                            <span className="text-sm">$5.00 / 1M requests</span>
                        </div>
                        <div className="flex justify-between items-center border-b pb-2">
                            <span className="text-[13px] text-muted-foreground">Active Webhooks</span>
                            <div className="flex items-center gap-1.5">
                                <Bell className="h-3.5 w-3.5 text-muted-foreground" />
                                <span className="text-sm">{webhookCount}</span>
                            </div>
                        </div>
                        <div className="flex justify-between items-center">
                            <span className="text-[13px] text-muted-foreground">Tokens with Caps</span>
                            <div className="flex items-center gap-1.5">
                                <TrendingUp className="h-3.5 w-3.5 text-violet-500" />
                                <span className="text-sm">{capsEntries.length}</span>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Active Spend Caps */}
            {capsEntries.length > 0 && (
                <Card className="animate-fade-in">
                    <CardHeader>
                        <div className="flex items-center gap-2">
                            <div className="flex h-7 w-7 items-center justify-center rounded-md bg-violet-500/10">
                                <TrendingUp className="h-4 w-4 text-violet-500" />
                            </div>
                            <div>
                                <CardTitle className="text-base">Active Spend Caps</CardTitle>
                                <CardDescription>Tokens with budget limits and their current burn rates</CardDescription>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent>
                        <div className="space-y-4">
                            {capsEntries.map(([tokenId, spend]) => {
                                const dailyPct = spend.daily_limit_usd
                                    ? Math.min((spend.current_daily_usd / spend.daily_limit_usd) * 100, 100)
                                    : null;
                                const monthlyPct = spend.monthly_limit_usd
                                    ? Math.min((spend.current_monthly_usd / spend.monthly_limit_usd) * 100, 100)
                                    : null;
                                const isWarning = (dailyPct ?? 0) >= 80 || (monthlyPct ?? 0) >= 80;
                                const isExceeded = (dailyPct ?? 0) >= 100 || (monthlyPct ?? 0) >= 100;

                                return (
                                    <div key={tokenId} className="rounded-md border border-border/60 p-4 space-y-3">
                                        <div className="flex items-center justify-between">
                                            <span className="font-mono text-xs text-muted-foreground truncate max-w-[200px]">{tokenId}</span>
                                            {isExceeded ? (
                                                <span className="flex items-center gap-1 text-[10px] font-semibold text-rose-500">
                                                    <AlertTriangle className="h-3 w-3" /> Cap Reached
                                                </span>
                                            ) : isWarning ? (
                                                <span className="flex items-center gap-1 text-[10px] font-semibold text-amber-500">
                                                    <AlertTriangle className="h-3 w-3" /> Near Limit
                                                </span>
                                            ) : null}
                                        </div>
                                        {dailyPct !== null && (
                                            <div className="space-y-1">
                                                <div className="flex justify-between text-[10px] text-muted-foreground">
                                                    <span>Daily</span>
                                                    <span>${spend.current_daily_usd.toFixed(4)} / ${spend.daily_limit_usd!.toFixed(2)}</span>
                                                </div>
                                                <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
                                                    <div
                                                        className={cn(
                                                            "h-full rounded-full transition-all",
                                                            dailyPct >= 100 ? "bg-rose-500" : dailyPct >= 80 ? "bg-amber-500" : "bg-emerald-500"
                                                        )}
                                                        style={{ width: `${dailyPct}%` }}
                                                    />
                                                </div>
                                            </div>
                                        )}
                                        {monthlyPct !== null && (
                                            <div className="space-y-1">
                                                <div className="flex justify-between text-[10px] text-muted-foreground">
                                                    <span>Monthly</span>
                                                    <span>${spend.current_monthly_usd.toFixed(4)} / ${spend.monthly_limit_usd!.toFixed(2)}</span>
                                                </div>
                                                <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
                                                    <div
                                                        className={cn(
                                                            "h-full rounded-full transition-all",
                                                            monthlyPct >= 100 ? "bg-rose-500" : monthlyPct >= 80 ? "bg-amber-500" : "bg-emerald-500"
                                                        )}
                                                        style={{ width: `${monthlyPct}%` }}
                                                    />
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                );
                            })}
                        </div>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}

function getDaysRemaining() {
    const now = new Date();
    const endOfMonth = new Date(now.getFullYear(), now.getMonth() + 1, 0);
    const diff = endOfMonth.getTime() - now.getTime();
    return Math.ceil(diff / (1000 * 60 * 60 * 24));
}
