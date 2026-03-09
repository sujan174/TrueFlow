import { useState, useEffect } from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Loader2, ArrowLeft, Clock } from "lucide-react";
import { PolicyVersion, listPolicyVersions } from "@/lib/api";
import { formatDistanceToNow } from "date-fns";

export function PolicyHistoryDialog({ policyId, open, onOpenChange }: { policyId: string; open: boolean; onOpenChange: (open: boolean) => void }) {
    const [versions, setVersions] = useState<PolicyVersion[]>([]);
    const [loading, setLoading] = useState(false);
    const [selectedVersion, setSelectedVersion] = useState<PolicyVersion | null>(null);

    useEffect(() => {
        let active = true;
        if (open && policyId) {
            setTimeout(() => {
                if (active) setLoading(true);
            }, 0);
            listPolicyVersions(policyId)
                .then((data) => {
                    if (active) setVersions(data);
                })
                .catch(console.error)
                .finally(() => {
                    if (active) setLoading(false);
                });
            setTimeout(() => {
                if (active) setSelectedVersion(null);
            }, 0);
        }
        return () => {
            active = false;
        };
    }, [open, policyId]);

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-[800px] h-[80vh] flex flex-col p-0 gap-0 overflow-hidden">
                <DialogHeader className="p-4 pb-4 border-b">
                    <DialogTitle className="flex items-center gap-2">
                        <Clock className="w-5 h-5 text-muted-foreground" />
                        Policy History
                    </DialogTitle>
                </DialogHeader>

                <div className="flex flex-1 overflow-hidden">
                    {/* List */}
                    <div className={`w-full md:w-1/3 border-r border-border/40 flex flex-col bg-muted/5 ${selectedVersion ? 'hidden md:flex' : 'flex'}`}>
                        <div className="flex-1 overflow-y-auto">
                            {loading ? (
                                <div className="p-4 flex justify-center"><Loader2 className="animate-spin text-muted-foreground" /></div>
                            ) : versions.length === 0 ? (
                                <div className="p-4 text-center text-muted-foreground text-sm">No history found</div>
                            ) : (
                                <div className="divide-y divide-border/40">
                                    {versions.map((v) => (
                                        <button
                                            key={v.version}
                                            onClick={() => setSelectedVersion(v)}
                                            className={`w-full text-left p-4 hover:bg-muted/50 transition-colors ${selectedVersion?.version === v.version ? 'bg-muted border-l-2 border-l-primary/50' : ''}`}
                                        >
                                            <div className="flex items-center justify-between mb-1">
                                                <span className="font-mono text-xs font-semibold">v{v.version}</span>
                                                <Badge variant="outline" className="text-[10px] h-5 px-1.5">{v.mode || 'enforce'}</Badge>
                                            </div>
                                            <div className="text-xs text-muted-foreground">
                                                {formatDistanceToNow(new Date(v.created_at), { addSuffix: true })}
                                            </div>
                                            {/* v.changed_by is not currently populated in API but prepared for future */}
                                            {v.changed_by && <div className="text-[10px] text-muted-foreground mt-1 truncate">by {v.changed_by}</div>}
                                        </button>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Detail */}
                    <div className={`flex-1 flex flex-col bg-background ${!selectedVersion ? 'hidden md:flex' : 'flex'}`}>
                        {selectedVersion ? (
                            <div className="flex flex-col h-full">
                                <div className="p-4 border-b flex items-center gap-2 md:hidden">
                                    <Button variant="ghost" size="sm" onClick={() => setSelectedVersion(null)}>
                                        <ArrowLeft className="w-4 h-4 mr-2" /> Back
                                    </Button>
                                </div>
                                <div className="p-4 overflow-y-auto flex-1 space-y-6">
                                    <div>
                                        <h3 className="text-lg font-semibold flex items-center gap-2">
                                            Version {selectedVersion.version}
                                            <span className="text-xs font-normal text-muted-foreground font-mono">
                                                {new Date(selectedVersion.created_at).toLocaleString()}
                                            </span>
                                        </h3>
                                    </div>

                                    <div className="grid grid-cols-2 gap-4">
                                        <div className="p-3 rounded-md border bg-muted/5">
                                            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Mode</label>
                                            <div className="text-sm font-medium mt-1">{selectedVersion.mode || 'enforce'}</div>
                                        </div>
                                        <div className="p-3 rounded-md border bg-muted/5">
                                            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Phase</label>
                                            <div className="text-sm font-medium mt-1">{selectedVersion.phase || 'pre-flight'}</div>
                                        </div>
                                    </div>

                                    <div>
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2 block">Rules Configuration</label>
                                        <div className="rounded-md border bg-muted/50 p-4 font-mono text-xs overflow-x-auto whitespace-pre">
                                            {JSON.stringify(selectedVersion.rules, null, 2)}
                                        </div>
                                    </div>

                                    {!!selectedVersion.retry && (
                                        <div>
                                            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2 block">Retry Policy</label>
                                            <div className="rounded-md border bg-muted/50 p-4 font-mono text-xs overflow-x-auto whitespace-pre">
                                                {JSON.stringify(selectedVersion.retry, null, 2)}
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </div>
                        ) : (
                            <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground text-sm p-4">
                                <Clock className="w-12 h-12 mb-4 opacity-20" />
                                <p>Select a version from the list to view details</p>
                            </div>
                        )}
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
