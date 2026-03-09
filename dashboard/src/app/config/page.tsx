"use client";

import { useState } from "react";
import { exportConfig, exportPolicies, exportTokens, importConfig } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Download, Upload, FileCode, FileJson, Loader2, ClipboardCopy, Check, AlertTriangle } from "lucide-react";
import { toast } from "sonner";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";

function downloadBlob(content: string, filename: string, type: string) {
    const blob = new Blob([content], { type });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
}

export default function ConfigPage() {
    const [loadingExport, setLoadingExport] = useState<string | null>(null);
    const [importContent, setImportContent] = useState("");
    const [importing, setImporting] = useState(false);
    const [importResult, setImportResult] = useState<{ ok: boolean; message: string } | null>(null);
    const [copied, setCopied] = useState(false);
    const [previewContent, setPreviewContent] = useState<string | null>(null);
    const [previewTitle, setPreviewTitle] = useState("");
    // SEC-08: Confirmation dialog for config import
    const [confirmImportOpen, setConfirmImportOpen] = useState(false);

    const handleExport = async (type: "full" | "policies" | "tokens", fmt?: "yaml" | "json") => {
        const format = fmt ?? "yaml";
        const key = `${type}-${format}`;
        setLoadingExport(key);
        try {
            let res: Response;
            if (type === "full") res = await exportConfig(format);
            else if (type === "policies") res = await exportPolicies();
            else res = await exportTokens();

            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const text = await res.text();
            const ext = format === "json" ? "json" : "yaml";
            const filename = `trueflow-${type}-${new Date().toISOString().slice(0, 10)}.${ext}`;
            downloadBlob(text, filename, format === "json" ? "application/json" : "text/yaml");
            setPreviewContent(text);
            setPreviewTitle(filename);
            toast.success(`Exported ${filename}`);
        } catch (e: unknown) {
            const err = e as Error;
            toast.error(`Export failed: ${err.message}`);
        } finally {
            setLoadingExport(null);
        }
    };

    const handleImport = async () => {
        setConfirmImportOpen(false);
        if (!importContent.trim()) {
            toast.error("Paste your YAML or JSON config first");
            return;
        }
        setImporting(true);
        setImportResult(null);
        try {
            const result = await importConfig(importContent);
            setImportResult({ ok: true, message: result.message });
            toast.success(result.message);
        } catch (e: unknown) {
            const err = e as Error;
            setImportResult({ ok: false, message: err.message });
            toast.error(`Import failed: ${err.message}`);
        } finally {
            setImporting(false);
        }
    };

    const handleCopyPreview = () => {
        if (!previewContent) return;
        navigator.clipboard.writeText(previewContent);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    return (
        <div className="space-y-4">
            <p className="text-xs text-muted-foreground">
                Export your gateway configuration as YAML or JSON. Import to sync policies and tokens across environments (CI/CD, staging → production).
            </p>

            <Tabs defaultValue="export">
                <TabsList>
                    <TabsTrigger value="export">
                        <Download className="h-3.5 w-3.5 mr-1.5" /> Export
                    </TabsTrigger>
                    <TabsTrigger value="import">
                        <Upload className="h-3.5 w-3.5 mr-1.5" /> Import
                    </TabsTrigger>
                </TabsList>

                {/* ── Export Tab ── */}
                <TabsContent value="export" className="space-y-4 pt-3">
                    <div className="grid gap-3 md:grid-cols-3">
                        {/* Full Config */}
                        <Card>
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm flex items-center gap-2">
                                    <FileCode className="h-4 w-4 text-violet-400" />
                                    Full Config
                                </CardTitle>
                                <CardDescription className="text-[11px]">
                                    All policies + token stubs. Best for backup and full environment replication.
                                </CardDescription>
                            </CardHeader>
                            <CardContent className="space-y-2">
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="w-full gap-2"
                                    disabled={!!loadingExport}
                                    onClick={() => handleExport("full", "yaml")}
                                >
                                    {loadingExport === "full-yaml" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
                                    Download YAML
                                </Button>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="w-full gap-2"
                                    disabled={!!loadingExport}
                                    onClick={() => handleExport("full", "json")}
                                >
                                    {loadingExport === "full-json" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <FileJson className="h-3.5 w-3.5" />}
                                    Download JSON
                                </Button>
                            </CardContent>
                        </Card>

                        {/* Policies Only */}
                        <Card>
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm flex items-center gap-2">
                                    <FileCode className="h-4 w-4 text-blue-400" />
                                    Policies Only
                                </CardTitle>
                                <CardDescription className="text-[11px]">
                                    Export all traffic policies. Useful for policy-as-code workflows in Git.
                                </CardDescription>
                            </CardHeader>
                            <CardContent>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="w-full gap-2"
                                    disabled={!!loadingExport}
                                    onClick={() => handleExport("policies")}
                                >
                                    {loadingExport === "policies-yaml" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
                                    Download YAML
                                </Button>
                            </CardContent>
                        </Card>

                        {/* Tokens Only */}
                        <Card>
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm flex items-center gap-2">
                                    <FileCode className="h-4 w-4 text-emerald-400" />
                                    Tokens Only
                                </CardTitle>
                                <CardDescription className="text-[11px]">
                                    Export virtual token stubs (no secrets). Useful for token inventory audits.
                                </CardDescription>
                            </CardHeader>
                            <CardContent>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="w-full gap-2"
                                    disabled={!!loadingExport}
                                    onClick={() => handleExport("tokens")}
                                >
                                    {loadingExport === "tokens-yaml" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
                                    Download YAML
                                </Button>
                            </CardContent>
                        </Card>
                    </div>

                    {/* Preview */}
                    {previewContent && (
                        <Card>
                            <CardHeader className="pb-2 flex flex-row items-center justify-between">
                                <div>
                                    <CardTitle className="text-sm">Preview</CardTitle>
                                    <CardDescription className="text-[11px]">{previewTitle}</CardDescription>
                                </div>
                                <Button variant="ghost" size="sm" className="gap-2" onClick={handleCopyPreview}>
                                    {copied ? <Check className="h-3.5 w-3.5" /> : <ClipboardCopy className="h-3.5 w-3.5" />}
                                    {copied ? "Copied" : "Copy"}
                                </Button>
                            </CardHeader>
                            <CardContent>
                                <pre className="text-[11px] font-mono bg-muted/50 rounded-md p-3 overflow-x-auto max-h-[400px] overflow-y-auto whitespace-pre-wrap break-all">
                                    {previewContent}
                                </pre>
                            </CardContent>
                        </Card>
                    )}
                </TabsContent>

                {/* ── Import Tab ── */}
                <TabsContent value="import" className="space-y-4 pt-3">
                    <Card>
                        <CardHeader className="pb-2">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <Upload className="h-4 w-4 text-amber-400" />
                                Import Configuration
                            </CardTitle>
                            <CardDescription className="text-[11px]">
                                Paste YAML or JSON config. Policies will be upserted; token stubs will be created (credentials not overwritten).
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-3">
                            <div className="rounded-md border border-amber-500/20 bg-amber-500/5 p-3 flex items-start gap-2">
                                <AlertTriangle className="h-4 w-4 text-amber-500 shrink-0 mt-0.5" />
                                <p className="text-[11px] text-amber-400">
                                    Import will overwrite existing policies with the same name. Tokens with matching IDs will be updated. This cannot be undone.
                                </p>
                            </div>
                            <textarea
                                value={importContent}
                                onChange={e => setImportContent(e.target.value)}
                                placeholder={`# Paste YAML or JSON config here\npolicies:\n  - name: my-policy\n    mode: enforce\n    rules: []\n`}
                                className="w-full h-64 font-mono text-xs bg-muted/30 border border-border rounded-md p-3 resize-none focus:outline-none focus:ring-1 focus:ring-primary/40"
                            />
                            {importResult && (
                                <div className={`flex items-center gap-2 text-sm rounded-md p-2 ${importResult.ok ? "bg-emerald-500/10 text-emerald-400" : "bg-red-500/10 text-red-400"}`}>
                                    {importResult.ok ? <Check className="h-4 w-4 shrink-0" /> : <AlertTriangle className="h-4 w-4 shrink-0" />}
                                    {importResult.message}
                                </div>
                            )}
                            {/* SEC-08: Require confirmation before destructive import */}
                            <Button
                                onClick={() => setConfirmImportOpen(true)}
                                disabled={importing || !importContent.trim()}
                                className="w-full"
                            >
                                {importing ? (
                                    <><Loader2 className="h-4 w-4 mr-2 animate-spin" /> Importing…</>
                                ) : (
                                    <><Upload className="h-4 w-4 mr-2" /> Import Config</>
                                )}
                            </Button>
                        </CardContent>
                    </Card>
                </TabsContent>
            </Tabs>

            {/* SEC-08: Import Confirmation Dialog */}
            <Dialog open={confirmImportOpen} onOpenChange={setConfirmImportOpen}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2 text-destructive">
                            <AlertTriangle className="h-5 w-5" /> Confirm Config Import
                        </DialogTitle>
                        <DialogDescription>
                            This will <strong>overwrite existing policies</strong> with matching names and update tokens with matching IDs. This action cannot be undone. Are you sure?
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setConfirmImportOpen(false)}>Cancel</Button>
                        <Button variant="destructive" onClick={handleImport}>Yes, Import & Overwrite</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
