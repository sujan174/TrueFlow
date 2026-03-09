"use client"

import { ColumnDef } from "@tanstack/react-table"
import { AuditLog } from "@/lib/api"
import { ArrowUpDown, Eye, Cpu, Zap, Wrench } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"

const finishReasonColor: Record<string, string> = {
    stop: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
    end_turn: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
    tool_calls: "bg-violet-500/15 text-violet-400 border-violet-500/30",
    tool_use: "bg-violet-500/15 text-violet-400 border-violet-500/30",
    length: "bg-amber-500/15 text-amber-400 border-amber-500/30",
    max_tokens: "bg-amber-500/15 text-amber-400 border-amber-500/30",
    content_filter: "bg-rose-500/15 text-rose-400 border-rose-500/30",
    safety: "bg-rose-500/15 text-rose-400 border-rose-500/30",
};

export const columns: ColumnDef<AuditLog>[] = [
    {
        accessorKey: "created_at",
        header: ({ column }) => (
            <Button
                variant="ghost"
                onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
                className="-ml-3 h-8 hover:bg-white/5 hover:text-white data-[state=open]:bg-white/5"
            >
                Time
                <ArrowUpDown className="ml-1 h-3 w-3" />
            </Button>
        ),
        cell: ({ row }) => (
            <div className="text-zinc-500 text-[11px] whitespace-nowrap font-mono">
                {new Date(row.getValue("created_at")).toLocaleTimeString()}
            </div>
        ),
    },
    {
        accessorKey: "method",
        header: "Method",
        cell: ({ row }) => {
            const method = row.getValue("method") as string
            let variant: "default" | "secondary" | "outline" | "destructive" | "success" = "outline";
            if (method === "GET") variant = "secondary";
            if (method === "POST") variant = "success";
            if (method === "DELETE") variant = "destructive";

            return <Badge variant={variant} className="text-[10px] px-1.5 py-0">{method}</Badge>
        },
    },
    {
        accessorKey: "path",
        header: "Path",
        cell: ({ row }) => (
            <div className="font-mono text-[11px] text-zinc-300 max-w-[180px] truncate" title={row.getValue("path")}>
                {row.getValue("path")}
            </div>
        ),
    },
    {
        accessorKey: "model",
        header: "Model",
        cell: ({ row }) => {
            const model = row.getValue("model") as string | null;
            if (!model) return <span className="text-zinc-600 text-[11px]">—</span>;
            return (
                <div className="flex items-center gap-1">
                    <Cpu className="h-3 w-3 text-violet-400" />
                    <span className="text-[11px] font-medium text-zinc-300 truncate max-w-[100px]" title={model}>{model}</span>
                </div>
            );
        },
    },
    {
        accessorKey: "upstream_status",
        header: "Status",
        cell: ({ row }) => {
            const code = row.getValue("upstream_status") as number;
            if (!code) return <span className="text-zinc-600">—</span>;

            let colorClass = "text-zinc-500";
            if (code < 300) colorClass = "text-emerald-400 font-medium";
            else if (code < 400) colorClass = "text-blue-400";
            else if (code < 500) colorClass = "text-amber-400 font-medium";
            else colorClass = "text-rose-400 font-bold";

            return <div className={`font-mono text-[11px] ${colorClass}`}>{code}</div>
        },
    },
    {
        id: "tokens",
        header: "Tokens",
        cell: ({ row }) => {
            const prompt = row.original.prompt_tokens;
            const completion = row.original.completion_tokens;
            if (prompt == null && completion == null) {
                return <span className="text-zinc-600 text-[11px]">—</span>;
            }
            return (
                <div className="text-[11px] font-mono">
                    <span className="text-blue-400">{prompt ?? 0}</span>
                    <span className="text-zinc-600 mx-0.5">→</span>
                    <span className="text-emerald-400">{completion ?? 0}</span>
                </div>
            );
        },
    },
    {
        accessorKey: "response_latency_ms",
        header: ({ column }) => (
            <Button
                variant="ghost"
                onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
                className="-ml-3 h-8 hover:bg-white/5 hover:text-white data-[state=open]:bg-white/5"
            >
                Latency
                <ArrowUpDown className="ml-1 h-3 w-3" />
            </Button>
        ),
        cell: ({ row }) => {
            const result = row.original.policy_result;
            // HITL latency is human wait time, not proxy performance
            if (["approved", "rejected", "timeout"].includes(result)) {
                return <span className="text-zinc-600 text-[11px]">—</span>;
            }
            const ms = row.getValue("response_latency_ms") as number;
            const color = ms < 500 ? "text-emerald-400" : ms < 2000 ? "text-amber-400" : "text-rose-400";
            return <div className={`font-mono text-[11px] ${color}`}>{ms}ms</div>
        },
    },
    {
        id: "cache",
        header: "Cache",
        cell: ({ row }) => {
            const hit = row.original.cache_hit;
            if (hit == null) return <span className="text-zinc-600 text-[11px]">—</span>;
            return hit ? (
                <Badge variant="outline" className="bg-emerald-500/10 text-emerald-400 border-emerald-500/20 text-[10px] px-1.5 py-0">
                    HIT
                </Badge>
            ) : (
                <Badge variant="outline" className="bg-zinc-500/10 text-zinc-400 border-zinc-500/20 text-[10px] px-1.5 py-0">
                    MISS
                </Badge>
            );
        },
    },
    {
        accessorKey: "estimated_cost_usd",
        header: "Cost",
        cell: ({ row }) => {
            const cost = row.getValue("estimated_cost_usd") as string | null;
            if (!cost || cost === "0") return <span className="text-zinc-600 text-[11px]">—</span>;
            return <div className="font-mono text-[11px] text-amber-400">${parseFloat(cost).toFixed(4)}</div>;
        },
    },
    {
        accessorKey: "finish_reason",
        header: "Finish",
        cell: ({ row }) => {
            const reason = row.getValue("finish_reason") as string | null;
            if (!reason) return <span className="text-zinc-600 text-[11px]">—</span>;
            const colorClass = finishReasonColor[reason] ?? "bg-white/[0.05] text-zinc-400 border-white/10";
            return (
                <Badge variant="outline" className={`text-[10px] px-1.5 py-0 ${colorClass}`}>
                    {reason}
                </Badge>
            );
        },
    },
    {
        id: "tools",
        header: "Tools",
        cell: ({ row }) => {
            const count = row.original.tool_call_count;
            if (!count) return <span className="text-zinc-600 text-[11px]">—</span>;
            return (
                <div className="flex items-center gap-1">
                    <Wrench className="h-3 w-3 text-violet-400" />
                    <span className="text-[11px] font-medium text-violet-400">{count}</span>
                </div>
            );
        },
    },
    {
        accessorKey: "error_type",
        header: "Error",
        cell: ({ row }) => {
            const errorType = row.getValue("error_type") as string | null;
            if (!errorType) return <span className="text-zinc-600 text-[11px]">—</span>;
            return (
                <Badge variant="outline" className="text-[10px] px-1.5 py-0 bg-rose-500/10 text-rose-400 border-rose-500/20">
                    {errorType.replace(/_/g, " ")}
                </Badge>
            );
        },
    },
    {
        id: "streaming",
        header: "",
        cell: ({ row }) => {
            if (!row.original.is_streaming) return null;
            return (
                <div title="Streaming response">
                    <Zap className="h-3.5 w-3.5 text-amber-400" />
                </div>
            );
        },
    },
    {
        accessorKey: "policy_result",
        header: "Policy",
        cell: ({ row }) => {
            const result = row.getValue("policy_result") as string;
            let variant: "default" | "destructive" | "warning" | "success" = "default";
            if (result === "allowed" || result === "approved") variant = "success";
            if (result === "denied" || result === "rejected") variant = "destructive";

            return <Badge variant={variant} className="capitalize text-[10px] px-1.5 py-0">{result}</Badge>
        }
    },
    {
        id: "user",
        header: "User",
        cell: ({ row }) => {
            const userId = row.original.user_id;
            if (!userId) return <span className="text-zinc-600 text-[11px]">—</span>;
            return (
                <div className="text-[11px] text-zinc-300 font-mono max-w-[80px] truncate" title={userId}>
                    {userId}
                </div>
            );
        },
    },
    {
        id: "actions",
        header: "",
        cell: () => (
            <Button variant="ghost" size="sm" className="h-7 w-7 p-0 text-zinc-500 hover:text-white hover:bg-white/5">
                <Eye className="h-3.5 w-3.5" />
            </Button>
        ),
    },
]

