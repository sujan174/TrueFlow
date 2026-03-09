"use client"

import { ColumnDef } from "@tanstack/react-table"
import { Token } from "@/lib/api"
import Link from "next/link"
import { ArrowUpDown, CheckCircle, XCircle, MoreHorizontal, Trash2, Copy, BarChart } from "lucide-react"
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

export const columns: ColumnDef<Token>[] = [
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
        accessorKey: "id",
        header: "Token ID",
        cell: ({ row }) => (
            <div className="flex items-center gap-2">
                <code className="relative rounded bg-white/[0.03] border border-white/10 px-1.5 py-0.5 font-mono text-[10px] text-zinc-400">
                    {row.getValue("id")}
                </code>
                <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5 opacity-0 group-hover:opacity-100 hover:bg-white/10 text-zinc-500 hover:text-white transition-all"
                    onClick={(e) => {
                        e.stopPropagation();
                        navigator.clipboard.writeText(row.getValue("id"));
                        toast.success("Copied Token ID");
                    }}
                >
                    <Copy className="h-3 w-3" />
                </Button>
            </div>
        ),
    },
    {
        accessorKey: "credential_id",
        header: "Credential",
        cell: ({ row }) => (
            <code className="relative rounded bg-white/[0.02] border border-white/[0.05] px-1.5 py-0.5 font-mono text-[10px] text-zinc-500 truncate max-w-[120px]" title={row.getValue("credential_id")}>
                {row.getValue("credential_id") || "passthrough"}
            </code>
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
            const token = row.original;
            const meta = table.options.meta as { onRevoke?: (t: Token) => void } | undefined;

            return (
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-8 w-8 text-zinc-500 hover:text-white hover:bg-white/5">
                            <MoreHorizontal className="h-4 w-4" />
                        </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="bg-zinc-950 border-white/10 text-white">
                        <DropdownMenuItem
                            onClick={(e) => {
                                e.stopPropagation();
                                navigator.clipboard.writeText(token.id);
                                toast.success("Copied");
                            }}
                            className="focus:bg-white/5 focus:text-white cursor-pointer"
                        >
                            <Copy className="mr-2 h-3.5 w-3.5" />
                            Copy ID
                        </DropdownMenuItem>
                        <DropdownMenuItem asChild className="focus:bg-white/5 focus:text-white cursor-pointer">
                            <Link href={`/analytics/token/${token.id}`} onClick={(e) => e.stopPropagation()}>
                                <BarChart className="mr-2 h-3.5 w-3.5" />
                                View Analytics
                            </Link>
                        </DropdownMenuItem>
                        {token.is_active && (
                            <DropdownMenuItem
                                className="text-rose-400 focus:text-rose-300 focus:bg-rose-500/10 cursor-pointer"
                                onClick={(e) => {
                                    e.stopPropagation();
                                    meta?.onRevoke?.(token);
                                }}
                            >
                                <Trash2 className="mr-2 h-3.5 w-3.5" />
                                Revoke Token
                            </DropdownMenuItem>
                        )}
                    </DropdownMenuContent>
                </DropdownMenu>
            )
        },
    },
]
