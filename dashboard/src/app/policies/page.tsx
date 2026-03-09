"use client";

import { useState, useEffect, useCallback } from "react";
import useSWR from "swr";
import { listPolicies, createPolicy, updatePolicy, deletePolicy, Policy, swrFetcher } from "@/lib/api";
import {
    RefreshCw, Plus, ShieldCheck, ShieldAlert, Eye, X,
    ChevronRight, ShieldBan, Zap, Clock, FileText, Code2,
    AlertTriangle, Check, Copy, Layers, Filter, Tag
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DataTable } from "@/components/data-table";
import { columns } from "./columns";
import { PolicyHistoryDialog } from "@/components/policy-history";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
    DialogFooter,
    DialogClose
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { CountUp } from "@/components/ui/count-up";
// Native select styled to match the design system (used with <option> children)
const Select = ({ className, children, ...props }: React.SelectHTMLAttributes<HTMLSelectElement>) => (
    <select
        className={cn(
            "flex h-9 w-full items-center rounded-md border border-white/10 bg-black px-3 py-1 text-sm text-zinc-300 shadow-sm transition-colors focus:outline-none focus:ring-1 focus:ring-white/20 disabled:cursor-not-allowed disabled:opacity-50",
            className
        )}
        {...props}
    >
        {children}
    </select>
);
import { formatDistanceToNow } from "date-fns";

// ── Constants ─────────────────────────────────────

const OPERATORS = [
    { value: "Eq", label: "equals" },
    { value: "Neq", label: "not equals" },
    { value: "Gt", label: ">" },
    { value: "Gte", label: ">=" },
    { value: "Lt", label: "<" },
    { value: "Lte", label: "<=" },
    { value: "Contains", label: "contains" },
    { value: "StartsWith", label: "starts with" },
    { value: "EndsWith", label: "ends with" },
    { value: "Regex", label: "matches regex" },
    { value: "In", label: "in list" },
    { value: "Exists", label: "exists" },
];

const FIELD_SUGGESTIONS = [
    { group: "Request", fields: ["request.method", "request.path", "request.body_size", "request.header.content-type"] },
    { group: "Agent", fields: ["agent.name"] },
    { group: "Token", fields: ["token.id", "token.name"] },
    { group: "Usage", fields: ["usage.spend_today_usd", "usage.spend_month_usd"] },
    { group: "Response", fields: ["response.status", "response.body.error"] },
    { group: "Context", fields: ["context.time.hour", "context.time.weekday", "context.ip"] },
];

const ACTION_TYPES = [
    { value: "Deny", label: "Deny", icon: ShieldBan, color: "text-rose-400", desc: "Block the request" },
    { value: "RequireApproval", label: "HITL Approval", icon: ShieldCheck, color: "text-amber-400", desc: "Require human approval" },
    { value: "RateLimit", label: "Rate Limit", icon: Zap, color: "text-blue-400", desc: "Limit request rate" },
    { value: "Redact", label: "Redact PII", icon: ShieldAlert, color: "text-violet-400", desc: "Scrub sensitive data" },
    { value: "ToolScope", label: "Tool RBAC", icon: Code2, color: "text-indigo-400", desc: "Allow/deny specific tool names" },
    { value: "Transform", label: "Transform", icon: FileText, color: "text-cyan-400", desc: "Modify request/response" },
    { value: "Log", label: "Log", icon: FileText, color: "text-emerald-400", desc: "Log event" },
    { value: "Throttle", label: "Throttle", icon: Clock, color: "text-orange-400", desc: "Add delay" },
    { value: "Tag", label: "Tag", icon: Tag, color: "text-pink-400", desc: "Add metadata tag" },
];

// ── Main Page ─────────────────────────────────────

// ── Main Page ─────────────────────────────────────

const EMPTY_POLICIES: Policy[] = [];

