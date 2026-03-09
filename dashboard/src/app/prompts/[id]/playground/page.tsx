"use client";

import { useState, useMemo, use } from "react";
import useSWR from "swr";
import { useRouter } from "next/navigation";
import {
    getPrompt,
    Prompt,
    PromptVersion,
    Token,
    swrFetcher,
} from "@/lib/api";
import {
    ArrowLeft,
    Play,
    Loader2,
    Sparkles,
    Clock,
    Zap,
    Hash,
    DollarSign,
    Plus,
    Columns,
    X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { PageSkeleton } from "@/components/page-skeleton";

interface PlaygroundRun {
    id: string;
    model: string;
    messages: Array<{ role: string; content: string }>;
    response: string | null;
    status: number | null;
    latencyMs: number | null;
    tokens: { prompt: number; completion: number } | null;
    loading: boolean;
}

export default function PromptPlaygroundPage({
    params,
}: {
    params: Promise<{ id: string }>;
}) {
    const { id } = use(params);
    const router = useRouter();
    const { data, isLoading } = useSWR<{
        prompt: Prompt;
        versions: PromptVersion[];
    }>(`/prompts/${id}`, swrFetcher);
    const { data: tokens } = useSWR<Token[]>("/tokens", swrFetcher);

    const [selectedTokenId, setSelectedTokenId] = useState("");
    const [selectedVersionNum, setSelectedVersionNum] = useState<string>("");
    const [variables, setVariables] = useState<Record<string, string>>({});
    const [runs, setRuns] = useState<PlaygroundRun[]>([]);
    const [compareMode, setCompareMode] = useState(false);

    const prompt = data?.prompt;
    const versions = data?.versions || [];

    const currentVersion = useMemo(() => {
        if (!selectedVersionNum) return versions[0] || null;
        return versions.find((v) => String(v.version) === selectedVersionNum) || null;
    }, [selectedVersionNum, versions]);

    // Detect variables from current version
    const detectedVars = useMemo(() => {
        if (!currentVersion) return [];
        const vars = new Set<string>();
        const msgs = currentVersion.messages as Array<{ role: string; content: string }>;
        msgs.forEach((m) => {
            const matches = m.content.matchAll(/\{\{(\w+)\}\}/g);
            for (const match of matches) vars.add(match[1]);
        });
        return Array.from(vars);
    }, [currentVersion]);

    // Compile messages with variable substitution
    const compileMessages = (
        msgs: Array<{ role: string; content: string }>
    ): Array<{ role: string; content: string }> => {
        return msgs.map((m) => ({
            role: m.role,
            content: m.content.replace(/\{\{(\w+)\}\}/g, (_, key) => variables[key] || `{{${key}}}`),
        }));
    };

    const runPrompt = async (overrideModel?: string) => {
        if (!currentVersion) {
            toast.error("Select a version first");
            return;
        }
        if (!selectedTokenId) {
            toast.error("Select a token to authenticate");
            return;
        }

        const model = overrideModel || currentVersion.model;
        const compiled = compileMessages(
            currentVersion.messages as Array<{ role: string; content: string }>
        );

        const runId = `run-${Date.now()}`;
        const newRun: PlaygroundRun = {
            id: runId,
            model,
            messages: compiled,
            response: null,
            status: null,
            latencyMs: null,
            tokens: null,
            loading: true,
        };

        setRuns((prev) => [newRun, ...prev]);

        const startTime = performance.now();

        try {
            const token = tokens?.find((t) => t.id === selectedTokenId);
            const res = await fetch("http://localhost:8443/v1/chat/completions", {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    Authorization: `Bearer ${token?.id}`,
                },
                body: JSON.stringify({
                    model,
                    messages: compiled,
                    temperature: currentVersion.temperature ?? 1.0,
                    max_tokens: currentVersion.max_tokens ?? undefined,
                }),
            });

            const elapsed = Math.round(performance.now() - startTime);
            const body = await res.json();
            const responseText =
                body.choices?.[0]?.message?.content || JSON.stringify(body, null, 2);
            const usage = body.usage;

            setRuns((prev) =>
                prev.map((r) =>
                    r.id === runId
                        ? {
                            ...r,
                            response: responseText,
                            status: res.status,
                            latencyMs: elapsed,
                            tokens: usage
                                ? {
                                    prompt: usage.prompt_tokens,
                                    completion: usage.completion_tokens,
                                }
                                : null,
                            loading: false,
                        }
                        : r
                )
            );
        } catch (err: unknown) {
            const error = err as Error;
            const elapsed = Math.round(performance.now() - startTime);
            setRuns((prev) =>
                prev.map((r) =>
                    r.id === runId
                        ? {
                            ...r,
                            response: `Error: ${error.message}`,
                            status: 0,
                            latencyMs: elapsed,
                            loading: false,
                        }
                        : r
                )
            );
        }
    };

    if (isLoading) return <PageSkeleton cards={2} rows={3} />;
    if (!prompt)
        return (
            <div className="p-8 text-center text-muted-foreground">
                Prompt not found
            </div>
        );

    return (
        <div className="p-4 max-w-[1600px] mx-auto space-y-4 h-[calc(100vh-60px)] flex flex-col">
            {/* Header */}
            <div className="flex items-center justify-between shrink-0 animate-fade-in">
                <div className="flex items-center gap-3">
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => router.push(`/prompts/${id}`)}
                    >
                        <ArrowLeft className="h-4 w-4" />
                    </Button>
                    <div>
                        <h1 className="text-lg font-semibold flex items-center gap-2">
                            <Sparkles className="h-4 w-4 text-primary" />
                            Playground — {prompt.name}
                        </h1>
                    </div>
                </div>
                <div className="flex items-center gap-2">
                    <Button
                        variant={compareMode ? "default" : "outline"}
                        size="sm"
                        onClick={() => setCompareMode(!compareMode)}
                    >
                        <Columns className="h-3.5 w-3.5 mr-1.5" />
                        {compareMode ? "Compare On" : "Compare"}
                    </Button>
                </div>
            </div>

            {/* Controls */}
            <div className="grid grid-cols-3 gap-3 shrink-0">
                <div className="space-y-1">
                    <Label className="text-[10px] text-muted-foreground uppercase">
                        Token
                    </Label>
                    <Select value={selectedTokenId} onValueChange={setSelectedTokenId}>
                        <SelectTrigger className="h-8 text-xs">
                            <SelectValue placeholder="Select token..." />
                        </SelectTrigger>
                        <SelectContent>
                            {tokens
                                ?.filter((t) => t.is_active)
                                .map((t) => (
                                    <SelectItem key={t.id} value={t.id} className="text-xs">
                                        {t.name}
                                    </SelectItem>
                                ))}
                        </SelectContent>
                    </Select>
                </div>
                <div className="space-y-1">
                    <Label className="text-[10px] text-muted-foreground uppercase">
                        Version
                    </Label>
                    <Select
                        value={selectedVersionNum || String(versions[0]?.version || "")}
                        onValueChange={setSelectedVersionNum}
                    >
                        <SelectTrigger className="h-8 text-xs">
                            <SelectValue placeholder="Latest" />
                        </SelectTrigger>
                        <SelectContent>
                            {versions.map((v) => (
                                <SelectItem
                                    key={v.version}
                                    value={String(v.version)}
                                    className="text-xs"
                                >
                                    v{v.version} — {v.model}
                                    {(v.labels as string[])?.includes("production")
                                        ? " 🟢"
                                        : ""}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>
                <div className="space-y-1">
                    <Label className="text-[10px] text-muted-foreground uppercase">
                        Model Override
                    </Label>
                    <div className="flex gap-2">
                        <Button
                            size="sm"
                            className="h-8 flex-1"
                            onClick={() => runPrompt()}
                            disabled={!currentVersion}
                        >
                            <Play className="h-3 w-3 mr-1" /> Run
                        </Button>
                        {compareMode && (
                            <Button
                                size="sm"
                                variant="outline"
                                className="h-8"
                                onClick={() => {
                                    runPrompt();
                                    runPrompt("claude-3-5-sonnet-latest");
                                }}
                            >
                                <Columns className="h-3 w-3 mr-1" /> Run Both
                            </Button>
                        )}
                    </div>
                </div>
            </div>

            {/* Variables */}
            {detectedVars.length > 0 && (
                <Card className="border-primary/20 bg-primary/5 shrink-0">
                    <CardContent className="p-3">
                        <p className="text-[10px] font-semibold uppercase text-primary mb-2">
                            Variables ({detectedVars.length})
                        </p>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
                            {detectedVars.map((v) => (
                                <div key={v} className="space-y-0.5">
                                    <Label className="text-[10px] font-mono text-muted-foreground">
                                        {`{{${v}}}`}
                                    </Label>
                                    <Input
                                        value={variables[v] || ""}
                                        onChange={(e) =>
                                            setVariables({ ...variables, [v]: e.target.value })
                                        }
                                        className="h-7 text-xs"
                                        placeholder={v}
                                    />
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Preview of compiled messages */}
            {currentVersion && (
                <Card className="shrink-0 border-border/60">
                    <CardHeader className="py-2 px-4 border-b bg-muted/20">
                        <div className="flex items-center gap-2">
                            <Hash className="h-3.5 w-3.5 text-muted-foreground" />
                            <span className="text-xs font-semibold">
                                Compiled Messages ({(currentVersion.messages as Array<unknown>).length})
                            </span>
                            <Badge variant="secondary" className="text-[9px] font-mono px-1 h-4">
                                {currentVersion.model}
                            </Badge>
                        </div>
                    </CardHeader>
                    <CardContent className="p-3 max-h-[150px] overflow-y-auto">
                        <div className="space-y-1.5">
                            {compileMessages(
                                currentVersion.messages as Array<{
                                    role: string;
                                    content: string;
                                }>
                            ).map((m, i) => (
                                <div key={i} className="flex gap-2 text-xs">
                                    <span
                                        className={cn(
                                            "font-semibold uppercase text-[10px] w-16 shrink-0 pt-0.5",
                                            m.role === "system"
                                                ? "text-violet-400"
                                                : m.role === "assistant"
                                                    ? "text-emerald-400"
                                                    : "text-blue-400"
                                        )}
                                    >
                                        {m.role}
                                    </span>
                                    <span className="text-muted-foreground font-mono text-[11px] whitespace-pre-wrap">
                                        {m.content}
                                    </span>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Results */}
            <div className="flex-1 overflow-y-auto space-y-3 min-h-0">
                {runs.length === 0 ? (
                    <div className="h-full flex flex-col items-center justify-center text-muted-foreground/40">
                        <Sparkles className="h-12 w-12 mb-4 opacity-20" />
                        <p className="text-sm">
                            Select a version and click Run to test your prompt
                        </p>
                    </div>
                ) : (
                    runs.map((run) => (
                        <Card
                            key={run.id}
                            className={cn(
                                "border-border/60 overflow-hidden",
                                run.loading && "animate-pulse"
                            )}
                        >
                            <CardHeader className="py-2 px-4 border-b bg-muted/20 flex flex-row items-center justify-between">
                                <div className="flex items-center gap-2">
                                    <Badge
                                        variant="secondary"
                                        className="text-[9px] font-mono px-1 h-4"
                                    >
                                        {run.model}
                                    </Badge>
                                    {run.status && (
                                        <Badge
                                            variant={run.status < 300 ? "success" : "destructive"}
                                            className="text-[9px] px-1 h-4"
                                        >
                                            {run.status}
                                        </Badge>
                                    )}
                                </div>
                                <div className="flex items-center gap-3 text-[10px] text-muted-foreground">
                                    {run.latencyMs !== null && (
                                        <span className="flex items-center gap-1">
                                            <Zap className="h-2.5 w-2.5" /> {run.latencyMs}ms
                                        </span>
                                    )}
                                    {run.tokens && (
                                        <span className="flex items-center gap-1">
                                            <Hash className="h-2.5 w-2.5" />{" "}
                                            {run.tokens.prompt + run.tokens.completion} tok
                                        </span>
                                    )}
                                </div>
                            </CardHeader>
                            <CardContent className="p-4">
                                {run.loading ? (
                                    <div className="flex items-center gap-2 text-muted-foreground">
                                        <Loader2 className="h-4 w-4 animate-spin" />
                                        <span className="text-xs">Running...</span>
                                    </div>
                                ) : (
                                    <pre className="text-xs font-mono whitespace-pre-wrap text-foreground/90">
                                        {run.response}
                                    </pre>
                                )}
                            </CardContent>
                        </Card>
                    ))
                )}
            </div>
        </div>
    );
}
