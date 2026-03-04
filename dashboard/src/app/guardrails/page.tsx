"use client";

import { useState, useCallback, useMemo, useEffect } from "react";
import useSWR, { mutate as globalMutate } from "swr";
import {
    getGuardrailPresets,
    getGuardrailStatus,
    listTokens,
    enableGuardrails,
    disableGuardrails,
    type GuardrailPresetsResponse,
    type GuardrailsStatus,
    type Token,
} from "@/lib/api";
import {
    Shield,
    ShieldCheck,
    ShieldAlert,
    Lock,
    Eye,
    Code,
    FileText,
    AlertTriangle,
    CheckCircle,
    Layers,
    RefreshCw,
    Zap,
    Activity,
    ChevronDown,
    Power,
    PowerOff,
    Loader2,
    Info,
    Search,
    X,
    Plus,
    Minus,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { toast } from "sonner";

// ── Constants ──────────────────────────────────────────────────

const categoryColors: Record<string, string> = {
    privacy: "bg-violet-500/10 text-violet-500 border-violet-500/30",
    safety: "bg-rose-500/10 text-rose-500 border-rose-500/30",
    compliance: "bg-blue-500/10 text-blue-500 border-blue-500/30",
};

const presetIcons: Record<string, React.ComponentType<{ className?: string }>> = {
    pii_redaction: Eye,
    pii_enterprise: Shield,
    pii_block: ShieldAlert,
    prompt_injection: ShieldCheck,
    code_injection: Code,
    hipaa: Lock,
    pci: Lock,
    topic_fence: Layers,
    length_limit: FileText,
};

// ── KPI Card ───────────────────────────────────────────────────

function KPICard({
    icon: Icon,
    label,
    value,
    sub,
    color,
}: {
    icon: React.ComponentType<{ className?: string }>;
    label: string;
    value: string;
    sub?: string;
    color: string;
}) {
    return (
        <Card className="glass-card hover-lift">
            <CardContent className="p-4 flex items-center gap-4">
                <div className={cn("p-2.5 rounded-md flex-shrink-0", color)}>
                    <Icon className="h-5 w-5" />
                </div>
                <div className="min-w-0 flex-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-1">
                        {label}
                    </p>
                    <p className="text-xl font-bold tabular-nums tracking-tight">{value}</p>
                    {sub && <p className="text-[10px] text-muted-foreground truncate">{sub}</p>}
                </div>
            </CardContent>
        </Card>
    );
}

// ── Interactive Preset Toggle Card ─────────────────────────────

function PresetToggleCard({
    name,
    description,
    category,
    patterns,
    required_fields,
    isSelected,
    onToggle,
    disabled,
}: {
    name: string;
    description: string;
    category: string;
    patterns?: string[];
    required_fields?: string[];
    isSelected: boolean;
    onToggle: () => void;
    disabled: boolean;
}) {
    const Icon = presetIcons[name] || Shield;
    const colorCls = categoryColors[category] || "bg-muted text-muted-foreground border-border";
    const [expanded, setExpanded] = useState(false);

    return (
        <Card
            className={cn(
                "transition-all cursor-pointer group border-2",
                isSelected
                    ? "border-emerald-500/50 bg-emerald-500/5 shadow-sm shadow-emerald-500/10"
                    : "border-transparent hover:border-border glass-card hover-lift",
                disabled && "opacity-50 pointer-events-none"
            )}
            onClick={onToggle}
        >
            <CardContent className="p-4 space-y-2.5">
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3 min-w-0 flex-1">
                        <div className={cn("p-2 rounded-md flex-shrink-0", colorCls.split(" ").slice(0, 2).join(" "))}>
                            <Icon className="h-4 w-4" />
                        </div>
                        <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                                <h3 className="text-sm font-semibold font-mono truncate">{name}</h3>
                                <Badge
                                    variant="outline"
                                    className={cn("text-[8px] uppercase tracking-wider flex-shrink-0", colorCls)}
                                >
                                    {category}
                                </Badge>
                            </div>
                            <p className="text-[11px] text-muted-foreground mt-0.5 line-clamp-1">{description}</p>
                        </div>
                    </div>
                    {/* Toggle */}
                    <div
                        className={cn(
                            "flex-shrink-0 ml-3 w-11 h-6 rounded-full relative transition-colors",
                            isSelected ? "bg-emerald-500" : "bg-muted-foreground/20"
                        )}
                    >
                        <div
                            className={cn(
                                "absolute top-[3px] w-[18px] h-[18px] rounded-full bg-white transition-transform shadow-sm",
                                isSelected ? "translate-x-[22px]" : "translate-x-[3px]"
                            )}
                        />
                    </div>
                </div>

                {/* Expandable details */}
                {(patterns || required_fields) && (
                    <>
                        <button
                            onClick={(e) => { e.stopPropagation(); setExpanded(!expanded); }}
                            className="text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-1 transition-colors"
                        >
                            <ChevronDown className={cn("h-3 w-3 transition-transform", expanded && "rotate-180")} />
                            {expanded ? "Hide" : "Show"} details
                        </button>

                        {expanded && (
                            <div className="space-y-2 animate-fade-in" onClick={(e) => e.stopPropagation()}>
                                {patterns && patterns.length > 0 && (
                                    <div>
                                        <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1 flex items-center gap-1">
                                            <CheckCircle className="h-3 w-3 text-emerald-500" /> Patterns
                                        </p>
                                        <div className="flex flex-wrap gap-1">
                                            {patterns.map(p => (
                                                <Badge key={p} variant="secondary" className="font-mono text-[9px]">
                                                    {p}
                                                </Badge>
                                            ))}
                                        </div>
                                    </div>
                                )}
                                {required_fields && required_fields.length > 0 && (
                                    <div className="rounded-md border border-amber-500/20 bg-amber-500/5 p-2 flex gap-2 text-[10px] text-amber-700 dark:text-amber-400">
                                        <AlertTriangle className="h-3 w-3 flex-shrink-0 mt-0.5" />
                                        <span>Requires: <code className="font-mono">{required_fields.join(", ")}</code></span>
                                    </div>
                                )}
                            </div>
                        )}
                    </>
                )}
            </CardContent>
        </Card>
    );
}

// ── Token Selector ──────────────────────────────────────────────

function TokenSelector({
    tokens,
    selectedId,
    onSelect,
}: {
    tokens: Token[];
    selectedId: string | null;
    onSelect: (id: string) => void;
}) {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");
    const selected = tokens.find(t => t.id === selectedId);

    const filtered = tokens.filter(t =>
        t.name.toLowerCase().includes(search.toLowerCase()) ||
        t.id.toLowerCase().includes(search.toLowerCase())
    );

    return (
        <div className="relative">
            <button
                onClick={() => setOpen(!open)}
                className={cn(
                    "w-full flex items-center justify-between gap-2 rounded-md border px-3 py-3 text-sm transition-colors",
                    "hover:border-foreground/30 focus:outline-none focus:ring-2 focus:ring-emerald-500/20",
                    open ? "border-emerald-500/50" : "border-border",
                    !selected && "text-muted-foreground"
                )}
            >
                <div className="flex items-center gap-2 min-w-0 flex-1">
                    <div className={cn(
                        "w-2 h-2 rounded-full flex-shrink-0",
                        selected?.is_active ? "bg-emerald-500" : selected ? "bg-amber-500" : "bg-muted-foreground/30"
                    )} />
                    <span className="truncate font-medium">
                        {selected ? selected.name : "Select an API token..."}
                    </span>
                    {selected && (
                        <span className="text-[10px] text-muted-foreground font-mono flex-shrink-0">
                            {selected.id.slice(0, 12)}...
                        </span>
                    )}
                </div>
                <ChevronDown className={cn("h-4 w-4 text-muted-foreground transition-transform flex-shrink-0", open && "rotate-180")} />
            </button>

            {open && (
                <div className="absolute z-50 mt-1 w-full rounded-md border border-border bg-popover shadow-lg animate-fade-in">
                    <div className="p-2 border-b border-border">
                        <div className="flex items-center gap-2 rounded-md border border-border px-2 py-2">
                            <Search className="h-3.5 w-3.5 text-muted-foreground" />
                            <input
                                type="text"
                                value={search}
                                onChange={e => setSearch(e.target.value)}
                                placeholder="Search tokens..."
                                className="bg-transparent text-sm outline-none w-full placeholder:text-muted-foreground/50"
                                autoFocus
                            />
                            {search && (
                                <button onClick={() => setSearch("")} className="text-muted-foreground hover:text-foreground">
                                    <X className="h-3 w-3" />
                                </button>
                            )}
                        </div>
                    </div>
                    <div className="max-h-48 overflow-y-auto p-1">
                        {filtered.length === 0 && (
                            <p className="text-xs text-muted-foreground text-center py-3">No tokens found</p>
                        )}
                        {filtered.map(t => (
                            <button
                                key={t.id}
                                className={cn(
                                    "w-full text-left px-3 py-2 rounded-md text-sm flex items-center gap-2 transition-colors",
                                    t.id === selectedId
                                        ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-400"
                                        : "hover:bg-muted"
                                )}
                                onClick={() => { onSelect(t.id); setOpen(false); setSearch(""); }}
                            >
                                <div className={cn(
                                    "w-2 h-2 rounded-full flex-shrink-0",
                                    t.is_active ? "bg-emerald-500" : "bg-amber-500"
                                )} />
                                <span className="truncate font-medium">{t.name}</span>
                                <span className="text-[10px] text-muted-foreground font-mono ml-auto flex-shrink-0">
                                    {t.policy_ids?.length || 0} policies
                                </span>
                            </button>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}

// ── Main Page ──────────────────────────────────────────────────

export default function GuardrailsPage() {
    const { data: presetsData, isLoading: presetsLoading } = useSWR<GuardrailPresetsResponse>(
        "/guardrails/presets",
        () => getGuardrailPresets()
    );

    const { data: tokensData, isLoading: tokensLoading } = useSWR<Token[]>(
        "/tokens",
        () => listTokens()
    );

    const [selectedTokenId, setSelectedTokenId] = useState<string | null>(null);
    const [selectedPresets, setSelectedPresets] = useState<Set<string>>(new Set());
    const [applying, setApplying] = useState(false);
    const [removing, setRemoving] = useState(false);
    const [tokenStatus, setTokenStatus] = useState<GuardrailsStatus | null>(null);
    const [statusLoading, setStatusLoading] = useState(false);

    const presets = presetsData?.presets ?? [];
    const tokens = tokensData ?? [];
    const totalPatterns = presets.reduce((acc, p) => acc + (p.patterns?.length ?? 0), 0);

    // Fetch guardrails status when token changes
    useEffect(() => {
        if (!selectedTokenId) {
            setTokenStatus(null);
            return;
        }
        setStatusLoading(true);
        getGuardrailStatus(selectedTokenId)
            .then(s => setTokenStatus(s))
            .catch(() => setTokenStatus(null))
            .finally(() => setStatusLoading(false));
    }, [selectedTokenId]);

    const togglePreset = useCallback((name: string) => {
        setSelectedPresets(prev => {
            const next = new Set(prev);
            next.has(name) ? next.delete(name) : next.add(name);
            return next;
        });
    }, []);

    const handleApply = useCallback(async () => {
        if (!selectedTokenId || selectedPresets.size === 0) return;
        setApplying(true);
        try {
            const result = await enableGuardrails(selectedTokenId, Array.from(selectedPresets), "dashboard");
            toast.success(`Guardrails enabled`, {
                description: `Applied ${result.applied_presets.length} preset(s) to token.${result.previous_source && result.previous_source !== "dashboard" ? ` (overrode ${result.previous_source} config)` : ""} ${result.skipped?.length ? `Skipped: ${result.skipped.join(", ")}` : ""}`,
            });
            // Refresh tokens to show updated policy count
            await globalMutate("/tokens");
            // Refresh status
            getGuardrailStatus(selectedTokenId).then(s => setTokenStatus(s)).catch(() => { });
        } catch (e) {
            toast.error("Failed to enable guardrails", {
                description: e instanceof Error ? e.message : "Unknown error",
            });
        } finally {
            setApplying(false);
        }
    }, [selectedTokenId, selectedPresets]);

    const handleRemove = useCallback(async () => {
        if (!selectedTokenId) return;
        setRemoving(true);
        try {
            const result = await disableGuardrails(selectedTokenId);
            toast.success(`Guardrails removed`, {
                description: `Removed ${result.removed} guardrail policy/policies from token.`,
            });
            setSelectedPresets(new Set());
            await globalMutate("/tokens");
            setTokenStatus(null);
        } catch (e) {
            toast.error("Failed to remove guardrails", {
                description: e instanceof Error ? e.message : "Unknown error",
            });
        } finally {
            setRemoving(false);
        }
    }, [selectedTokenId]);

    const selectedToken = tokens.find(t => t.id === selectedTokenId);
    const hasGuardrails = (selectedToken?.policy_ids?.length ?? 0) > 0;

    // Group presets by category
    const grouped = useMemo(() => ({
        safety: presets.filter(p => p.category === "safety"),
        privacy: presets.filter(p => p.category === "privacy"),
        compliance: presets.filter(p => p.category === "compliance"),
    }), [presets]);

    const isLoading = presetsLoading || tokensLoading;

    return (
        <div className="space-y-6 pb-10 animate-fade-in">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-lg font-semibold flex items-center gap-2">
                        <ShieldCheck className="h-5 w-5 text-emerald-500" />
                        Guardrails
                    </h1>
                    <p className="text-xs text-muted-foreground mt-0.5">
                        Configure safety, privacy, and compliance guardrails per API token
                    </p>
                </div>
            </div>

            {/* KPI Strip */}
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 animate-slide-up">
                <KPICard icon={Shield} label="Available Presets" value={presets.length.toLocaleString()} sub="One-click guardrail configurations" color="bg-blue-500/10 text-blue-500" />
                <KPICard icon={Eye} label="PII Patterns" value={totalPatterns.toLocaleString()} sub="Across all privacy presets" color="bg-violet-500/10 text-violet-500" />
                <KPICard icon={ShieldAlert} label="Detection Rules" value="35+" sub="Jailbreak + code injection regex" color="bg-rose-500/10 text-rose-500" />
                <KPICard icon={Zap} label="Latency Impact" value="<1ms" sub="Compiled regex — zero LLM overhead" color="bg-emerald-500/10 text-emerald-500" />
            </div>

            {/* ── Configuration Panel ───────────────────────────────── */}
            <Card className="glass-card animate-slide-up stagger-2">
                <CardHeader className="pb-3">
                    <div className="flex items-center gap-2">
                        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-emerald-500/10">
                            <Activity className="h-4 w-4 text-emerald-500" />
                        </div>
                        <div>
                            <CardTitle className="text-sm">Configure Guardrails</CardTitle>
                            <CardDescription className="text-xs">
                                Select a token, pick your guardrails, and apply them in one click
                            </CardDescription>
                        </div>
                    </div>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Step 1: Token Selector */}
                    <div className="space-y-2">
                        <div className="flex items-center gap-2">
                            <div className="flex h-5 w-5 items-center justify-center rounded-full bg-emerald-500 text-white text-[10px] font-bold">1</div>
                            <p className="text-xs font-semibold">Select API Token</p>
                        </div>
                        {tokensLoading ? (
                            <div className="h-10 bg-muted/40 rounded-md shimmer" />
                        ) : (
                            <TokenSelector
                                tokens={tokens}
                                selectedId={selectedTokenId}
                                onSelect={setSelectedTokenId}
                            />
                        )}
                        {/* Current guardrails status */}
                        {statusLoading && (
                            <div className="h-10 bg-muted/40 rounded-md shimmer" />
                        )}
                        {tokenStatus?.has_guardrails && !statusLoading && (
                            <div className="rounded-md border border-emerald-500/20 bg-emerald-500/5 px-3 py-2 space-y-1.5">
                                <div className="flex items-center gap-2 text-[11px] text-emerald-700 dark:text-emerald-400">
                                    <ShieldCheck className="h-3.5 w-3.5 flex-shrink-0" />
                                    <span>
                                        Active guardrails: <strong>{tokenStatus.presets.join(", ") || "custom config"}</strong>
                                    </span>
                                    <Badge variant="outline" className="ml-auto text-[9px] font-mono">
                                        via {tokenStatus.source || "unknown"}
                                    </Badge>
                                </div>
                            </div>
                        )}
                        {/* Drift warning: guardrails set via SDK */}
                        {tokenStatus?.has_guardrails && tokenStatus.source === "sdk" && !statusLoading && (
                            <div className="rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-3 flex gap-2 text-[11px] text-amber-700 dark:text-amber-400">
                                <AlertTriangle className="h-4 w-4 flex-shrink-0 mt-0.5" />
                                <div>
                                    <p className="font-semibold">Drift warning</p>
                                    <p className="mt-0.5 leading-relaxed">
                                        These guardrails were configured via <strong>SDK code</strong>. Changing them here
                                        may cause drift if the SDK re-applies its config on next deploy.
                                        Consider updating the SDK code instead, or coordinate with your team.
                                    </p>
                                </div>
                            </div>
                        )}
                        {selectedToken && !tokenStatus?.has_guardrails && !statusLoading && (
                            <div className="rounded-md border border-border/50 bg-muted/10 px-3 py-2 flex items-center gap-2 text-[11px] text-muted-foreground">
                                <Info className="h-3.5 w-3.5 flex-shrink-0" />
                                <span>No guardrails configured on this token yet.</span>
                            </div>
                        )}
                    </div>

                    {/* Step 2: Pick Presets */}
                    <div className="space-y-3">
                        <div className="flex items-center gap-2">
                            <div className={cn(
                                "flex h-5 w-5 items-center justify-center rounded-full text-[10px] font-bold",
                                selectedTokenId ? "bg-emerald-500 text-white" : "bg-muted text-muted-foreground"
                            )}>2</div>
                            <p className="text-xs font-semibold">Pick Guardrails</p>
                            {selectedPresets.size > 0 && (
                                <Badge className="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30 text-[10px]">
                                    {selectedPresets.size} selected
                                </Badge>
                            )}
                            {selectedPresets.size > 0 && (
                                <button
                                    onClick={() => setSelectedPresets(new Set())}
                                    className="ml-auto text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-1"
                                >
                                    <X className="h-3 w-3" /> Clear all
                                </button>
                            )}
                        </div>

                        {!selectedTokenId && (
                            <div className="rounded-md border border-dashed border-border p-4 text-center">
                                <Info className="h-5 w-5 text-muted-foreground mx-auto mb-2" />
                                <p className="text-xs text-muted-foreground">Select a token above to configure guardrails</p>
                            </div>
                        )}

                        {selectedTokenId && (
                            <div className="space-y-4">
                                {/* Quick Actions */}
                                <div className="flex gap-2 flex-wrap">
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="text-[11px] h-7"
                                        onClick={() => setSelectedPresets(new Set(["prompt_injection", "pii_redaction"]))}
                                    >
                                        <Zap className="h-3 w-3 mr-1" /> Quick: Standard
                                    </Button>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="text-[11px] h-7"
                                        onClick={() => setSelectedPresets(new Set(["prompt_injection", "code_injection", "pii_enterprise"]))}
                                    >
                                        <Shield className="h-3 w-3 mr-1" /> Quick: Enterprise
                                    </Button>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="text-[11px] h-7"
                                        onClick={() => setSelectedPresets(new Set(["hipaa", "pii_enterprise", "prompt_injection"]))}
                                    >
                                        <Lock className="h-3 w-3 mr-1" /> Quick: Healthcare
                                    </Button>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="text-[11px] h-7"
                                        onClick={() => setSelectedPresets(new Set(presets.map(p => p.name).filter(n => n !== "topic_fence")))}
                                    >
                                        <Plus className="h-3 w-3 mr-1" /> Select All
                                    </Button>
                                    {selectedPresets.size > 0 && (
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            className="text-[11px] h-7 text-muted-foreground"
                                            onClick={() => setSelectedPresets(new Set())}
                                        >
                                            <Minus className="h-3 w-3 mr-1" /> Deselect All
                                        </Button>
                                    )}
                                </div>

                                {/* Safety */}
                                <div className="space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="flex h-5 w-5 items-center justify-center rounded-md bg-rose-500/10">
                                            <ShieldAlert className="h-3 w-3 text-rose-500" />
                                        </div>
                                        <span className="text-xs font-semibold">Safety</span>
                                    </div>
                                    <div className="grid gap-2 sm:grid-cols-2">
                                        {(presetsLoading ? Array.from({ length: 3 }).map((_, i) => (
                                            <div key={i} className="h-20 bg-muted/40 rounded-md shimmer" />
                                        )) : grouped.safety.map(p => (
                                            <PresetToggleCard
                                                key={p.name}
                                                {...p}
                                                isSelected={selectedPresets.has(p.name)}
                                                onToggle={() => togglePreset(p.name)}
                                                disabled={!selectedTokenId}
                                            />
                                        )))}
                                    </div>
                                </div>

                                {/* Privacy */}
                                <div className="space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="flex h-5 w-5 items-center justify-center rounded-md bg-violet-500/10">
                                            <Eye className="h-3 w-3 text-violet-500" />
                                        </div>
                                        <span className="text-xs font-semibold">Privacy &amp; PII</span>
                                    </div>
                                    <div className="grid gap-2 sm:grid-cols-2">
                                        {(presetsLoading ? Array.from({ length: 3 }).map((_, i) => (
                                            <div key={i} className="h-20 bg-muted/40 rounded-md shimmer" />
                                        )) : grouped.privacy.map(p => (
                                            <PresetToggleCard
                                                key={p.name}
                                                {...p}
                                                isSelected={selectedPresets.has(p.name)}
                                                onToggle={() => togglePreset(p.name)}
                                                disabled={!selectedTokenId}
                                            />
                                        )))}
                                    </div>
                                </div>

                                {/* Compliance */}
                                <div className="space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="flex h-5 w-5 items-center justify-center rounded-md bg-blue-500/10">
                                            <Lock className="h-3 w-3 text-blue-500" />
                                        </div>
                                        <span className="text-xs font-semibold">Compliance</span>
                                    </div>
                                    <div className="grid gap-2 sm:grid-cols-2">
                                        {(presetsLoading ? Array.from({ length: 2 }).map((_, i) => (
                                            <div key={i} className="h-20 bg-muted/40 rounded-md shimmer" />
                                        )) : grouped.compliance.map(p => (
                                            <PresetToggleCard
                                                key={p.name}
                                                {...p}
                                                isSelected={selectedPresets.has(p.name)}
                                                onToggle={() => togglePreset(p.name)}
                                                disabled={!selectedTokenId}
                                            />
                                        )))}
                                    </div>
                                </div>
                            </div>
                        )}
                    </div>

                    {/* Step 3: Apply */}
                    {selectedTokenId && (
                        <div className="space-y-2 pt-2 border-t border-border">
                            <div className="flex items-center gap-2">
                                <div className={cn(
                                    "flex h-5 w-5 items-center justify-center rounded-full text-[10px] font-bold",
                                    selectedPresets.size > 0 ? "bg-emerald-500 text-white" : "bg-muted text-muted-foreground"
                                )}>3</div>
                                <p className="text-xs font-semibold">Apply</p>
                            </div>
                            <div className="flex gap-2 flex-wrap">
                                <Button
                                    onClick={handleApply}
                                    disabled={selectedPresets.size === 0 || applying}
                                    className="bg-emerald-600 hover:bg-emerald-700 text-white"
                                    size="sm"
                                >
                                    {applying ? (
                                        <Loader2 className="h-3.5 w-3.5 mr-1.5 animate-spin" />
                                    ) : (
                                        <Power className="h-3.5 w-3.5 mr-1.5" />
                                    )}
                                    Enable {selectedPresets.size} Guardrail{selectedPresets.size !== 1 ? "s" : ""}
                                </Button>
                                {hasGuardrails && (
                                    <Button
                                        onClick={handleRemove}
                                        disabled={removing}
                                        variant="outline"
                                        size="sm"
                                        className="text-rose-600 border-rose-500/30 hover:bg-rose-500/10"
                                    >
                                        {removing ? (
                                            <Loader2 className="h-3.5 w-3.5 mr-1.5 animate-spin" />
                                        ) : (
                                            <PowerOff className="h-3.5 w-3.5 mr-1.5" />
                                        )}
                                        Remove All Guardrails
                                    </Button>
                                )}
                            </div>
                        </div>
                    )}
                </CardContent>
            </Card>

            {/* How Guardrails Work */}
            <Card className="glass-card animate-slide-up stagger-3">
                <CardHeader className="pb-3">
                    <div className="flex items-center gap-2">
                        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-emerald-500/10">
                            <Activity className="h-4 w-4 text-emerald-500" />
                        </div>
                        <div>
                            <CardTitle className="text-sm">How It Works</CardTitle>
                            <CardDescription className="text-xs">Guardrails execute in the proxy pipeline — zero LLM overhead</CardDescription>
                        </div>
                    </div>
                </CardHeader>
                <CardContent>
                    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                        {[
                            {
                                icon: Layers,
                                color: "text-blue-500 bg-blue-500/10",
                                title: "Policy Engine",
                                desc: "Condition→action rules evaluated per request. Shadow mode for safe testing."
                            },
                            {
                                icon: Shield,
                                color: "text-violet-500 bg-violet-500/10",
                                title: "PII Redaction",
                                desc: "12 built-in patterns (SSN, email, passport, AWS key) with bidirectional scrubbing."
                            },
                            {
                                icon: ShieldAlert,
                                color: "text-rose-500 bg-rose-500/10",
                                title: "Content Safety",
                                desc: "35+ regex patterns detect jailbreaks, code injection, and harmful content."
                            },
                            {
                                icon: Zap,
                                color: "text-emerald-500 bg-emerald-500/10",
                                title: "ReDoS Protected",
                                desc: "Custom regex compiled with 1MB size cap. Zero catastrophic backtracking."
                            },
                        ].map(({ icon: Icon, color, title, desc }) => (
                            <div key={title} className="rounded-md border border-border/40 bg-muted/10 p-4 space-y-2">
                                <div className="flex items-center gap-2">
                                    <div className={cn("flex h-7 w-7 items-center justify-center rounded-md", color)}>
                                        <Icon className="h-3.5 w-3.5" />
                                    </div>
                                    <p className="text-sm font-semibold">{title}</p>
                                </div>
                                <p className="text-xs text-muted-foreground leading-relaxed">{desc}</p>
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>

            {/* SDK / API Quick-Start */}
            <div className="rounded-md border border-emerald-500/20 bg-emerald-500/5 p-4 flex gap-3 items-start animate-slide-up stagger-4">
                <Code className="h-5 w-5 text-emerald-500 flex-shrink-0 mt-0.5" />
                <div className="space-y-1.5 flex-1">
                    <p className="text-sm font-semibold text-emerald-700 dark:text-emerald-400">Also available via API &amp; SDK</p>
                    <p className="text-xs text-muted-foreground">
                        Prefer code? Attach guardrails programmatically:
                    </p>
                    <pre className="text-[11px] font-mono bg-muted/40 rounded-md p-3 overflow-x-auto">
                        {`# Python SDK
client.guardrails.enable(
    token_id="tok_abc123",
    presets=["prompt_injection", "pii_enterprise"]
)

# Or via header (per-request)
headers["X-TrueFlow-Guardrails"] = "prompt_injection,pii_redaction"`}
                    </pre>
                </div>
            </div>
        </div>
    );
}