export default function PoliciesPage() {
    const { data: policiesData, isLoading, mutate } = useSWR<Policy[]>("/policies", swrFetcher);
    const policies = policiesData || EMPTY_POLICIES;

    const [createOpen, setCreateOpen] = useState(false);
    const [detailPolicy, setDetailPolicy] = useState<Policy | null>(null);
    const [editPolicy, setEditPolicy] = useState<Policy | null>(null);
    const [historyPolicy, setHistoryPolicy] = useState<Policy | null>(null);

    const blockingCount = policies.filter(p => p.mode === 'enforce').length;
    const shadowCount = policies.filter(p => p.mode === 'shadow').length;
    const totalRules = policies.reduce((sum, p) => sum + (p.rules?.length || 0), 0);

    return (
        <div className="space-y-4">
            {/* Header */}
            <div className="flex items-center justify-between gap-4">
                <div>
                    <h1 className="text-lg font-semibold tracking-tight text-white">Policies</h1>
                    <p className="text-xs text-zinc-500 mt-0.5">Traffic control rules for conditions and actions.</p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                    <Button variant="outline" size="sm" onClick={() => mutate()} disabled={isLoading}>
                        <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", isLoading && "animate-spin")} />
                        Refresh
                    </Button>
                    <Dialog open={createOpen} onOpenChange={setCreateOpen}>
                        <DialogTrigger asChild>
                            <Button size="sm">
                                <Plus className="mr-1.5 h-3.5 w-3.5" /> New Policy
                            </Button>
                        </DialogTrigger>
                        <DialogContent className="sm:max-w-[680px] max-h-[85vh] overflow-y-auto bg-zinc-950 border-white/10 p-0">
                            <div className="p-6">
                                <PolicyFormDialog
                                    mode="create"
                                    onSuccess={() => { setCreateOpen(false); mutate(); }}
                                />
                            </div>
                        </DialogContent>
                    </Dialog>
                </div>
            </div>

            {/* KPI Strip */}
            <div className="grid gap-4 md:grid-cols-4 animate-slide-up">
                <KPIMini icon={Layers} value={policies.length} label="Total Policies" color="blue" loading={isLoading} />
                <KPIMini icon={ShieldBan} value={blockingCount} label="Blocking" color="rose" loading={isLoading} />
                <KPIMini icon={Eye} value={shadowCount} label="Shadow Mode" color="amber" loading={isLoading} />
                <KPIMini icon={Filter} value={totalRules} label="Total Rules" color="violet" loading={isLoading} />
            </div>

            {/* Table */}
            <div className="animate-slide-up stagger-2">
                {isLoading && policies.length === 0 ? (
                    <div className="space-y-4">
                        <div className="h-10 w-[300px] bg-white/5 border border-white/10 rounded-md shimmer" />
                        <div className="rounded-md border border-white/10 bg-white/[0.02] h-[400px] shimmer" />
                    </div>
                ) : (
                    <div className="rounded-md border border-white/10 bg-black overflow-hidden shadow-sm">
                        <DataTable
                            columns={columns}
                            data={policies}
                            searchKey="name"
                            searchPlaceholder="Search policies..."
                            meta={{
                                onView: (p: Policy) => setDetailPolicy(p),
                                onEdit: (p: Policy) => setEditPolicy(p),
                                onRefresh: mutate,
                            }}
                        />
                    </div>
                )}
            </div>

            {/* Detail Panel */}
            {detailPolicy && (
                <PolicyDetailPanel
                    policy={detailPolicy}
                    onClose={() => setDetailPolicy(null)}
                    onEdit={() => { setEditPolicy(detailPolicy); setDetailPolicy(null); }}
                    onHistory={() => setHistoryPolicy(detailPolicy)}
                />
            )}

            {/* Edit Dialog */}
            {editPolicy && (
                <Dialog open={!!editPolicy} onOpenChange={(open) => !open && setEditPolicy(null)}>
                    <DialogContent className="sm:max-w-[680px] max-h-[85vh] overflow-y-auto bg-zinc-950 border-white/10 p-0">
                        <div className="p-6">
                            <PolicyFormDialog
                                mode="edit"
                                initialPolicy={editPolicy}
                                onSuccess={() => { setEditPolicy(null); mutate(); }}
                            />
                        </div>
                    </DialogContent>
                </Dialog>
            )}

            {/* History Dialog */}
            {historyPolicy && (
                <PolicyHistoryDialog
                    policyId={historyPolicy.id}
                    open={!!historyPolicy}
                    onOpenChange={(open) => !open && setHistoryPolicy(null)}
                />
            )}
        </div>
    );
}

// ── KPI Mini Card ─────────────────────────────────

function KPIMini({ icon: Icon, value, label, color, loading }: {
    icon: React.ComponentType<{ className?: string }>;
    value: number;
    label: string;
    color: "blue" | "rose" | "amber" | "violet";
    loading?: boolean;
}) {
    const bgColors = {
        blue: "text-blue-400 bg-blue-500/10 border-blue-500/20",
        rose: "text-rose-400 bg-rose-500/10 border-rose-500/20",
        amber: "text-amber-400 bg-amber-500/10 border-amber-500/20",
        violet: "text-violet-400 bg-violet-500/10 border-violet-500/20",
    };
    return (
        <div className="machined-card rounded-lg p-5">
            <div className="flex items-center gap-4">
                <div className={cn("flex h-10 w-10 shrink-0 items-center justify-center rounded-full border", bgColors[color])}>
                    <Icon className="h-5 w-5" />
                </div>
                <div>
                    {loading ? (
                        <div className="h-7 w-16 bg-white/5 rounded shimmer my-0.5" />
                    ) : (
                        <p className="text-2xl font-semibold tabular-nums text-white leading-none tracking-tight">
                            <CountUp value={value} />
                        </p>
                    )}
                    <p className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest mt-1">{label}</p>
                </div>
            </div>
        </div>
    );
}

