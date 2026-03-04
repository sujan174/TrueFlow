"use client";

import { useState, useEffect } from "react";
import useSWR, { mutate } from "swr";
import {
  listTokens,
  createToken,
  revokeToken,
  listCredentials,
  listTeams,
  Token,
  Team,
  Credential,
  CreateTokenRequest,
  swrFetcher,
} from "@/lib/api";
import {
  Plus, RefreshCw, Key, Shield, Trash2, Loader2, AlertTriangle, Blocks
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/empty-state";
import { PageSkeleton } from "@/components/page-skeleton";
import { useRouter } from "next/navigation";
import { Card } from "@/components/ui/card";
import { DataTable } from "@/components/data-table";
import { columns } from "./columns";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogClose,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

export default function TokensPage() {
  const router = useRouter();
  const { data: rawTokens = [], mutate: mutateTokens, isLoading: loading } = useSWR<Token[]>("/tokens", swrFetcher);
  const [createOpen, setCreateOpen] = useState(false);
  const [revokeTokenData, setRevokeTokenData] = useState<Token | null>(null);

  const tokens = [...rawTokens].sort((a, b) => {
    if (a.is_active && !b.is_active) return -1;
    if (!a.is_active && b.is_active) return 1;
    return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
  });

  const handleRevoke = async () => {
    if (!revokeTokenData) return;
    try {
      await mutateTokens(
        async () => {
          await revokeToken(revokeTokenData.id);
          return rawTokens.map(t => t.id === revokeTokenData.id ? { ...t, is_active: false } : t);
        },
        {
          optimisticData: rawTokens.map(t => t.id === revokeTokenData.id ? { ...t, is_active: false } : t),
          rollbackOnError: true,
          revalidate: true
        }
      );
      toast.success("Token revoked successfully");
      setRevokeTokenData(null);
    } catch {
      toast.error("Failed to revoke token");
    }
  };

  const handleCreateToken = async (data: CreateTokenRequest) => {
    const tempToken: Token = {
      id: "temp-" + Date.now(),
      project_id: "",
      name: data.name,
      credential_id: data.credential_id || "",
      upstream_url: data.upstream_url,
      scopes: [],
      policy_ids: [],
      log_level: data.log_level ?? 1,
      is_active: true,
      created_at: new Date().toISOString(),
      team_id: data.team_id ?? null,
      allowed_models: data.allowed_models ?? null,
      allowed_model_group_ids: null,
      tags: data.tags ?? null,
      mcp_allowed_tools: data.mcp_allowed_tools ?? null,
      mcp_blocked_tools: data.mcp_blocked_tools ?? null,
    };

    await mutateTokens(
      async () => {
        await createToken(data);
        // Trigger revalidation to get the real token
        return [...rawTokens, tempToken];
      },
      {
        optimisticData: [...rawTokens, tempToken],
        rollbackOnError: true,
        revalidate: true
      }
    );
  };

  const activeCount = tokens.filter((t) => t.is_active).length;
  const revokedCount = tokens.length - activeCount;

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Agents</h1>
          <p className="text-xs text-muted-foreground mt-0.5">Virtual tokens for AI agent authentication.</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button variant="outline" size="sm" onClick={() => mutateTokens()} disabled={loading}>
            <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", loading && "animate-spin")} />
            Refresh
          </Button>
          <Dialog open={createOpen} onOpenChange={setCreateOpen}>
            <DialogTrigger asChild>
              <Button size="sm">
                <Plus className="mr-1.5 h-3.5 w-3.5" /> Create Token
              </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-[480px]">
              <CreateTokenForm
                onSuccess={() => setCreateOpen(false)}
                onCreate={handleCreateToken}
              />
            </DialogContent>
          </Dialog>
        </div>
      </div>

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-3 animate-slide-up">
        <Card className="glass-card hover-lift p-4">
          <div className="flex items-center gap-3">
            <div className="icon-circle-blue">
              <Key className="h-4 w-4" />
            </div>
            <div>
              <p className="text-xl font-semibold tabular-nums">{tokens.length}</p>
              <p className="text-xs text-muted-foreground">Total Tokens</p>
            </div>
          </div>
        </Card>
        <Card className={cn("glass-card hover-lift p-4", activeCount > 0 && "animate-glow border-emerald-500/30")}>
          <div className="flex items-center gap-3">
            <div className="icon-circle-emerald">
              <Shield className="h-4 w-4" />
            </div>
            <div>
              <p className="text-xl font-semibold tabular-nums text-emerald-500">{activeCount}</p>
              <p className="text-xs text-muted-foreground">Active</p>
            </div>
          </div>
        </Card>
        <Card className="glass-card hover-lift p-4">
          <div className="flex items-center gap-3">
            <div className="icon-circle-rose">
              <Trash2 className="h-4 w-4" />
            </div>
            <div>
              <p className="text-xl font-semibold tabular-nums text-rose-500">{revokedCount}</p>
              <p className="text-xs text-muted-foreground">Revoked</p>
            </div>
          </div>
        </Card>
      </div>

      {/* Table */}
      {loading ? (
        <PageSkeleton cards={3} rows={5} />
      ) : tokens.length === 0 ? (
        <EmptyState
          icon={Key}
          title="No tokens created"
          description="Create a virtual token to give your agents controlled access to upstream APIs."
          actionLabel="Create Token"
          onAction={() => setCreateOpen(true)}
          className="bg-card/50 backdrop-blur-sm"
        />
      ) : (
        <div className="animate-slide-up stagger-2">
          <DataTable
            columns={columns}
            data={tokens}
            searchKey="name"
            searchPlaceholder="Filter tokens..."
            onRowClick={(token) => router.push(`/virtual-keys/${token.id}`)}
            meta={{
              onRevoke: (t: Token) => setRevokeTokenData(t),
            }}
          />
        </div>
      )}

      {/* Revoke Confirmation Dialog */}
      <Dialog open={!!revokeTokenData} onOpenChange={(open) => !open && setRevokeTokenData(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <AlertTriangle className="h-5 w-5" /> Revoke Token
            </DialogTitle>
            <DialogDescription>
              Are you sure you want to revoke the token <span className="font-mono font-medium text-foreground">{revokeTokenData?.name}</span>?
              This action cannot be undone and any agents using this token will effectively stop working.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRevokeTokenData(null)}>Cancel</Button>
            <Button variant="destructive" onClick={handleRevoke}>Revoke Token</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// ── Create Token Form ─────────────────────────────

function CreateTokenForm({ onSuccess, onCreate }: { onSuccess: () => void; onCreate: (data: CreateTokenRequest) => Promise<void> }) {
  const [loading, setLoading] = useState(false);
  const [credentials, setCredentials] = useState<Credential[]>([]);
  const [fetchingCreds, setFetchingCreds] = useState(true);
  const [mode, setMode] = useState<"managed" | "passthrough">("managed");
  const [upstreamMode, setUpstreamMode] = useState<"single" | "multi">("single");

  const [formData, setFormData] = useState<CreateTokenRequest>({
    name: "",
    credential_id: "",
    upstream_url: "https://api.openai.com/v1", // Default good DX
    log_level: 1, // 1 = Redacted payload
  });

  // Multi-upstream entries
  const [upstreams, setUpstreams] = useState<Array<{ url: string; weight: string; priority: string }>>([
    { url: "https://api.openai.com/v1", weight: "1", priority: "1" },
  ]);

  // Extended fields
  const [teams, setTeams] = useState<Team[]>([]);
  const [allowedModelsInput, setAllowedModelsInput] = useState("");
  const [tagsInput, setTagsInput] = useState(""); // JSON string for tags kv
  const [mcpAllowedInput, setMcpAllowedInput] = useState("");
  const [mcpBlockedInput, setMcpBlockedInput] = useState("");

  useEffect(() => {
    listCredentials()
      .then(setCredentials)
      .catch(() => toast.error("Failed to load credentials"))
      .finally(() => setFetchingCreds(false));
    listTeams()
      .then(setTeams)
      .catch(() => { }); // teams are optional, soft fail
  }, []);

  const addUpstream = () =>
    setUpstreams((prev) => [...prev, { url: "", weight: "1", priority: String(prev.length + 1) }]);

  const removeUpstream = (i: number) =>
    setUpstreams((prev) => prev.filter((_, idx) => idx !== i));

  const updateUpstream = (i: number, field: "url" | "weight" | "priority", value: string) =>
    setUpstreams((prev) => prev.map((u, idx) => idx === i ? { ...u, [field]: value } : u));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      setLoading(true);
      const payload: CreateTokenRequest = { ...formData };
      if (mode === "passthrough") {
        delete (payload as any).credential_id;
      }
      if (upstreamMode === "multi") {
        // Validate all URLs are filled
        if (upstreams.some((u) => !u.url.trim())) {
          toast.error("All upstream URLs must be filled in");
          return;
        }
        payload.upstreams = upstreams.map((u) => ({
          url: u.url.trim(),
          weight: u.weight ? parseFloat(u.weight) : undefined,
          priority: u.priority ? parseInt(u.priority) : undefined,
        }));
        // Use first upstream as the primary URL for backward compat
        payload.upstream_url = upstreams[0].url.trim();
      }
      // Parse allowed_models from comma-separated input
      if (allowedModelsInput.trim()) {
        payload.allowed_models = allowedModelsInput.split(",").map(m => m.trim()).filter(Boolean);
      }
      // Parse tags from JSON
      if (tagsInput.trim()) {
        try {
          payload.tags = JSON.parse(tagsInput);
        } catch {
          toast.error("Tags must be valid JSON, e.g. {\"team\": \"eng\"}");
          return;
        }
      }
      // Parse MCP allowed tools
      if (mcpAllowedInput.trim()) {
        payload.mcp_allowed_tools = mcpAllowedInput.split(",").map(t => t.trim()).filter(Boolean);
      }
      // Parse MCP blocked tools
      if (mcpBlockedInput.trim()) {
        payload.mcp_blocked_tools = mcpBlockedInput.split(",").map(t => t.trim()).filter(Boolean);
      }
      await onCreate(payload);
      toast.success("Token created successfully");
      onSuccess();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to create token");
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <DialogHeader>
        <DialogTitle>Create Token</DialogTitle>
        <DialogDescription>
          Issue a new virtual token for agent authentication.
        </DialogDescription>
      </DialogHeader>
      <div className="grid gap-4 py-4">
        {/* Credential Mode Selector */}
        <div className="space-y-1.5">
          <Label className="text-xs">Credential Mode</Label>
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={() => setMode("managed")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                mode === "managed"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-muted bg-muted/30 text-muted-foreground hover:bg-muted/50"
              )}
            >
              <Shield className="h-3.5 w-3.5 mx-auto mb-1" />
              Managed Credentials
            </button>
            <button
              type="button"
              onClick={() => setMode("passthrough")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                mode === "passthrough"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-muted bg-muted/30 text-muted-foreground hover:bg-muted/50"
              )}
            >
              <Key className="h-3.5 w-3.5 mx-auto mb-1" />
              Passthrough / BYOK
            </button>
          </div>
          <p className="text-[10px] text-muted-foreground">
            {mode === "managed"
              ? "TrueFlow injects credentials from the vault. Agents never see real API keys."
              : "Agents provide their own API key via X-Real-Authorization header. TrueFlow provides observability, analytics, and policies."}
          </p>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="name" className="text-xs">
            Token Name
          </Label>
          <Input
            id="name"
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
            placeholder="e.g. billing-agent-v1"
            required
          />
        </div>

        {/* Upstream Mode Selector */}
        <div className="space-y-1.5">
          <Label className="text-xs">Upstream Configuration</Label>
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={() => setUpstreamMode("single")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                upstreamMode === "single"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-muted bg-muted/30 text-muted-foreground hover:bg-muted/50"
              )}
            >
              Single Upstream
            </button>
            <button
              type="button"
              onClick={() => setUpstreamMode("multi")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                upstreamMode === "multi"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-muted bg-muted/30 text-muted-foreground hover:bg-muted/50"
              )}
            >
              <Blocks className="h-3.5 w-3.5 mx-auto mb-1" />
              Load Balancer
            </button>
          </div>
        </div>

        {upstreamMode === "single" ? (
          <div className="space-y-1.5">
            <Label htmlFor="upstream" className="text-xs">
              Upstream API URL
            </Label>
            <Input
              id="upstream"
              value={formData.upstream_url}
              onChange={(e) => setFormData({ ...formData, upstream_url: e.target.value })}
              placeholder="https://api.openai.com/v1"
              required
            />
          </div>
        ) : (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-xs">Upstream Endpoints</Label>
              <button
                type="button"
                onClick={addUpstream}
                className="text-[10px] text-primary hover:text-primary/80 flex items-center gap-1 transition-colors"
              >
                <Plus className="h-3 w-3" /> Add endpoint
              </button>
            </div>
            <p className="text-[10px] text-muted-foreground">
              Requests are distributed by weight. Higher priority endpoints are tried first on failover.
            </p>
            <div className="space-y-2 max-h-[220px] overflow-y-auto pr-1">
              {upstreams.map((u, i) => (
                <div key={i} className="rounded-md border border-border/60 bg-muted/20 p-3 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
                      Endpoint {i + 1}
                    </span>
                    {upstreams.length > 1 && (
                      <button
                        type="button"
                        onClick={() => removeUpstream(i)}
                        className="text-rose-400 hover:text-rose-300 transition-colors"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    )}
                  </div>
                  <Input
                    value={u.url}
                    onChange={(e) => updateUpstream(i, "url", e.target.value)}
                    placeholder="https://api.openai.com/v1"
                    className="h-8 text-xs font-mono"
                  />
                  <div className="grid grid-cols-2 gap-2">
                    <div className="space-y-1">
                      <Label className="text-[10px] text-muted-foreground">Weight</Label>
                      <Input
                        type="number"
                        min="0"
                        step="0.1"
                        value={u.weight}
                        onChange={(e) => updateUpstream(i, "weight", e.target.value)}
                        placeholder="1"
                        className="h-7 text-xs"
                      />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-[10px] text-muted-foreground">Priority</Label>
                      <Input
                        type="number"
                        min="1"
                        value={u.priority}
                        onChange={(e) => updateUpstream(i, "priority", e.target.value)}
                        placeholder="1"
                        className="h-7 text-xs"
                      />
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {mode === "managed" && (
          <div className="space-y-1.5">
            <Label htmlFor="cred_id" className="text-xs">
              Backing Credential
            </Label>
            {fetchingCreds ? (
              <div className="h-10 w-full animate-pulse bg-muted rounded-md" />
            ) : (
              <Select
                value={formData.credential_id}
                onValueChange={(val) => setFormData({ ...formData, credential_id: val })}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select a credential..." />
                </SelectTrigger>
                <SelectContent>
                  {credentials.filter(c => c.is_active).map((cred) => (
                    <SelectItem key={cred.id} value={cred.id}>
                      {cred.name} ({cred.provider})
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {credentials.length === 0 && !fetchingCreds && (
              <p className="text-[10px] text-muted-foreground mt-1">
                No active credentials found. Create one first.
              </p>
            )}
          </div>
        )}

        {/* Advanced Options */}
        <div className="space-y-3 pt-2 border-t border-border/50">
          <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Advanced Options</Label>

          {/* Team Assignment */}
          {teams.length > 0 && (
            <div className="space-y-1.5">
              <Label htmlFor="team_id" className="text-xs">Team (optional)</Label>
              <Select
                value={formData.team_id || ""}
                onValueChange={(val) => setFormData({ ...formData, team_id: val || undefined })}
              >
                <SelectTrigger>
                  <SelectValue placeholder="No team assigned" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="">No team</SelectItem>
                  {teams.map((t) => (
                    <SelectItem key={t.id} value={t.id}>{t.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {/* Fallback URL — single upstream only */}
          {upstreamMode === "single" && (
            <div className="space-y-1.5">
              <Label htmlFor="fallback_url" className="text-xs">Fallback URL (optional)</Label>
              <Input
                id="fallback_url"
                value={formData.fallback_url || ""}
                onChange={(e) => setFormData({ ...formData, fallback_url: e.target.value || undefined })}
                placeholder="https://api.backup.com/v1"
              />
              <p className="text-[10px] text-muted-foreground">Used automatically on upstream failure.</p>
            </div>
          )}

          {/* Allowed Models */}
          <div className="space-y-1.5">
            <Label htmlFor="allowed_models" className="text-xs">Allowed Models (optional)</Label>
            <Input
              id="allowed_models"
              value={allowedModelsInput}
              onChange={(e) => setAllowedModelsInput(e.target.value)}
              placeholder="gpt-4o, claude-3-*, gemini-1.5-pro"
            />
            <p className="text-[10px] text-muted-foreground">Comma-separated model names or glob patterns. Leave blank to allow all.</p>
          </div>

          {/* Tags */}
          <div className="space-y-1.5">
            <Label htmlFor="tags" className="text-xs">Tags (optional)</Label>
            <Input
              id="tags"
              value={tagsInput}
              onChange={(e) => setTagsInput(e.target.value)}
              placeholder='{"team": "eng", "env": "prod"}'
            />
            <p className="text-[10px] text-muted-foreground">JSON object for cost attribution and filtering.</p>
          </div>

          {/* MCP Tool Access Control */}
          <div className="space-y-1.5">
            <Label htmlFor="mcp_allowed_tools" className="text-xs">MCP Allowed Tools (optional)</Label>
            <Input
              id="mcp_allowed_tools"
              value={mcpAllowedInput}
              onChange={(e) => setMcpAllowedInput(e.target.value)}
              placeholder="mcp__slack__*, mcp__brave__search"
            />
            <p className="text-[10px] text-muted-foreground">Comma-separated. Only these MCP tools will be injected. Glob patterns supported. Leave blank to allow all.</p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="mcp_blocked_tools" className="text-xs">MCP Blocked Tools (optional)</Label>
            <Input
              id="mcp_blocked_tools"
              value={mcpBlockedInput}
              onChange={(e) => setMcpBlockedInput(e.target.value)}
              placeholder="mcp__*__delete_*, mcp__slack__admin_*"
            />
            <p className="text-[10px] text-muted-foreground">Comma-separated. These MCP tools will be blocked even if in the allowed list. Takes priority.</p>
          </div>
        </div>

        {/* Privacy & Logging */}
        <div className="space-y-1.5 pt-2 border-t border-border/50">
          <Label className="text-xs flex items-center gap-2">
            Privacy & Logging
          </Label>
          <Select
            value={String(formData.log_level)}
            onValueChange={(val) => setFormData({ ...formData, log_level: parseInt(val) })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="0">Metadata Only (No payloads saved)</SelectItem>
              <SelectItem value="1">Redacted Payload (Scrub PII keys/secrets)</SelectItem>
              <SelectItem value="2">Full Payload (Everything saved - Best for debugging)</SelectItem>
            </SelectContent>
          </Select>
          <p className="text-[10px] text-muted-foreground">
            Controls what request and response data is stored in the Gateway's audit logs.
          </p>
        </div>
      </div>
      <DialogFooter>
        <DialogClose asChild>
          <Button variant="outline" type="button">Cancel</Button>
        </DialogClose>
        <Button type="submit" disabled={loading || (mode === "managed" && !formData.credential_id)}>
          {loading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
          {loading ? "Creating..." : "Create Token"}
        </Button>
      </DialogFooter>
    </form>
  );
}
