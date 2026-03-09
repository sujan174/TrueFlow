"use client"

import { ColumnDef } from "@tanstack/react-table"
import { ApiKey } from "@/lib/api"
import { ArrowUpDown, Copy, MoreHorizontal, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { formatDistanceToNow } from "date-fns"
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { toast } from "sonner"

export const columns: ColumnDef<ApiKey>[] = [
    {
        accessorKey: "created_at",
        header: ({ column }) => {
            return (
                <Button
                    variant="ghost"
                    size="sm"
                    className="-ml-3 h-8 hover:bg-white/5 hover:text-white data-[state=open]:bg-white/5"
                    onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
                >
                    Created
                    <ArrowUpDown className="ml-2 h-3 w-3" />
                </Button>
            )
        },
        cell: ({ row }) => (
            <div className="text-zinc-500 text-[11px] whitespace-nowrap font-mono tracking-tight">
                {formatDistanceToNow(new Date(row.getValue("created_at")), { addSuffix: true })}
            </div>
        ),
    },
    {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => <div className="font-medium text-[13px] text-zinc-200">{row.getValue("name")}</div>,
    },
    {
        accessorKey: "role",
        header: "Role",
        cell: ({ row }) => (
            <div className="relative rounded bg-white/[0.03] border border-white/10 px-1.5 py-0.5 font-mono text-[10px] text-zinc-400 inline-block uppercase tracking-widest">
                {row.getValue("role")}
            </div>
        ),
    },
    {
        accessorKey: "key_prefix",
        header: "Prefix",
        cell: ({ row }) => <code className="text-[10px] text-zinc-400 bg-white/[0.03] border border-white/10 px-1.5 py-0.5 rounded font-mono">{row.getValue("key_prefix")}...</code>,
    },
    {
        accessorKey: "scopes",
        header: "Scopes",
        cell: ({ row }) => {
            const scopes = row.getValue("scopes") as string[];
            return <div className="text-[11px] text-zinc-500 max-w-[200px] truncate" title={scopes.join(", ")}>{scopes.join(", ")}</div>
        },
    },
    {
        accessorKey: "id",
        header: "ID",
        cell: ({ row }) => (
            <div className="flex items-center gap-2">
                <code className="relative rounded bg-white/[0.03] border border-white/10 px-1.5 py-0.5 font-mono text-[10px] text-zinc-400 truncate max-w-[100px]" title={row.getValue("id")}>
                    {row.getValue("id")}
                </code>
            </div>
        ),
    },
    {
        accessorKey: "is_active",
        header: "Status",
        cell: ({ row }) => {
            const isActive = row.getValue("is_active") as boolean;
            return (
                <div className="flex items-center gap-2">
                    <div className={`h-1.5 w-1.5 rounded-full ${isActive ? "bg-white shadow-[0_0_8px_rgba(255,255,255,0.5)]" : "bg-zinc-700"}`} />
                    <span className="text-[11px] font-medium uppercase tracking-wider text-zinc-400">
                        {isActive ? "Active" : "Revoked"}
                    </span>
                </div>
            )
        },
    },
    {
        id: "actions",
        cell: ({ row, table }) => {
            const key = row.original;
            const meta = table.options.meta as { onRevoke?: (k: ApiKey) => void } | undefined;

            return (
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-8 w-8 text-zinc-500 hover:text-white hover:bg-white/5">
                            <MoreHorizontal className="h-4 w-4" />
                        </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="bg-zinc-950 border-white/10 text-white">
                        <DropdownMenuItem
                            onClick={() => {
                                navigator.clipboard.writeText(key.id);
                                toast.success("Copied ID");
                            }}
                            className="focus:bg-white/5 focus:text-white cursor-pointer"
                        >
                            <Copy className="mr-2 h-3.5 w-3.5" />
                            Copy ID
                        </DropdownMenuItem>
                        {key.is_active && (
                            <DropdownMenuItem
                                className="text-rose-400 focus:text-rose-300 focus:bg-rose-500/10 cursor-pointer"
                                onClick={() => meta?.onRevoke?.(key)}
                            >
                                <Trash2 className="mr-2 h-3.5 w-3.5" />
                                Revoke Key
                            </DropdownMenuItem>
                        )}
                    </DropdownMenuContent>
                </DropdownMenu>
            )
        },
    },
]