// ── Policy Detail Panel (Slide-over) ──────────────

function PolicyDetailPanel({ policy, onClose, onEdit, onHistory }: {
    policy: Policy;
    onClose: () => void;
    onEdit: () => void;
    onHistory: () => void;
}) {
    return (
        <div className="fixed inset-y-0 right-0 w-[480px] z-50 bg-black/95 backdrop-blur-xl border-l border-white/10 shadow-2xl flex flex-col animate-slide-in-right">
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-white/10">
                <div className="flex items-center gap-3 min-w-0">
                    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-white/5 border border-white/10">
                        <ShieldAlert className="h-4 w-4 text-white" />
                    </div>
                    <div className="min-w-0">
                        <h3 className="font-semibold text-sm truncate text-white">{policy.name}</h3>
                        <p className="text-[11px] text-zinc-500 font-mono truncate">{policy.id}</p>
                    </div>
                </div>
                <div className="flex items-center gap-1">
                    <Button variant="ghost" size="icon" className="h-7 w-7 text-zinc-500 hover:text-white hover:bg-white/5" onClick={() => {
                        navigator.clipboard.writeText(policy.id);
                        toast.success("Copied");
                    }}>
                        <Copy className="h-3.5 w-3.5" />
                    </Button>
                    <Button variant="ghost" size="icon" className="h-7 w-7 text-zinc-500 hover:text-white hover:bg-white/5" onClick={onHistory} title="View History">
                        <Clock className="h-4 w-4" />
                    </Button>
                    <Button variant="ghost" size="icon" className="h-7 w-7 text-zinc-500 hover:text-white hover:bg-white/5" onClick={onClose}>
                        <X className="h-4 w-4" />
                    </Button>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-4 space-y-6">
                {/* Meta */}
                <div className="grid grid-cols-2 gap-4">
                    <div>
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest mb-1">Mode</p>
                        <Badge variant={policy.mode === "enforce" ? "destructive" : "warning"} dot className="capitalize">
                            {policy.mode}
                        </Badge>
                    </div>
                    <div>
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest mb-1">Status</p>
                        <Badge variant={policy.is_active ? "success" : "secondary"} dot>
                            {policy.is_active ? "Active" : "Disabled"}
                        </Badge>
                    </div>
                    <div>
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest mb-1">Created</p>
                        <p className="text-[13px] font-mono text-zinc-300">{formatDistanceToNow(new Date(policy.created_at), { addSuffix: true })}</p>
                    </div>
                    <div>
                        <p className="text-[11px] text-zinc-500 uppercase tracking-widest mb-1">Rules</p>
                        <p className="text-[13px] font-mono text-zinc-300">{policy.rules?.length || 0}</p>
                    </div>
                </div>

                {/* Rules */}
                <div>
                    <p className="text-[11px] text-zinc-500 uppercase tracking-widest mb-3">Rules</p>
                    <div className="space-y-3">
                        {(policy.rules || []).map((rule, idx) => (
                            <RuleCard key={idx} rule={rule as Record<string, unknown>} index={idx} />
                        ))}
                        {(!policy.rules || policy.rules.length === 0) && (
                            <div className="text-center py-8 text-zinc-600">
                                <Filter className="h-6 w-6 mx-auto mb-2 opacity-30" />
                                <p className="text-[11px] uppercase tracking-widest">No rules defined</p>
                            </div>
                        )}
                    </div>
                </div>

                {/* Raw JSON */}
                <div>
                    <p className="text-[11px] text-zinc-500 uppercase tracking-widest mb-2">Raw Definition</p>
                    <pre className="bg-white/[0.02] border border-white/10 rounded-md p-3 text-[11px] font-mono text-zinc-400 overflow-x-auto max-h-[200px]">
                        {JSON.stringify(policy.rules, null, 2)}
                    </pre>
                </div>
            </div>

            {/* Footer */}
            <div className="border-t border-white/10 px-6 py-3 flex items-center gap-2">
                <Button size="sm" className="flex-1" onClick={onEdit}>
                    Edit Policy
                </Button>
                <Button size="sm" variant="outline" onClick={onClose}>
                    Close
                </Button>
            </div>
        </div >
    );
}

// ── Rule Card (inside detail panel) ───────────────

