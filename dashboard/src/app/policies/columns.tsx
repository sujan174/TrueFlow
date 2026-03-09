"use client"

import { ColumnDef } from "@tanstack/react-table"
import { Policy } from "@/lib/api"
import { ArrowUpDown, Trash2, Copy, Pencil, MoreHorizontal, Eye, ShieldCheck, ShieldAlert, ShieldBan, Zap, Clock, FileText } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { formatDistanceToNow } from "date-fns"
import { deletePolicy } from "@/lib/api"
import { toast } from "sonner"
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
    DialogClose
} from "@/components/ui/dialog"
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
    DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu"

// Helpers to analyze policy rules
function getRulesSummary(rules: unknown[]): string {
    if (!rules || rules.length === 0) return "No rules";
    const count = rules.length;
    const types = new Set<string>();
    for (const rule of rules) {
        const r = rule as Record<string, unknown>;
        const action = r.action as Record<string, string> | undefined;
        if (action) {
            const actionType = Object.keys(action)[0];
            if (actionType) types.add(actionType);
        }
        // Handle legacy format
        if (r.type) types.add(r.type as string);
    }
    if (types.size === 0) return `${count} rule${count > 1 ? "s" : ""}`;
    return `${count} rule${count > 1 ? "s" : ""} · ${[...types].join(", ")}`;
}

function getActionIcons(rules: unknown[]): string[] {
    const icons: string[] = [];
    for (const rule of rules) {
        const r = rule as Record<string, unknown>;
        const action = r.action as Record<string, unknown> | undefined;
        if (!action) {
            if (r.type === "spend_cap") icons.push("spend");
            if (r.type === "rate_limit") icons.push("rate");
            continue;
        }
        const type = Object.keys(action)[0];
        if (type && !icons.includes(type)) icons.push(type);
    }
    return icons;
}

function ActionIcon({ type }: { type: string }) {
    switch (type) {
        case "Deny": return <ShieldBan className="h-3 w-3 text-rose-400" />;
        case "RequireApproval": return <ShieldCheck className="h-3 w-3 text-amber-400" />;
        case "RateLimit": case "rate": return <Zap className="h-3 w-3 text-blue-400" />;
        case "Redact": return <ShieldAlert className="h-3 w-3 text-violet-400" />;
        case "Transform": return <FileText className="h-3 w-3 text-cyan-400" />;
        case "Log": return <FileText className="h-3 w-3 text-emerald-400" />;
        case "Throttle": return <Clock className="h-3 w-3 text-orange-400" />;
        case "spend": return <ShieldBan className="h-3 w-3 text-amber-400" />;
        default: return <ShieldAlert className="h-3 w-3 text-muted-foreground" />;
    }
}

