"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis, CartesianGrid } from "recharts";
import { getRequestVolume, VolumeStat } from "@/lib/api";
import { format, parseISO } from "date-fns";
import { CustomTooltip, CHART_AXIS_PROPS } from "@/components/ui/chart-utils";

export function VolumeChart() {
    const [data, setData] = useState<{ name: string; requests: number }[]>([]);

    useEffect(() => {
        getRequestVolume().then(volume => {
            // Fill in missing hours if necessary, but for now just map existing
            const formatted = volume.map((v: VolumeStat) => ({
                name: format(parseISO(v.bucket), "HH:mm"),
                requests: v.count
            }));
            setData(formatted);
        }).catch(err => console.error("Failed to fetch volume", err));
    }, []);

    return (
        <Card className="col-span-4 border-border/40 shadow-sm glass-card">
            <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Request Volume (24h)</CardTitle>
            </CardHeader>
            <CardContent className="pl-0 pb-4 pr-4 border-t border-border/10 pt-4">
                <ResponsiveContainer width="100%" height={300}>
                    <AreaChart data={data} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                        <defs>
                            <linearGradient id="colorRequests" x1="0" y1="0" x2="0" y2="1">
                                <stop offset="5%" stopColor="#cf3453" stopOpacity={0.3} />
                                <stop offset="95%" stopColor="#cf3453" stopOpacity={0} />
                            </linearGradient>
                        </defs>
                        <CartesianGrid stroke="#2d2520" strokeDasharray="3 3" vertical={false} />
                        <XAxis
                            dataKey="name"
                            {...CHART_AXIS_PROPS}
                        />
                        <YAxis
                            {...CHART_AXIS_PROPS}
                            tickFormatter={(value) => `${value}`}
                        />
                        <Tooltip content={<CustomTooltip contentStyle={{ backgroundColor: "#161210", borderColor: "#2d2520", color: "#eee9e5" }} />} cursor={{ stroke: 'var(--border)', strokeWidth: 1, strokeDasharray: '4 4' }} />
                        <Area
                            type="monotone"
                            dataKey="requests"
                            name="Requests"
                            stroke="#cf3453"
                            strokeWidth={2}
                            fill="url(#colorRequests)"
                            activeDot={{ r: 4, strokeWidth: 0, fill: '#cf3453' }}
                        />
                    </AreaChart>
                </ResponsiveContainer>
            </CardContent>
        </Card>
    );
}
