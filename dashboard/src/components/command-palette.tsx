"use client";

import * as React from "react";
import { useRouter } from "next/navigation";
import { Search, Calculator, User, CreditCard, Settings, FileText, CheckCircle2, Shield, Key, Zap } from "lucide-react";

import {
    Dialog,
    DialogContent,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

export function CommandPalette() {
    const [open, setOpen] = React.useState(false);
    const [query, setQuery] = React.useState("");
    const router = useRouter();

    React.useEffect(() => {
        const down = (e: KeyboardEvent) => {
            if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                setOpen((open) => !open);
            }
        };
        document.addEventListener("keydown", down);
        return () => document.removeEventListener("keydown", down);
    }, []);

    const runCommand = React.useCallback((command: () => void) => {
        setOpen(false);
        command();
    }, []);

    const groups: { heading: string; items: { icon: React.ElementType; name: string; shortcut?: string; action: () => void }[] }[] = [
        {
            heading: "Navigation",
            items: [
                { icon: Search, name: "Audit Logs", shortcut: "G T", action: () => router.push("/audit") },
                { icon: Key, name: "Agents", shortcut: "G K", action: () => router.push("/virtual-keys") },
                { icon: Shield, name: "Guardrails", shortcut: "G P", action: () => router.push("/guardrails") },
                { icon: CheckCircle2, name: "Approvals", shortcut: "G A", action: () => router.push("/approvals") },
                { icon: Calculator, name: "Analytics", shortcut: "G Y", action: () => router.push("/analytics") },
                { icon: CreditCard, name: "Vault", shortcut: "G C", action: () => router.push("/vault") },
                { icon: Zap, name: "Playground", shortcut: "G L", action: () => router.push("/playground") },
                { icon: Settings, name: "Settings", shortcut: "G S", action: () => router.push("/settings") },
            ]
        },
        {
            heading: "Actions",
            items: [
                { icon: FileText, name: "View Documentation", action: () => window.open("https://docs.trueflow.app", "_blank") },
            ]
        }
    ];

    const filteredGroups = groups.map(group => ({
        ...group,
        items: group.items.filter(item => item.name.toLowerCase().includes(query.toLowerCase()))
    })).filter(group => group.items.length > 0);

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent className="p-0 overflow-hidden shadow-2xl bg-card border-none max-w-lg">
                <div className="flex items-center border-b px-3">
                    <Search className="mr-2 h-4 w-4 shrink-0 opacity-50" />
                    <input
                        className="flex h-11 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50"
                        placeholder="Type a command or search..."
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                    />
                </div>
                <div className="max-h-[300px] overflow-y-auto p-2">
                    {filteredGroups.length === 0 ? (
                        <p className="text-[13px] text-muted-foreground text-center py-6">No results found.</p>
                    ) : (
                        filteredGroups.map((group, i) => (
                            <div key={i} className="mb-2">
                                <h4 className="px-2 py-1 text-xs font-medium text-muted-foreground">{group.heading}</h4>
                                {group.items.map((item, j) => (
                                    <button
                                        key={j}
                                        onClick={() => runCommand(item.action)}
                                        className="flex w-full items-center rounded-md px-2 py-2 text-sm text-foreground hover:bg-accent hover:text-accent-foreground aria-selected:bg-accent aria-selected:text-accent-foreground group transition-colors"
                                    >
                                        <item.icon className="mr-2 h-4 w-4" />
                                        <span>{item.name}</span>
                                        {item.shortcut && (
                                            <span className="ml-auto text-xs tracking-widest text-muted-foreground opacity-60 group-hover:opacity-100">
                                                {item.shortcut}
                                            </span>
                                        )}
                                    </button>
                                ))}
                            </div>
                        ))
                    )}
                </div>
                <div className="border-t p-2 text-xs text-muted-foreground flex justify-end px-4">
                    <kbd className="pointer-events-none inline-flex h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground opacity-100">
                        <span className="text-xs">⌘</span>K
                    </kbd>
                </div>
            </DialogContent>
        </Dialog>
    );
}