export const columns: ColumnDef<Policy>[] = [
    {
        accessorKey: "name",
        header: ({ column }) => (
            <Button
                variant="ghost"
                size="sm"
                className="-ml-3 h-8 hover:bg-white/5 hover:text-white data-[state=open]:bg-white/5"
                onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            >
                Policy
                <ArrowUpDown className="ml-1.5 h-3 w-3" />
            </Button>
        ),
        cell: ({ row }) => {
            const policy = row.original;
            const actionTypes = getActionIcons(policy.rules);
            return (
                <div className="flex items-center gap-3 min-w-[200px]">
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-white/[0.02] border border-white/[0.05]">
                        {actionTypes.length > 0 ? <ActionIcon type={actionTypes[0]} /> : <ShieldAlert className="h-3.5 w-3.5 text-zinc-500" />}
                    </div>
                    <div className="min-w-0">
                        <p className="font-medium text-[13px] text-zinc-200 truncate">{policy.name}</p>
                        <p className="text-[11px] text-zinc-500 truncate mt-0.5">
                            {getRulesSummary(policy.rules)}
                        </p>
                    </div>
                </div>
            );
        },
    },
    {
        accessorKey: "mode",
        header: "Mode",
        cell: ({ row }) => {
            const mode = row.getValue("mode") as string;
            return (
                <Badge
                    variant={mode === "enforce" ? "destructive" : mode === "shadow" ? "warning" : "secondary"}
                    dot
                    className="capitalize text-[11px]"
                >
                    {mode}
                </Badge>
            );
        },
    },
    {
        id: "actions_summary",
        header: "Actions",
        cell: ({ row }) => {
            const actionTypes = getActionIcons(row.original.rules);
            if (actionTypes.length === 0) return <span className="text-zinc-600 text-xs">—</span>;
            return (
                <div className="flex items-center gap-1">
                    {actionTypes.map((type) => (
                        <div key={type} className="flex items-center gap-1 rounded bg-white/[0.03] border border-white/5 px-1.5 py-0.5" title={type}>
                            <ActionIcon type={type} />
                            <span className="text-[10px] text-zinc-400 capitalize">{type}</span>
                        </div>
                    ))}
                </div>
            );
        },
    },
    {
        accessorKey: "is_active",
        header: "Status",
        cell: ({ row }) => {
            const active = row.getValue("is_active") as boolean;
            return (
                <div className="flex items-center gap-2">
                    <div className={`h-1.5 w-1.5 rounded-full ${active ? "bg-white shadow-[0_0_8px_rgba(255,255,255,0.5)]" : "bg-zinc-700"}`} />
                    <span className="text-[11px] font-medium uppercase tracking-wider text-zinc-400">
                        {active ? "Active" : "Disabled"}
                    </span>
                </div>
            );
        },
    },
    {
        accessorKey: "created_at",
        header: ({ column }) => (
            <Button
                variant="ghost"
                size="sm"
                className="-ml-3 h-8 hover:bg-white/5 hover:text-white data-[state=open]:bg-white/5"
                onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            >
                Created
                <ArrowUpDown className="ml-1.5 h-3 w-3" />
            </Button>
        ),
        cell: ({ row }) => (
            <div className="text-zinc-500 text-[11px] whitespace-nowrap font-mono tracking-tight">
                {formatDistanceToNow(new Date(row.getValue("created_at")), { addSuffix: true })}
            </div>
        ),
    },
    {
        id: "actions",
        cell: ({ row, table }) => {
            const policy = row.original;
            const meta = table.options.meta as { onView?: (p: Policy) => void; onEdit?: (p: Policy) => void; onRefresh?: () => void } | undefined;

            return (
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-8 w-8 text-zinc-500 hover:text-white hover:bg-white/5">
                            <MoreHorizontal className="h-4 w-4" />
                        </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="bg-zinc-950 border-white/10 text-white w-40">
                        <DropdownMenuItem onClick={() => meta?.onView?.(policy)} className="focus:bg-white/5 focus:text-white cursor-pointer">
                            <Eye className="mr-2 h-3.5 w-3.5" />
                            View Details
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => meta?.onEdit?.(policy)} className="focus:bg-white/5 focus:text-white cursor-pointer">
                            <Pencil className="mr-2 h-3.5 w-3.5" />
                            Edit
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => {
                            navigator.clipboard.writeText(policy.id);
                            toast.success("Policy ID copied");
                        }} className="focus:bg-white/5 focus:text-white cursor-pointer">
                            <Copy className="mr-2 h-3.5 w-3.5" />
                            Copy ID
                        </DropdownMenuItem>
                        <DropdownMenuSeparator className="bg-white/10" />
                        <Dialog>
                            <DialogTrigger asChild>
                                <DropdownMenuItem onSelect={(e) => e.preventDefault()} className="text-rose-400 focus:text-rose-300 focus:bg-rose-500/10 cursor-pointer">
                                    <Trash2 className="mr-2 h-3.5 w-3.5" />
                                    Delete
                                </DropdownMenuItem>
                            </DialogTrigger>
                            <DialogContent className="bg-zinc-950 border-white/10">
                                <DialogHeader>
                                    <DialogTitle className="text-white">Delete Policy?</DialogTitle>
                                    <DialogDescription className="text-zinc-500">
                                        This will permanently delete <span className="font-bold text-white">{policy.name}</span> and stop enforcing its rules.
                                    </DialogDescription>
                                </DialogHeader>
                                <DialogFooter>
                                    <DialogClose asChild>
                                        <Button variant="ghost" size="sm" className="text-zinc-400 hover:text-white hover:bg-white/5">Cancel</Button>
                                    </DialogClose>
                                    <Button
                                        variant="destructive"
                                        size="sm"
                                        onClick={async () => {
                                            try {
                                                await deletePolicy(policy.id);
                                                toast.success("Policy deleted");
                                                meta?.onRefresh?.();
                                            } catch {
                                                toast.error("Failed to delete");
                                            }
                                        }}
                                    >
                                        Delete
                                    </Button>
                                </DialogFooter>
                            </DialogContent>
                        </Dialog>
                    </DropdownMenuContent>
                </DropdownMenu>
            );
        },
    },
]