function RuleCard({ rule, index }: { rule: Record<string, unknown>; index: number }) {
    const condition = rule.condition as Record<string, unknown> | undefined;
    const action = rule.action as Record<string, unknown> | undefined;

    // Legacy format detection
    if (rule.type) {
        return (
            <div className="rounded-md border border-white/10 bg-white/[0.02] p-3">
                <div className="flex items-center gap-2 mb-2">
                    <span className="text-[10px] font-mono text-zinc-500 bg-black border border-white/10 px-1.5 py-0.5 rounded">
                        #{index + 1}
                    </span>
                    <Badge variant="secondary" className="text-[10px]">{rule.type as string}</Badge>
                </div>
                <pre className="text-[11px] font-mono text-zinc-500 overflow-x-auto">{JSON.stringify(rule, null, 2)}</pre>
            </div>
        );
    }

    const actionType = action ? Object.keys(action)[0] : "unknown";
    const at = ACTION_TYPES.find(a => a.value === actionType);

    return (
        <div className="rounded-md border border-white/10 bg-white/[0.02] p-3 space-y-2">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <span className="text-[10px] font-mono text-zinc-500 bg-black border border-white/10 px-1.5 py-0.5 rounded">
                        #{index + 1}
                    </span>
                    {at && <at.icon className={cn("h-3.5 w-3.5", at.color)} />}
                    <span className="text-xs font-medium text-white">{at?.label || actionType}</span>
                </div>
                {Boolean(rule.phase) && (
                    <Badge variant="outline" className="text-[10px]">{String(rule.phase)}</Badge>
                )}
            </div>

            {/* Condition */}
            {condition && (
                <div className="flex items-start gap-2">
                    <span className="text-[10px] uppercase text-zinc-500 font-semibold mt-0.5 shrink-0 w-8 tracking-widest">IF</span>
                    <pre className="text-[11px] font-mono text-zinc-400 bg-black border border-white/10 rounded px-2 py-1 flex-1 overflow-x-auto">
                        {JSON.stringify(condition, null, 1)}
                    </pre>
                </div>
            )}

            {/* Action */}
            {action && (
                <div className="flex items-start gap-2">
                    <span className="text-[10px] uppercase text-zinc-500 font-semibold mt-0.5 shrink-0 w-8 tracking-widest">DO</span>
                    <pre className="text-[11px] font-mono text-zinc-400 bg-black border border-white/10 rounded px-2 py-1 flex-1 overflow-x-auto">
                        {JSON.stringify(action, null, 1)}
                    </pre>
                </div>
            )}
        </div>
    );
}

// ── Policy Form (Create / Edit) ───────────────────

interface RuleForm {
    conditionMode: "always" | "check";
    field: string;
    operator: string;
    value: string;
    actionType: string;
    // Deny
    denyMessage: string;
    denyStatus: number;
    // RateLimit
    rateMax: number;
    rateWindow: string;
    rateKey: string;
    // Redact
    redactDirection: string;
    redactPatterns: string;
    redactFields: string;
    // HITL
    hitlTimeout: number;
    // Throttle
    throttleMs: number;
    // Log
    logLevel: string;
    logTags: string;
    // Tag
    tagKey: string;
    tagValue: string;
    // Transform
    transformOps: string;
    // ToolScope (Tool RBAC)
    toolScopeAllowed: string;   // comma-separated allowed tool patterns (allowlist)
    toolScopeBlocked: string;   // comma-separated blocked tool patterns (denylist)
    toolScopeDenyMessage: string;
    // Phase
    phase: string;
}

function emptyRule(): RuleForm {
    return {
        conditionMode: "always",
        field: "",
        operator: "Gt",
        value: "",
        actionType: "Deny",
        denyMessage: "Request blocked by policy",
        denyStatus: 403,
        rateMax: 100,
        rateWindow: "60s",
        rateKey: "token",
        redactDirection: "Request",
        redactPatterns: "email,ssn",
        redactFields: "",
        hitlTimeout: 300,
        throttleMs: 2000,
        logLevel: "info",
        logTags: "",
        tagKey: "",
        tagValue: "",
        transformOps: "",
        toolScopeAllowed: "",
        toolScopeBlocked: "",
        toolScopeDenyMessage: "Tool not permitted by policy",
        phase: "pre",
    };
}

