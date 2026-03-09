"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from "recharts";
import { getStatusDistribution, StatusStat } from "@/lib/api";
import { CustomTooltip } from "@/components/ui/chart-utils";

const COLORS = {
    "200": "var(--chart-1, #818cf8)",
    "400": "var(--chart-2, #22d3ee)",
    "500": "var(--destructive, #ef4444)",
};

const DEFAULT_COLOR = "#6b7280";

export function StatusPieChart() {
    const [data, setData] = useState<{ name: string; value: number; fill: string }[]>([]);

    useEffect(() => {
        getStatusDistribution().then(stats => {
            const formatted = stats.map((s: StatusStat) => {
                const name = `${s.status_class}xx`;
                return {
                    name,
                    value: s.count,
                    fill: COLORS[s.status_class.toString() as keyof typeof COLORS] || DEFAULT_COLOR
                };
            });
            setData(formatted);
        }).catch(err => console.error("Failed to fetch status distribution", err));
    }, []);

    return (
        <Card className="col-span-1 border-border/40 shadow-sm glass-card">
            <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Status Codes (24h)</CardTitle>
            </CardHeader>
            <CardContent className="h-[350px]">
                <ResponsiveContainer width="100%" height="100%">
                    <PieChart margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
                        <Pie
                            data={data}
                            cx="50%"
                            cy="50%"
                            innerRadius={60}
                            outerRadius={80}
                            paddingAngle={5}
                            dataKey="value"
                            stroke="none"
                        >
                            {data.map((entry, index) => (
                                <Cell key={`cell-${index}`} fill={entry.fill} />
                            ))}
                        </Pie>
                        <Tooltip content={<CustomTooltip />} />
                        <Legend verticalAlign="bottom" height={36} wrapperStyle={{ fontSize: '12px' }} />
                    </PieChart>
                </ResponsiveContainer>
            </CardContent>
        </Card>
    );
}