function PolicyFormDialog({ mode, initialPolicy, onSuccess }: {
    mode: "create" | "edit";
    initialPolicy?: Policy;
    onSuccess: () => void;
}) {
    const [saving, setSaving] = useState(false);
    const [name, setName] = useState(initialPolicy?.name || "");
    const [policyMode, setPolicyMode] = useState(initialPolicy?.mode || "enforce");
    const [inputMode, setInputMode] = useState<"visual" | "json">("visual");
    const [rules, setRules] = useState<RuleForm[]>([emptyRule()]);
    const [jsonRules, setJsonRules] = useState("[]");

    useEffect(() => {
        if (initialPolicy?.rules) {
            setJsonRules(JSON.stringify(initialPolicy.rules, null, 2));
        }
    }, [initialPolicy]);

    const addRule = () => setRules([...rules, emptyRule()]);
    const removeRule = (idx: number) => setRules(rules.filter((_, i) => i !== idx));

    const updateRule = (idx: number, updates: Partial<RuleForm>) => {
        setRules(rules.map((r, i) => i === idx ? { ...r, ...updates } : r));
    };

    function buildCondition(rule: RuleForm) {
        if (rule.conditionMode === "always") return { Always: true };
        return {
            Check: {
                field: rule.field,
                operator: rule.operator,
                value: rule.value,
            }
        };
    }

    function buildAction(rule: RuleForm) {
        switch (rule.actionType) {
            case "Deny":
                return { Deny: { status: rule.denyStatus, message: rule.denyMessage } };
            case "RateLimit":
                return { RateLimit: { max: rule.rateMax, window: rule.rateWindow, key: rule.rateKey } };
            case "Redact":
                return {
                    Redact: {
                        direction: rule.redactDirection,
                        patterns: rule.redactPatterns.split(",").map(s => s.trim()).filter(Boolean),
                        fields: rule.redactFields.split(",").map(s => s.trim()).filter(Boolean),
                    }
                };
            case "RequireApproval":
                return { RequireApproval: { timeout_secs: rule.hitlTimeout } };
            case "Throttle":
                return { Throttle: { delay_ms: rule.throttleMs } };
            case "Log":
                return {
                    Log: {
                        level: rule.logLevel,
                        tags: rule.logTags.split(",").map(s => s.trim()).filter(Boolean),
                    }
                };
            case "Tag":
                return { Tag: { key: rule.tagKey, value: rule.tagValue } };
            case "Transform":
                try { return { Transform: { operations: JSON.parse(rule.transformOps || "[]") } }; }
                catch { return { Transform: { operations: [] } }; }
            case "ToolScope":
                return {
                    ToolScope: {
                        allowed_tools: rule.toolScopeAllowed.split(",").map(s => s.trim()).filter(Boolean),
                        blocked_tools: rule.toolScopeBlocked.split(",").map(s => s.trim()).filter(Boolean),
                        deny_message: rule.toolScopeDenyMessage,
                    }
                };
            default:
                return { Deny: { status: 403, message: "Unknown action" } };
        }
    }

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            setSaving(true);
            let finalRules;
            if (inputMode === "json") {
                finalRules = JSON.parse(jsonRules);
            } else {
                finalRules = rules.map(r => ({
                    condition: buildCondition(r),
                    action: buildAction(r),
                    phase: r.phase,
                }));
            }

            if (mode === "edit" && initialPolicy) {
                await updatePolicy(initialPolicy.id, { name, mode: policyMode, rules: finalRules });
                toast.success("Policy updated");
            } else {
                await createPolicy({ name, mode: policyMode, rules: finalRules });
                toast.success("Policy created");
            }
            onSuccess();
        } catch (err) {
            toast.error(mode === "edit" ? "Failed to update policy" : "Failed to create policy");
            console.error(err);
        } finally {
            setSaving(false);
        }
    };

    return (
        <form onSubmit={handleSubmit}>
            <DialogHeader>
                <DialogTitle className="text-white">{mode === "edit" ? "Edit Policy" : "Create Policy"}</DialogTitle>
                <DialogDescription className="text-zinc-500">
                    {mode === "edit"
                        ? "Modify the rules and configuration for this policy."
                        : "Define condition → action rules for traffic control and AI safety."}
                </DialogDescription>
            </DialogHeader>

            <div className="space-y-6 py-4">
                {/* Name + Mode */}
                <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-1.5">
                        <Label htmlFor="name" className="text-xs text-zinc-400">Policy Name</Label>
                        <Input
                            id="name"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            placeholder="e.g. PII Protection"
                            required
                        />
                    </div>
                    <div className="space-y-1.5">
                        <Label htmlFor="mode" className="text-xs text-zinc-400">Mode</Label>
                        <Select value={policyMode} onChange={(e) => setPolicyMode(e.target.value)}>
                            <option value="enforce">🔒 Blocking (Enforce)</option>
                            <option value="shadow">👁 Shadow (Log only)</option>
                        </Select>
                    </div>
                </div>

                {/* Templates */}
                {mode === "create" && (
                    <div className="space-y-2 pt-2 border-t border-white/5">
                        <Label className="text-xs text-zinc-500 flex items-center gap-2"><Zap className="h-3 w-3 text-amber-500" /> Start from a Template</Label>
                        <div className="flex flex-wrap gap-2">
                            <Button
                                type="button" variant="outline" size="sm"
                                className="text-xs h-7 px-3 bg-black border-white/10 text-zinc-400 hover:text-white hover:bg-white/5"
                                onClick={() => {
                                    setInputMode("json");
                                    setJsonRules(JSON.stringify([{
                                        phase: "post",
                                        condition: "Always",
                                        action: { Log: { level: "info", tags: ["shadow-test"] } }
                                    }], null, 2));
                                    setPolicyMode("shadow");
                                    if (!name) setName("Shadow Logger");
                                    toast.success("Applied Shadow Logger template!");
                                }}
                            >
                                👁 Shadow Logger
                            </Button>

                            <Button
                                type="button" variant="outline" size="sm"
                                className="text-xs h-7 px-3 bg-black border-white/10 text-zinc-400 hover:text-white hover:bg-white/5"
                                onClick={() => {
                                    setInputMode("json");
                                    setJsonRules(JSON.stringify([{
                                        phase: "pre",
                                        condition: "Always",
                                        action: {
                                            Split: {
                                                experiment: "model-ab-test",
                                                variants: [
                                                    { weight: 50, name: "gpt-4o", set_body_fields: { model: "gpt-4o" } },
                                                    { weight: 50, name: "claude-3-5", set_body_fields: { model: "claude-3-5-sonnet-20241022" } }
                                                ]
                                            }
                                        }
                                    }], null, 2));
                                    setPolicyMode("enforce");
                                    if (!name) setName("A/B Model Split");
                                    toast.success("Applied A/B Split template! (Make sure to switch to JSON mode)");
                                }}
                            >
                                ⚖️ A/B Model Split
                            </Button>

                            <Button
                                type="button" variant="outline" size="sm"
                                className="text-xs h-7 px-3 bg-black border-white/10 text-zinc-400 hover:text-white hover:bg-white/5"
                                onClick={() => {
                                    setInputMode("visual");
                                    setRules([emptyRule()]);
                                    setRules(prev => {
                                        const r = { ...prev[0] };
                                        r.conditionMode = "check";
                                        r.field = "provider";
                                        r.operator = "In";
                                        r.value = "openai, anthropic";
                                        r.actionType = "Deny";
                                        r.phase = "pre";
                                        return [r];
                                    });
                                    setPolicyMode("enforce");
                                    if (!name) setName("Restrict Providers");
                                    toast.success("Applied Provider Restriction template!");
                                }}
                            >
                                🚫 Restrict Providers
                            </Button>
                        </div>
                    </div>
                )}

                {/* Input Mode Toggle */}
                <div className="flex items-center gap-1 border border-white/10 rounded-md p-0.5 w-fit">
                    <button
                        type="button"
                        className={cn(
                            "px-3 py-2 rounded-md text-xs font-medium transition-all",
                            inputMode === "visual" ? "bg-white/10 text-white shadow-sm" : "text-zinc-500 hover:text-zinc-300"
                        )}
                        onClick={() => setInputMode("visual")}
                    >
                        <Layers className="h-3 w-3 inline mr-1.5" /> Visual Builder
                    </button>
                    <button
                        type="button"
                        className={cn(
                            "px-3 py-2 rounded-md text-xs font-medium transition-all",
                            inputMode === "json" ? "bg-white/10 text-white shadow-sm" : "text-zinc-500 hover:text-zinc-300"
                        )}
                        onClick={() => setInputMode("json")}
                    >
                        <Code2 className="h-3 w-3 inline mr-1.5" /> JSON
                    </button>
                </div>

                {inputMode === "visual" ? (
                    <div className="space-y-3">
                        {rules.map((rule, idx) => (
                            <VisualRuleEditor
                                key={idx}
                                rule={rule}
                                index={idx}
                                total={rules.length}
                                onUpdate={(updates) => updateRule(idx, updates)}
                                onRemove={() => removeRule(idx)}
                            />
                        ))}
                        <Button type="button" variant="outline" size="sm" className="w-full bg-black border-dashed border-white/20 text-zinc-400 hover:text-white hover:bg-white/5 hover:border-white/40" onClick={addRule}>
                            <Plus className="h-3 w-3 mr-1.5" /> Add Rule
                        </Button>
                    </div>
                ) : (
                    <div className="space-y-1.5">
                        <Label className="text-xs text-zinc-400">Rules JSON</Label>
                        <textarea
                            className="flex min-h-[200px] w-full rounded-md border border-white/10 bg-black px-3 py-2 text-xs font-mono text-zinc-300 ring-offset-background focus:outline-none focus:ring-1 focus:ring-white/20"
                            value={jsonRules}
                            onChange={(e) => setJsonRules(e.target.value)}
                        />
                    </div>
                )}
            </div>

            <DialogFooter className="border-t border-white/10 pt-4 mt-2">
                <DialogClose asChild>
                    <Button type="button" variant="ghost" size="sm" className="text-zinc-400 hover:text-white hover:bg-white/5">Cancel</Button>
                </DialogClose>
                <Button type="submit" size="sm" disabled={saving} className="bg-white text-black hover:bg-zinc-200">
                    {saving ? "Saving..." : mode === "edit" ? "Update Policy" : "Create Policy"}
                </Button>
            </DialogFooter>
        </form>
    );
}

// ── Visual Rule Editor ────────────────────────────

function VisualRuleEditor({ rule, index, total, onUpdate, onRemove }: {
    rule: RuleForm;
    index: number;
    total: number;
    onUpdate: (updates: Partial<RuleForm>) => void;
    onRemove: () => void;
}) {
    const at = ACTION_TYPES.find(a => a.value === rule.actionType);

    return (
        <div className="rounded-md border border-white/10 bg-white/[0.02] p-4 space-y-4">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <span className="text-[10px] font-mono text-zinc-500 bg-white/5 border border-white/10 px-1.5 py-0.5 rounded">
                        Rule #{index + 1}
                    </span>
                    <Select value={rule.phase} onChange={(e) => onUpdate({ phase: e.target.value })} className="h-7 w-20 text-[11px] bg-black">
                        <option value="pre">Pre</option>
                        <option value="post">Post</option>
                    </Select>
                </div>
                {total > 1 && (
                    <Button type="button" variant="ghost" size="icon" className="h-6 w-6 text-zinc-500 hover:text-rose-400 hover:bg-rose-500/10" onClick={onRemove}>
                        <X className="h-3 w-3" />
                    </Button>
                )}
            </div>

            {/* Condition */}
            <div className="space-y-2">
                <div className="flex items-center gap-2">
                    <span className="text-[10px] uppercase tracking-widest font-semibold text-amber-500/80 w-8">IF</span>
                    <Select value={rule.conditionMode} onChange={(e) => onUpdate({ conditionMode: e.target.value as "always" | "check" })} className="h-7 w-32 text-[11px] bg-black">
                        <option value="always">Always</option>
                        <option value="check">Condition...</option>
                    </Select>
                </div>
                {rule.conditionMode === "check" && (
                    <div className="ml-10 grid grid-cols-3 gap-2">
                        <div className="space-y-1">
                            <Input
                                value={rule.field}
                                onChange={(e) => onUpdate({ field: e.target.value })}
                                placeholder="request.path"
                                className="h-7 text-[11px] font-mono bg-black"
                                list={`fields-${index}`}
                            />
                            <datalist id={`fields-${index}`}>
                                {FIELD_SUGGESTIONS.flatMap(g => g.fields.map(f => (
                                    <option key={f} value={f} />
                                )))}
                            </datalist>
                        </div>
                        <Select value={rule.operator} onChange={(e) => onUpdate({ operator: e.target.value })} className="h-7 text-[11px] bg-black">
                            {OPERATORS.map(op => (
                                <option key={op.value} value={op.value}>{op.label}</option>
                            ))}
                        </Select>
                        <Input
                            value={rule.value}
                            onChange={(e) => onUpdate({ value: e.target.value })}
                            placeholder="value"
                            className="h-7 text-[11px] font-mono bg-black"
                        />
                    </div>
                )}
            </div>

            {/* Action */}
            <div className="space-y-2">
                <div className="flex items-center gap-2">
                    <span className="text-[10px] uppercase tracking-widest font-semibold text-emerald-500/80 w-8">DO</span>
                    <Select value={rule.actionType} onChange={(e) => onUpdate({ actionType: e.target.value })} className="h-7 text-[11px] bg-black">
                        {ACTION_TYPES.map(a => (
                            <option key={a.value} value={a.value}>{a.label} — {a.desc}</option>
                        ))}
                    </Select>
                </div>

                {/* Action-specific fields */}
                <div className="ml-10 space-y-2">
                    {rule.actionType === "Deny" && (
                        <div className="grid grid-cols-4 gap-2">
                            <Input
                                type="number"
                                value={rule.denyStatus}
                                onChange={(e) => onUpdate({ denyStatus: parseInt(e.target.value) })}
                                className="h-7 text-[11px] font-mono bg-black"
                                placeholder="403"
                            />
                            <Input
                                value={rule.denyMessage}
                                onChange={(e) => onUpdate({ denyMessage: e.target.value })}
                                className="h-7 text-[11px] col-span-3 bg-black"
                                placeholder="Denial message"
                            />
                        </div>
                    )}
                    {rule.actionType === "RateLimit" && (
                        <div className="grid grid-cols-3 gap-2">
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Max requests</Label>
                                <Input type="number" value={rule.rateMax} onChange={(e) => onUpdate({ rateMax: parseInt(e.target.value) })} className="h-7 text-[11px] font-mono bg-black" />
                            </div>
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Window</Label>
                                <Input value={rule.rateWindow} onChange={(e) => onUpdate({ rateWindow: e.target.value })} className="h-7 text-[11px] font-mono bg-black" placeholder="60s" />
                            </div>
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Key</Label>
                                <Select value={rule.rateKey} onChange={(e) => onUpdate({ rateKey: e.target.value })} className="h-7 text-[11px] bg-black">
                                    <option value="token">Per Token</option>
                                    <option value="ip">Per IP</option>
                                    <option value="agent">Per Agent</option>
                                    <option value="global">Global</option>
                                </Select>
                            </div>
                        </div>
                    )}
                    {rule.actionType === "Redact" && (
                        <div className="grid grid-cols-3 gap-2">
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Direction</Label>
                                <Select value={rule.redactDirection} onChange={(e) => onUpdate({ redactDirection: e.target.value })} className="h-7 text-[11px] bg-black">
                                    <option value="Request">Request</option>
                                    <option value="Response">Response</option>
                                    <option value="Both">Both</option>
                                </Select>
                            </div>
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Patterns</Label>
                                <Input value={rule.redactPatterns} onChange={(e) => onUpdate({ redactPatterns: e.target.value })} className="h-7 text-[11px] font-mono bg-black" placeholder="email,ssn,phone" />
                            </div>
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Fields</Label>
                                <Input value={rule.redactFields} onChange={(e) => onUpdate({ redactFields: e.target.value })} className="h-7 text-[11px] font-mono bg-black" placeholder="password,secret" />
                            </div>
                        </div>
                    )}
                    {rule.actionType === "RequireApproval" && (
                        <div className="space-y-0.5 w-32">
                            <Label className="text-[10px] text-zinc-500">Timeout (sec)</Label>
                            <Input type="number" value={rule.hitlTimeout} onChange={(e) => onUpdate({ hitlTimeout: parseInt(e.target.value) })} className="h-7 text-[11px] font-mono bg-black" />
                        </div>
                    )}
                    {rule.actionType === "Throttle" && (
                        <div className="space-y-0.5 w-32">
                            <Label className="text-[10px] text-zinc-500">Delay (ms)</Label>
                            <Input type="number" value={rule.throttleMs} onChange={(e) => onUpdate({ throttleMs: parseInt(e.target.value) })} className="h-7 text-[11px] font-mono bg-black" />
                        </div>
                    )}
                    {rule.actionType === "Log" && (
                        <div className="grid grid-cols-2 gap-2">
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Level</Label>
                                <Select value={rule.logLevel} onChange={(e) => onUpdate({ logLevel: e.target.value })} className="h-7 text-[11px] bg-black">
                                    <option value="info">Info</option>
                                    <option value="warn">Warn</option>
                                    <option value="error">Error</option>
                                </Select>
                            </div>
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Tags</Label>
                                <Input value={rule.logTags} onChange={(e) => onUpdate({ logTags: e.target.value })} className="h-7 text-[11px] font-mono bg-black" placeholder="compliance,audit" />
                            </div>
                        </div>
                    )}
                    {rule.actionType === "Tag" && (
                        <div className="grid grid-cols-2 gap-2">
                            <Input value={rule.tagKey} onChange={(e) => onUpdate({ tagKey: e.target.value })} className="h-7 text-[11px] font-mono bg-black" placeholder="key" />
                            <Input value={rule.tagValue} onChange={(e) => onUpdate({ tagValue: e.target.value })} className="h-7 text-[11px] font-mono bg-black" placeholder="value" />
                        </div>
                    )}
                    {rule.actionType === "Transform" && (
                        <div className="space-y-0.5">
                            <Label className="text-[10px] text-zinc-500">Operations JSON</Label>
                            <textarea
                                className="flex min-h-[60px] w-full rounded-md border border-white/10 bg-black px-2 py-1 text-[11px] font-mono text-zinc-300 focus:outline-none focus:ring-1 focus:ring-white/20"
                                value={rule.transformOps}
                                onChange={(e) => onUpdate({ transformOps: e.target.value })}
                                placeholder={'[{"AppendSystemPrompt": {"text": "Be helpful"}}]'}
                            />
                        </div>
                    )}
                    {rule.actionType === "ToolScope" && (
                        <div className="space-y-2">
                            <div className="grid grid-cols-2 gap-2">
                                <div className="space-y-0.5">
                                    <Label className="text-[10px] text-zinc-500">
                                        Blocked Tools <span className="text-rose-400">(denylist)</span>
                                    </Label>
                                    <Input
                                        value={rule.toolScopeBlocked}
                                        onChange={(e) => onUpdate({ toolScopeBlocked: e.target.value })}
                                        className="h-7 text-[11px] font-mono bg-black"
                                        placeholder="stripe.*, db.drop*"
                                    />
                                </div>
                                <div className="space-y-0.5">
                                    <Label className="text-[10px] text-zinc-500">
                                        Allowed Tools <span className="text-emerald-400">(allowlist, empty = all)</span>
                                    </Label>
                                    <Input
                                        value={rule.toolScopeAllowed}
                                        onChange={(e) => onUpdate({ toolScopeAllowed: e.target.value })}
                                        className="h-7 text-[11px] font-mono bg-black"
                                        placeholder="jira.*, github.read"
                                    />
                                </div>
                            </div>
                            <div className="space-y-0.5">
                                <Label className="text-[10px] text-zinc-500">Deny Message</Label>
                                <Input
                                    value={rule.toolScopeDenyMessage}
                                    onChange={(e) => onUpdate({ toolScopeDenyMessage: e.target.value })}
                                    className="h-7 text-[11px] bg-black"
                                    placeholder="Tool not permitted by policy"
                                />
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
